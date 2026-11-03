// SPDX-License-Identifier: AGPL-3.0-or-later
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use aster_bridge_core::api_client::ApiClient;
use aster_bridge_core::auth::device_identity;
use aster_bridge_core::auth::session::{self, Session};
use aster_bridge_core::config::{self, BridgeConfig};
use aster_bridge_core::db::Database;
use aster_bridge_core::jmap::state::{self as jmap_state, StateChange};
use aster_bridge_core::outbox;
use aster_bridge_core::runtime::{
    self as bridge_runtime, check_bridge_access, ClientProfile, RuntimeTuning, StartError,
};
use aster_bridge_core::secrets::{self, SecretBackend};
use aster_bridge_core::sync::poller::{self, SyncTrigger};
use aster_bridge_core::tls;
use fs2::FileExt;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;

use crate::config::ClientConfig;
use crate::error::{ClientError, ClientResult};

const TOKEN_RETRY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub check_plan: bool,
    /// Start background sync/outbox/token refresh workers.
    pub start_workers: bool,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            check_plan: true,
            start_workers: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountStatus {
    pub email: String,
    pub username: String,
    pub default_sender: String,
    pub plan_code: String,
    pub data_dir: String,
    pub poll_interval_secs: u64,
    pub full_mail_history: bool,
    pub outbox_pending: usize,
}

pub struct ClientRuntime {
    config: ClientConfig,
    bridge_config: BridgeConfig,
    db: Arc<Database>,
    client: Arc<ApiClient>,
    session: Arc<RwLock<Session>>,
    plan_code: String,
    broadcaster: broadcast::Sender<StateChange>,
    sync_tx: mpsc::Sender<SyncTrigger>,
    pub(crate) outbox_tx: mpsc::Sender<i64>,
    workers: Vec<JoinHandle<()>>,
    _lock: File,
}

impl ClientRuntime {
    pub async fn start(config: ClientConfig) -> ClientResult<Self> {
        Self::start_with(config, RuntimeOptions::default()).await
    }

    pub async fn start_with(config: ClientConfig, opts: RuntimeOptions) -> ClientResult<Self> {
        config.prepare().map_err(ClientError::Storage)?;
        let lock = acquire_lock(&config.lock_path())?;

        secrets::set_data_dir_override(Some(config.data_dir.clone()));
        secrets::install_backend(SecretBackend::Keyring {
            service: config.keyring_service.clone(),
        });
        tls::install_default_crypto_provider();

        let bridge_config = config::load_config().map_err(ClientError::Storage)?;
        let identity = device_identity::get_or_create_identity(&config.data_dir)
            .map_err(ClientError::Secrets)?;
        let device_id = identity.device_id.ok_or(ClientError::NotSignedIn)?;

        let db = Arc::new(Database::open(&config.data_dir).map_err(ClientError::Storage)?);
        db.seed_jmap_mailboxes().map_err(ClientError::Storage)?;
        crate::rules::ensure_schema(&db).map_err(ClientError::Storage)?;
        let client = Arc::new(ApiClient::new());

        let restored = session::restore_or_login(&bridge_config, &identity, &client)
            .await
            .map_err(ClientError::from_bridge)?;
        let session = Arc::new(RwLock::new(restored));

        let tuning = RuntimeTuning::for_config(&bridge_config);
        let plan_code = if opts.check_plan {
            match check_bridge_access(&client, &session, tuning.plan_retry_delay).await {
                Ok(grant) => grant.plan_code().to_string(),
                Err(err) => return Err(map_start_error(err)),
            }
        } else {
            "unchecked".to_string()
        };

        let (broadcaster, _) = broadcast::channel(64);
        let (sync_tx, sync_rx) = poller::sync_trigger_channel();
        let (outbox_tx, outbox_rx) = outbox::outbox_trigger_channel();

        let mut workers = Vec::new();
        if opts.start_workers {
            poller::set_global_sync_trigger(Some(sync_tx.clone()));
            let _ = db.outbox_reset_stale_sending(0);

            workers.push(tokio::spawn({
                let session = session.clone();
                let client = client.clone();
                let db = db.clone();
                let broadcaster = broadcaster.clone();
                let tuning = tuning.poll;
                async move {
                    let _ = poller::run_poll_loop_tuned(
                        session,
                        client,
                        db,
                        Some(broadcaster),
                        sync_rx,
                        tuning,
                    )
                    .await;
                }
            }));

            workers.push(tokio::spawn({
                let session = session.clone();
                let client = client.clone();
                let db = db.clone();
                async move {
                    outbox::run_outbox_loop(session, client, db, outbox_rx).await;
                }
            }));

            workers.push(tokio::spawn({
                let session = session.clone();
                let client = client.clone();
                let signing_key = identity.ed25519_signing_key.clone();
                let interval = tuning.token_refresh_interval;
                async move {
                    session::set_session_rejected(false);
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(interval) => {}
                            _ = session::token_refresh_requested() => {}
                        }
                        if let Err(err) =
                            session::refresh_access_token(&session, device_id, &signing_key, &client)
                                .await
                        {
                            tracing::warn!("token refresh failed: {}", err);
                            tokio::time::sleep(TOKEN_RETRY).await;
                        }
                    }
                }
            }));

            workers.push(tokio::spawn({
                let db = db.clone();
                let every = tuning.blob_gc_interval;
                async move {
                    let mut tick = tokio::time::interval(every);
                    loop {
                        tick.tick().await;
                        match db.jmap_blob_gc(24 * 3600) {
                            Ok(0) => {}
                            Ok(n) => tracing::debug!("jmap_blob GC removed {} blobs", n),
                            Err(e) => tracing::warn!("jmap_blob GC failed: {}", e),
                        }
                    }
                }
            }));
        }

        Ok(Self {
            config,
            bridge_config,
            db,
            client,
            session,
            plan_code,
            broadcaster,
            sync_tx,
            outbox_tx,
            workers,
            _lock: lock,
        })
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn client(&self) -> &Arc<ApiClient> {
        &self.client
    }

    pub fn session(&self) -> &Arc<RwLock<Session>> {
        &self.session
    }

    pub fn data_dir(&self) -> &std::path::Path {
        &self.config.data_dir
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn bridge_config(&self) -> &BridgeConfig {
        &self.bridge_config
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StateChange> {
        self.broadcaster.subscribe()
    }

    pub async fn status(&self) -> AccountStatus {
        let session = self.session.read().await;
        AccountStatus {
            email: session.email.clone(),
            username: session.username.clone(),
            default_sender: session.default_sender_address(),
            plan_code: self.plan_code.clone(),
            data_dir: self.config.data_dir.to_string_lossy().to_string(),
            poll_interval_secs: self.bridge_config.poll_interval_secs,
            full_mail_history: self.bridge_config.full_mail_history,
            outbox_pending: self.db.outbox_pending_count().unwrap_or(0),
        }
    }

    pub async fn sync_now(&self) -> ClientResult<()> {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        self.sync_tx
            .send(SyncTrigger { done: done_tx })
            .await
            .map_err(|_| ClientError::Session("sync worker is not running".into()))?;
        done_rx
            .await
            .map_err(|_| ClientError::Session("sync worker stopped".into()))?
            .map_err(ClientError::Session)
    }

    pub async fn retry_send(&self, id: i64) -> ClientResult<()> {
        self.outbox_tx
            .send(id)
            .await
            .map_err(|_| ClientError::Session("outbox worker is not running".into()))
    }

    pub async fn stop(&mut self) {
        poller::clear_global_sync_trigger_if(&self.sync_tx);
        for worker in self.workers.drain(..) {
            worker.abort();
        }
    }
}

impl Drop for ClientRuntime {
    fn drop(&mut self) {
        for worker in self.workers.drain(..) {
            worker.abort();
        }
    }
}

fn acquire_lock(path: &Path) -> ClientResult<File> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| ClientError::Storage(format!("open {}: {}", path.display(), e)))?;
    file.try_lock_exclusive()
        .map_err(|_| ClientError::AlreadyRunning(path.display().to_string()))?;
    Ok(file)
}

fn map_start_error(err: StartError) -> ClientError {
    match err {
        StartError::BridgeAccessRequired { plan_code } => {
            ClientError::PlanRequired(plan_code.unwrap_or_else(|| "unknown".into()))
        }
        StartError::PlanCheckFailed(message) => ClientError::PlanCheckFailed(message),
        StartError::NotSignedIn => ClientError::NotSignedIn,
        StartError::DeviceRevoked => ClientError::DeviceRevoked,
        other => ClientError::Session(other.to_string()),
    }
}

#[allow(dead_code)]
fn _assert_client_profile(profile: ClientProfile) -> ClientProfile {
    profile
}

#[allow(dead_code)]
fn _assert_runtime_module() {
    let _ = bridge_runtime::token_refresh_wait;
}

#[allow(dead_code)]
fn _assert_state_module() -> broadcast::Sender<StateChange> {
    jmap_state::broadcaster()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("instance.lock");
        let _first = acquire_lock(&path).unwrap();
        assert!(matches!(acquire_lock(&path), Err(ClientError::AlreadyRunning(_))));
    }
}
