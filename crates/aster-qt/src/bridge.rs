// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use aster_client_core::settings as app_settings;
use aster_client_core::{ClientConfig, ClientRuntime, RuntimeOptions};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;

use crate::theme_bridge;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, tokens_json, cxx_name = "tokensJson")]
        #[qproperty(QString, themes_json, cxx_name = "themesJson")]
        #[qproperty(QString, theme_id, cxx_name = "themeId")]
        #[qproperty(QString, theme_mode, cxx_name = "themeMode")]
        #[qproperty(QString, light_theme, cxx_name = "lightTheme")]
        #[qproperty(QString, dark_theme, cxx_name = "darkTheme")]
        #[qproperty(bool, theme_light, cxx_name = "themeLight")]
        #[qproperty(bool, system_dark, cxx_name = "systemDark")]
        #[qproperty(QString, settings_json, cxx_name = "settingsJson")]
        #[qproperty(QString, data_dir, cxx_name = "dataDir")]
        #[qproperty(QString, account_email, cxx_name = "accountEmail")]
        #[qproperty(QString, status_text, cxx_name = "statusText")]
        #[qproperty(QString, mailboxes_json, cxx_name = "mailboxesJson")]
        #[qproperty(QString, messages_json, cxx_name = "messagesJson")]
        #[qproperty(QString, selected_json, cxx_name = "selectedJson")]
        #[qproperty(QString, rules_json, cxx_name = "rulesJson")]
        #[qproperty(QString, identities_json, cxx_name = "identitiesJson")]
        #[qproperty(QString, toast_text, cxx_name = "toastText")]
        #[qproperty(QString, pgp_json, cxx_name = "pgpJson")]
        #[qproperty(QString, contacts_json, cxx_name = "contactsJson")]
        #[qproperty(QString, folder_id, cxx_name = "folderId")]
        #[qproperty(bool, loading, cxx_name = "loading")]
        #[qproperty(bool, ready, cxx_name = "ready")]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn boot(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "confirmDelete"]
        fn confirm_delete(self: Pin<&mut Self>, token: QString, ids: QString);

        #[qinvokable]
        #[cxx_name = "resolveDelete"]
        fn resolve_delete(self: &Self, token: &QString, approved: bool);

        #[qinvokable]
        #[cxx_name = "refreshMail"]
        fn refresh_mail(self: &Self);

        #[qinvokable]
        #[cxx_name = "openMailbox"]
        fn open_mailbox(self: &Self, folder: &QString);

        #[qinvokable]
        #[cxx_name = "selectMessage"]
        fn select_message(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "loadSettingsData"]
        fn load_settings_data(self: &Self);

        #[qinvokable]
        #[cxx_name = "createRuleJson"]
        fn create_rule_json(self: &Self, body: &QString);

        #[qinvokable]
        #[cxx_name = "deleteRule"]
        fn delete_rule(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "runRule"]
        fn run_rule(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "routeAlias"]
        fn route_alias(self: &Self, alias: &QString, mailbox: &QString);

        #[qinvokable]
        #[cxx_name = "createMailbox"]
        fn create_mailbox(self: &Self, name: &QString);

        #[qinvokable]
        #[cxx_name = "sendMessage"]
        fn send_message(
            self: &Self,
            to: &QString,
            cc: &QString,
            bcc: &QString,
            subject: &QString,
            markdown: &QString,
        );

        #[qinvokable]
        #[cxx_name = "saveAttachment"]
        fn save_attachment(self: &Self, id: &QString, seq: i64);

        #[qinvokable]
        #[cxx_name = "saveContact"]
        fn save_contact(self: &Self, uid: &QString, vcard: &QString);

        #[qinvokable]
        #[cxx_name = "deleteContact"]
        fn delete_contact(self: &Self, uid: &QString);

        #[qinvokable]
        #[cxx_name = "publishPgp"]
        fn publish_pgp(self: &Self, kind: &QString);

        #[qinvokable]
        #[cxx_name = "applySettings"]
        fn apply_settings(self: Pin<&mut Self>, patch: &QString);

        #[qinvokable]
        #[cxx_name = "setThemeSlot"]
        fn set_theme_slot(self: Pin<&mut Self>, id: &QString, slot: &QString);

        #[qinvokable]
        #[cxx_name = "applyThemeMode"]
        fn apply_theme_mode(self: Pin<&mut Self>, mode: &QString);

        #[qinvokable]
        #[cxx_name = "applySystemDark"]
        fn apply_system_dark(self: Pin<&mut Self>, dark: bool);

        #[qinvokable]
        #[cxx_name = "refreshThemes"]
        fn refresh_themes(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "openThemesFolder"]
        fn open_themes_folder(self: &Self);

        #[qinvokable]
        #[cxx_name = "createThemeFromTemplate"]
        fn create_theme_from_template(self: Pin<&mut Self>) -> QString;
    }

    impl cxx_qt::Threading for AppController {}
}

pub(crate) enum Command {
    Refresh,
    OpenMailbox(String),
    Select(String),
    LoadSettingsData,
    CreateRule(String),
    DeleteRule(String),
    RunRule(String),
    RouteAlias(String, String),
    CreateMailbox(String),
    SendMessage {
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        markdown: String,
    },
    Rpc {
        tool: String,
        args: serde_json::Value,
        reply: std::sync::mpsc::Sender<Result<serde_json::Value, String>>,
    },
    PgpPublish(String),
    SaveContact { uid: String, vcard: String },
    DeleteContact(String),
    SaveAttachment { id: String, seq: i64 },
}

pub struct AppControllerRust {
    tokens_json: QString,
    themes_json: QString,
    theme_id: QString,
    theme_mode: QString,
    light_theme: QString,
    dark_theme: QString,
    theme_light: bool,
    system_dark: bool,
    settings_json: QString,
    data_dir: QString,
    account_email: QString,
    status_text: QString,
    mailboxes_json: QString,
    messages_json: QString,
    selected_json: QString,
    rules_json: QString,
    identities_json: QString,
    toast_text: QString,
    pgp_json: QString,
    contacts_json: QString,
    folder_id: QString,
    loading: bool,
    ready: bool,
    started: bool,
    tx: Option<Sender<Command>>,
    pending_delete: Arc<Mutex<Option<(String, std::sync::mpsc::Sender<bool>)>>>,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let dir = theme_bridge::default_theme_dir();
        let settings = app_settings::load(&dir);
        let themes = theme_bridge::themes_json(&dir);
        let mode = app_settings::string(&settings, &["appearance", "mode"], "system");
        let light_theme = app_settings::string(&settings, &["appearance", "light"], "aster-light");
        let dark_theme = app_settings::string(&settings, &["appearance", "dark"], "aster-dark");
        let active = active_id(&mode, &light_theme, &dark_theme, false);
        let tokens = theme_bridge::tokens_json(&dir, &active).unwrap_or_else(|| "{}".to_string());
        let light = theme_bridge::theme_is_light(&dir, &active).unwrap_or(false);
        Self {
            tokens_json: tokens.into(),
            themes_json: themes.into(),
            theme_id: active.into(),
            theme_mode: mode.into(),
            light_theme: light_theme.into(),
            dark_theme: dark_theme.into(),
            theme_light: light,
            system_dark: false,
            settings_json: serde_json::to_string_pretty(&settings)
                .unwrap_or_else(|_| "{}".into())
                .into(),
            data_dir: dir.to_string_lossy().to_string().into(),
            account_email: QString::default(),
            status_text: "Starting".into(),
            mailboxes_json: "[]".into(),
            messages_json: "[]".into(),
            selected_json: "{}".into(),
            rules_json: "{}".into(),
            identities_json: "[]".into(),
            toast_text: QString::default(),
            pgp_json: "{}".into(),
            contacts_json: "[]".into(),
            folder_id: "inbox".into(),
            loading: true,
            ready: false,
            started: false,
            tx: None,
            pending_delete: Arc::new(Mutex::new(None)),
        }
    }
}

impl qobject::AppController {
    pub fn boot(mut self: Pin<&mut Self>) {
        if self.started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        self.as_mut().set_loading(true);
        self.as_mut().set_status_text("Opening your mailbox".into());

        let (tx, rx) = mpsc::channel::<Command>();
        self.as_mut().rust_mut().tx = Some(tx.clone());
        let qt = self.as_ref().qt_thread();
        let pending = self.pending_delete.clone();
        let mcp_dir = PathBuf::from(self.data_dir.to_string());
        let rpc_dir = PathBuf::from(self.data_dir.to_string());
        crate::mcp::start(tx.clone(), mcp_dir);

        thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(3)
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    let message = format!("Could not start the mail engine: {err}");
                    let _ = qt.queue(move |mut q| {
                        q.as_mut().set_loading(false);
                        q.as_mut().set_status_text(message.into());
                    });
                    return;
                }
            };

            let mut runtime = match rt.block_on(ClientRuntime::start_with(
                ClientConfig::default(),
                RuntimeOptions {
                    check_plan: false,
                    start_workers: true,
                },
            )) {
                Ok(rt) => rt,
                Err(err) => {
                    let message = format!("{err}");
                    let _ = qt.queue(move |mut q| {
                        q.as_mut().set_loading(false);
                        q.as_mut().set_status_text(message.into());
                    });
                    return;
                }
            };

            let mut folder = "inbox".to_string();
            let mut selected: Option<String> = None;
            rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
            selected = first_id(&runtime, &folder);
            rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));

            while let Ok(command) = rx.recv() {
                let mut snapshot_needed = true;
                match command {
                    Command::Refresh => {}
                    Command::OpenMailbox(next) => {
                        folder = next;
                        selected = first_id(&runtime, &folder);
                    }
                    Command::Select(id) => selected = Some(id),
                    Command::LoadSettingsData => {
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::CreateRule(body) => {
                        let result = rt.block_on(async {
                            let value: serde_json::Value = serde_json::from_str(&body)
                                .map_err(|e| format!("invalid rule JSON: {e}"))?;
                            runtime
                                .create_rule(value)
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Rule created");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::DeleteRule(id) => {
                        let result = rt.block_on(async {
                            runtime.delete_rule(&id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Rule deleted");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::RunRule(id) => {
                        let result = rt.block_on(async {
                            runtime
                                .run_rule_on_existing(&id, false)
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Rule run started");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::RouteAlias(alias, mailbox) => {
                        let result = rt.block_on(async {
                            runtime
                                .route_alias_to_mailbox(&alias, &mailbox)
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Alias mailbox created");
                        rt.block_on(push_settings_data(&runtime, &qt));
                    }
                    Command::CreateMailbox(name) => {
                        let result = rt.block_on(async {
                            runtime
                                .create_mailbox(&name)
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Mailbox created");
                        rt.block_on(push_settings_data(&runtime, &qt));
                    }
                    Command::SendMessage {
                        to,
                        cc,
                        bcc,
                        subject,
                        markdown,
                    } => {
                        let result = rt.block_on(async {
                            let parse = |value: String| -> Vec<String> {
                                value
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|part| !part.is_empty())
                                    .map(str::to_string)
                                    .collect()
                            };
                            runtime
                                .send_markdown(
                                    None,
                                    &parse(to),
                                    &parse(cc),
                                    &parse(bcc),
                                    &subject,
                                    &markdown,
                                )
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message queued for sending");
                        snapshot_needed = false;
                    }
                    Command::SaveContact { uid, vcard } => {
                        let result = rt.block_on(async {
                            runtime
                                .save_contact(&uid, &vcard)
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Contact saved");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::SaveAttachment { id, seq } => {
                        let result = runtime
                            .save_attachment(&id, seq)
                            .map(|path| path.to_string_lossy().to_string())
                            .map_err(|e| e.to_string());
                        let message = match &result {
                            Ok(path) => format!("Saved to {path}"),
                            Err(err) => err.clone(),
                        };
                        let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
                        snapshot_needed = false;
                    }
                    Command::DeleteContact(uid) => {
                        let result = rt.block_on(async {
                            runtime.delete_contact(&uid).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Contact deleted");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::PgpPublish(kind) => {
                        let result = rt.block_on(async {
                            let addresses: Vec<String> = Vec::new();
                            match kind.as_str() {
                                "wkd" => runtime.publish_wkd().await.map(|_| ()),
                                "wkd-off" => runtime.unpublish_wkd().await.map(|_| ()),
                                _ => runtime.publish_keyserver(&addresses).await.map(|_| ()),
                            }
                            .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "PGP publication updated");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::Rpc { tool, args, reply } => {
                        let result = rt.block_on(handle_rpc(
                            &runtime,
                            &pending,
                            &qt,
                            &rpc_dir,
                            &tool,
                            &args,
                        ));
                        let _ = reply.send(result);
                        snapshot_needed = false;
                    }
                }
                if snapshot_needed {
                    rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
                }
            }
            rt.block_on(runtime.stop());
        });
    }

    pub fn refresh_mail(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Refresh);
        }
    }

    pub fn open_mailbox(self: &Self, folder: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::OpenMailbox(folder.to_string()));
        }
    }

    pub fn select_message(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Select(id.to_string()));
        }
    }

    pub fn load_settings_data(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::LoadSettingsData);
        }
    }

    pub fn create_rule_json(self: &Self, body: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CreateRule(body.to_string()));
        }
    }

    pub fn delete_rule(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::DeleteRule(id.to_string()));
        }
    }

    pub fn run_rule(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::RunRule(id.to_string()));
        }
    }

    pub fn route_alias(self: &Self, alias: &QString, mailbox: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::RouteAlias(
                alias.to_string(),
                mailbox.to_string(),
            ));
        }
    }

    pub fn create_mailbox(self: &Self, name: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CreateMailbox(name.to_string()));
        }
    }

    pub fn send_message(
        self: &Self,
        to: &QString,
        cc: &QString,
        bcc: &QString,
        subject: &QString,
        markdown: &QString,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SendMessage {
                to: to.to_string(),
                cc: cc.to_string(),
                bcc: bcc.to_string(),
                subject: subject.to_string(),
                markdown: markdown.to_string(),
            });
        }
    }

    pub fn save_attachment(self: &Self, id: &QString, seq: i64) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SaveAttachment {
                id: id.to_string(),
                seq,
            });
        }
    }

    pub fn save_contact(self: &Self, uid: &QString, vcard: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SaveContact {
                uid: uid.to_string(),
                vcard: vcard.to_string(),
            });
        }
    }

    pub fn delete_contact(self: &Self, uid: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::DeleteContact(uid.to_string()));
        }
    }

    pub fn publish_pgp(self: &Self, kind: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::PgpPublish(kind.to_string()));
        }
    }

    pub fn resolve_delete(self: &Self, token: &QString, approved: bool) {
        let mut guard = match self.pending_delete.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some((pending_token, tx)) = guard.take() {
            if pending_token == token.to_string() {
                let _ = tx.send(approved);
            }
        }
    }

    pub fn set_theme_slot(mut self: Pin<&mut Self>, id: &QString, slot: &QString) {
        let id = id.to_string();
        if slot.to_string() == "light" {
            self.as_mut().set_light_theme(id.into());
        } else {
            self.as_mut().set_dark_theme(id.into());
        }
        self.as_mut().persist_theme_settings();
        self.as_mut().refresh_active_theme();
    }

    pub fn apply_theme_mode(mut self: Pin<&mut Self>, mode: &QString) {
        let mode = match mode.to_string().as_str() {
            "light" | "dark" => mode.to_string(),
            _ => "system".to_string(),
        };
        self.as_mut().set_theme_mode(mode.into());
        self.as_mut().persist_theme_settings();
        self.as_mut().refresh_active_theme();
    }

    pub fn apply_system_dark(mut self: Pin<&mut Self>, dark: bool) {
        self.as_mut().set_system_dark(dark);
        if self.theme_mode.to_string() == "system" {
            self.as_mut().refresh_active_theme();
        }
    }

    pub fn refresh_themes(mut self: Pin<&mut Self>) {
        let dir = PathBuf::from(self.data_dir.to_string());
        let themes = theme_bridge::themes_json(&dir);
        self.as_mut().set_themes_json(themes.into());
        self.as_mut().refresh_active_theme();
    }

    pub fn open_themes_folder(self: &Self) {
        let dir = PathBuf::from(self.data_dir.to_string()).join("themes");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }

    pub fn create_theme_from_template(mut self: Pin<&mut Self>) -> QString {
        let dir = PathBuf::from(self.data_dir.to_string());
        let path = theme_bridge::create_template_theme(&dir);
        self.as_mut().refresh_themes();
        path.unwrap_or_default().to_string_lossy().to_string().into()
    }

    fn persist_theme_settings(mut self: Pin<&mut Self>) {
        let dir = PathBuf::from(self.data_dir.to_string());
        let patch = json!({
            "appearance": {
                "mode": self.theme_mode.to_string(),
                "light": self.light_theme.to_string(),
                "dark": self.dark_theme.to_string(),
            }
        });
        if let Ok(settings) = app_settings::apply_patch(&dir, &patch) {
            self.as_mut().set_settings_json(
                serde_json::to_string_pretty(&settings)
                    .unwrap_or_else(|_| "{}".into())
                    .into(),
            );
        }
    }

    pub fn apply_settings(mut self: Pin<&mut Self>, patch: &QString) {
        let dir = PathBuf::from(self.data_dir.to_string());
        let patch: serde_json::Value = match serde_json::from_str(&patch.to_string()) {
            Ok(value) => value,
            Err(_) => return,
        };
        if let Ok(settings) = app_settings::apply_patch(&dir, &patch) {
            self.as_mut().set_settings_json(
                serde_json::to_string_pretty(&settings)
                    .unwrap_or_else(|_| "{}".into())
                    .into(),
            );
            let mode = app_settings::string(&settings, &["appearance", "mode"], "system");
            let light = app_settings::string(&settings, &["appearance", "light"], "aster-light");
            let dark = app_settings::string(&settings, &["appearance", "dark"], "aster-dark");
            self.as_mut().set_theme_mode(mode.into());
            self.as_mut().set_light_theme(light.into());
            self.as_mut().set_dark_theme(dark.into());
            self.as_mut().refresh_active_theme();
        }
    }

    fn refresh_active_theme(mut self: Pin<&mut Self>) {
        let dir = PathBuf::from(self.data_dir.to_string());
        let mut id = active_id(
            &self.theme_mode.to_string(),
            &self.light_theme.to_string(),
            &self.dark_theme.to_string(),
            self.system_dark,
        );
        if !theme_bridge::theme_exists(&dir, &id) {
            id = if self.system_dark {
                "aster-dark".to_string()
            } else {
                "aster-light".to_string()
            };
        }
        let tokens = theme_bridge::tokens_json(&dir, &id).unwrap_or_else(|| "{}".to_string());
        let light = theme_bridge::theme_is_light(&dir, &id).unwrap_or(false);
        self.as_mut().set_theme_id(id.into());
        self.as_mut().set_theme_light(light);
        self.as_mut().set_tokens_json(tokens.into());
    }
}

fn active_id(mode: &str, light: &str, dark: &str, system_dark: bool) -> String {
    match mode {
        "light" => light.to_string(),
        "dark" => dark.to_string(),
        _ => {
            if system_dark {
                dark.to_string()
            } else {
                light.to_string()
            }
        }
    }
}

fn first_id(runtime: &ClientRuntime, folder: &str) -> Option<String> {
    runtime
        .messages(folder, 1, 0)
        .ok()
        .and_then(|mut rows| rows.pop())
        .map(|msg| msg.id)
}

async fn snapshot(
    runtime: &ClientRuntime,
    folder: &str,
    selected: Option<&str>,
    qt: &cxx_qt::CxxQtThread<qobject::AppController>,
) {
    let mailboxes = runtime
        .mailboxes()
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let messages = runtime
        .messages(folder, 100, 0)
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let detail = selected
        .and_then(|id| runtime.message(id).ok())
        .and_then(|detail| serde_json::to_string(&detail).ok())
        .unwrap_or_else(|| "{}".into());
    let status = runtime.status().await;

    let folder = folder.to_string();
    let _ = qt.queue(move |mut q| {
        q.as_mut().set_account_email(status.default_sender.into());
        q.as_mut().set_mailboxes_json(mailboxes.into());
        q.as_mut().set_messages_json(messages.into());
        q.as_mut().set_selected_json(detail.into());
        q.as_mut().set_folder_id(folder.into());
        q.as_mut().set_loading(false);
        q.as_mut().set_ready(true);
        let label = if status.outbox_pending == 0 {
            "Ready".to_string()
        } else {
            format!("{} queued in outbox", status.outbox_pending)
        };
        q.as_mut().set_status_text(label.into());
    });
}


async fn push_settings_data(
    runtime: &ClientRuntime,
    qt: &cxx_qt::CxxQtThread<qobject::AppController>,
) {
    let rules = runtime
        .rules()
        .await
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "{}".into());
    let identities = serde_json::to_string(&runtime.send_identities().await)
        .unwrap_or_else(|_| "[]".into());
    let pgp = runtime
        .pgp_key()
        .await
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "{}".into());
    let contacts = runtime
        .contacts()
        .await
        .map(|value| serde_json::to_string(&value).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let _ = qt.queue(move |mut q| {
        q.as_mut().set_rules_json(rules.into());
        q.as_mut().set_identities_json(identities.into());
        q.as_mut().set_pgp_json(pgp.into());
        q.as_mut().set_contacts_json(contacts.into());
    });
}

fn notify_result(
    qt: &cxx_qt::CxxQtThread<qobject::AppController>,
    result: Result<(), String>,
    success: &str,
) {
    let message = match result {
        Ok(()) => success.to_string(),
        Err(err) => err,
    };
    let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
}

async fn handle_rpc(
    runtime: &ClientRuntime,
    pending: &Arc<Mutex<Option<(String, std::sync::mpsc::Sender<bool>)>>>,
    qt: &cxx_qt::CxxQtThread<qobject::AppController>,
    data_dir: &std::path::Path,
    tool: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let ids = || {
        args.get("ids")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    match tool {
        "status" => Ok(serde_json::to_value(runtime.status().await).map_err(|e| e.to_string())?),
        "mailboxes_list" => Ok(serde_json::to_value(
            runtime.mailboxes().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "identities" => Ok(serde_json::to_value(runtime.send_identities().await)
            .map_err(|e| e.to_string())?),
        "mail_list" => {
            let folder = args
                .get("folder")
                .and_then(|value| value.as_str())
                .unwrap_or("inbox");
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(50) as usize;
            Ok(serde_json::to_value(
                runtime
                    .messages(folder, limit, 0)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "mail_search" => {
            let query = args
                .get("query")
                .and_then(|value| value.as_str())
                .ok_or("query is required")?;
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(50) as usize;
            Ok(serde_json::to_value(
                runtime.search(query, limit).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "mail_get" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            Ok(serde_json::to_value(runtime.message(id).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?)
        }
        "mail_move" => {
            let folder = args
                .get("folder_token")
                .and_then(|value| value.as_str())
                .ok_or("folder_token is required")?;
            runtime
                .move_messages(&ids(), folder)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_set_read" => {
            let read = args
                .get("read")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            runtime
                .set_read(&ids(), read)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_star" => {
            let starred = args
                .get("starred")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            runtime
                .set_starred(&ids(), starred)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_trash" => {
            runtime
                .trash_messages(&ids())
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_delete" => {
            let ids = ids();
            if ids.is_empty() {
                return Err("ids is required".into());
            }
            let token = uuid::Uuid::new_v4().to_string();
            let (tx, rx) = std::sync::mpsc::channel();
            {
                let mut guard = pending.lock().map_err(|e| e.to_string())?;
                *guard = Some((token.clone(), tx));
            }
            let ids_json = serde_json::to_string(&ids).map_err(|e| e.to_string())?;
            {
                let qt = qt.clone();
                let token = token.clone();
                let _ = qt.queue(move |mut q| {
                    q.as_mut().confirm_delete(token.into(), ids_json.into());
                });
            }
            let approved = tokio::task::spawn_blocking(move || {
                rx.recv_timeout(std::time::Duration::from_secs(120))
            })
            .await
            .map_err(|e| e.to_string())?;
            if let Ok(mut guard) = pending.lock() {
                *guard = None;
            }
            if !matches!(approved, Ok(true)) {
                return Err("delete denied by the user".into());
            }
            runtime
                .delete_messages_permanent(&ids)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "rules_list" => Ok(runtime.rules().await.map_err(|e| e.to_string())?),
        "rules_create" => {
            let rule = args.get("rule").cloned().ok_or("rule is required")?;
            Ok(runtime.create_rule(rule).await.map_err(|e| e.to_string())?)
        }
        "rules_delete" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            runtime.delete_rule(id).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mailbox_create" => {
            let name = args
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or("name is required")?;
            Ok(serde_json::json!({
                "token": runtime.create_mailbox(name).await.map_err(|e| e.to_string())?
            }))
        }
        "alias_route" => {
            let alias = args
                .get("alias")
                .and_then(|value| value.as_str())
                .ok_or("alias is required")?;
            let mailbox = args
                .get("mailbox")
                .and_then(|value| value.as_str())
                .ok_or("mailbox is required")?;
            let (token, rule_id) = runtime
                .route_alias_to_mailbox(alias, mailbox)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "folder_token": token, "rule_id": rule_id }))
        }
        "sync_now" => {
            runtime.sync_now().await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "contacts_list" => Ok(serde_json::to_value(
            runtime.contacts().await.map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "contacts_save" => {
            let uid = args
                .get("uid")
                .and_then(|value| value.as_str())
                .ok_or("uid is required")?;
            let vcard = args
                .get("vcard")
                .and_then(|value| value.as_str())
                .ok_or("vcard is required")?;
            Ok(serde_json::to_value(
                runtime
                    .save_contact(uid, vcard)
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "contacts_delete" => {
            let uid = args
                .get("uid")
                .and_then(|value| value.as_str())
                .ok_or("uid is required")?;
            runtime
                .delete_contact(uid)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "attachment_list" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            Ok(serde_json::json!({
                "attachments": runtime.message(id).map_err(|e| e.to_string())?.attachments
            }))
        }
        "attachment_save" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            let seq = args
                .get("seq")
                .and_then(|value| value.as_i64())
                .ok_or("seq is required")?;
            Ok(serde_json::json!({
                "path": runtime.save_attachment(id, seq).map_err(|e| e.to_string())?
            }))
        }
        "drafts_list" => {
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(50) as usize;
            Ok(serde_json::to_value(
                runtime
                    .messages("drafts", limit, 0)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "pgp_key_info" => Ok(runtime.pgp_key().await.map_err(|e| e.to_string())?),
        "pgp_status" => {
            let wkd = runtime.wkd_status().await.unwrap_or_else(|_| serde_json::json!({}));
            let keyserver = runtime
                .keyserver_status()
                .await
                .unwrap_or_else(|_| serde_json::json!({}));
            Ok(serde_json::json!({ "wkd": wkd, "keyserver": keyserver }))
        }
        "pgp_publish" => {
            let kind = args
                .get("kind")
                .and_then(|value| value.as_str())
                .unwrap_or("wkd");
            if kind == "keyserver" {
                let addresses: Vec<String> = args
                    .get("addresses")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(runtime
                    .publish_keyserver(&addresses)
                    .await
                    .map_err(|e| e.to_string())?)
            } else {
                Ok(runtime.publish_wkd().await.map_err(|e| e.to_string())?)
            }
        }
        "mail_send" => {
            let settings = app_settings::load(data_dir);
            if !app_settings::boolean(&settings, &["mcp", "allow_send"], false) {
                return Err("sending from MCP is disabled in Settings > MCP".into());
            }
            let to = args
                .get("to")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            let cc = args
                .get("cc")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            let bcc = args
                .get("bcc")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            let subject = args
                .get("subject")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let markdown = args
                .get("markdown")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let id = runtime
                .send_markdown(None, &to, &cc, &bcc, subject, markdown)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "outbox_id": id }))
        }
        _ => Err(format!("unknown tool: {tool}")),
    }
}
