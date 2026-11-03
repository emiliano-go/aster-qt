// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::PathBuf;
use std::time::Duration;

use aster_bridge_core::api_client::ApiClient;
use aster_bridge_core::auth::device_identity;
use aster_bridge_core::config::{self, BridgeConfig};
use aster_bridge_core::ops::{self, SignInPoll};
use aster_bridge_core::runtime::ClientProfile;
use aster_bridge_core::secrets::{self, SecretBackend};
use aster_bridge_core::tls;
use aster_client_core::{ClientConfig, ClientRuntime, RuntimeOptions};

struct Args {
    command: String,
    positional: Vec<String>,
    data_dir: Option<PathBuf>,
    limit: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let config = match args.data_dir {
        Some(dir) => ClientConfig::with_data_dir(dir),
        None => ClientConfig::default(),
    };

    match args.command.as_str() {
        "login" => login(config).await?,
        "status" => {
            let mut rt = ClientRuntime::start_with(
                config,
                RuntimeOptions {
                    check_plan: false,
                    start_workers: false,
                },
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&rt.status().await)?);
            rt.stop().await;
        }
        "list" => {
            let folder = args
                .positional
                .first()
                .cloned()
                .unwrap_or_else(|| "inbox".into());
            let rt = ClientRuntime::start_with(
                config,
                RuntimeOptions {
                    check_plan: false,
                    start_workers: false,
                },
            )
            .await?;
            let rows = rt.messages(&folder, args.limit, 0)?;
            for msg in rows {
                println!(
                    "{}\t{}\t{}\t{}",
                    msg.date.as_deref().unwrap_or("-"),
                    msg.sender,
                    msg.subject,
                    msg.id
                );
            }
        }
        "search" => {
            let query = args
                .positional
                .first()
                .ok_or("usage: aster-headless search <query>")?;
            let rt = ClientRuntime::start_with(
                config,
                RuntimeOptions {
                    check_plan: false,
                    start_workers: false,
                },
            )
            .await?;
            for msg in rt.search(query, args.limit)? {
                println!("{}\t{}\t{}", msg.sender, msg.subject, msg.id);
            }
        }
        "show" => {
            let id = args
                .positional
                .first()
                .ok_or("usage: aster-headless show <message-id>")?;
            let rt = ClientRuntime::start_with(
                config,
                RuntimeOptions {
                    check_plan: false,
                    start_workers: false,
                },
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&rt.message(id)?)?);
        }
        "sync" => {
            let mut rt = ClientRuntime::start(config).await?;
            rt.sync_now().await?;
            println!("{}", serde_json::to_string_pretty(&rt.status().await)?);
            rt.stop().await;
        }
        other => return Err(format!("unknown command: {}", other).into()),
    }
    Ok(())
}

async fn login(config: ClientConfig) -> Result<(), Box<dyn std::error::Error>> {
    config.prepare()?;
    secrets::set_data_dir_override(Some(config.data_dir.clone()));
    secrets::install_backend(SecretBackend::Keyring {
        service: config.keyring_service.clone(),
    });
    tls::install_default_crypto_provider();
    let bridge_config: BridgeConfig = config::load_config()?;
    let mut identity = device_identity::get_or_create_identity(&config.data_dir)?;
    let client = ApiClient::new();

    let code = ops::request_device_code(&client, &identity, ClientProfile::Desktop).await?;
    println!("Enter this code at https://app.astermail.org/link-device");
    println!("  code: {}", code.code);
    println!("  device fingerprint: {}", code.device_fingerprint);
    println!("Waiting for approval (valid for {} seconds)...", code.expires_in);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(code.expires_in);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err("device code expired".into());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        match ops::poll_device_sign_in(&client, &mut identity, &config.data_dir, &code.normalized)
            .await?
        {
            SignInPoll::Pending(_) => {}
            SignInPoll::Expired => return Err("device code expired".into()),
            SignInPoll::Confirmed(session) => {
                println!("Signed in as {}", session.default_sender_address());
                let _ = bridge_config;
                return Ok(());
            }
        }
    }
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut command = String::from("status");
    let mut positional = Vec::new();
    let mut data_dir = None;
    let mut limit = 50usize;
    let mut iter = std::env::args().skip(1).peekable();
    let mut first = true;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--data-dir" => {
                data_dir = Some(PathBuf::from(iter.next().ok_or("--data-dir needs a path")?));
            }
            "--limit" => {
                limit = iter
                    .next()
                    .ok_or("--limit needs a number")?
                    .parse()
                    .map_err(|_| "--limit must be a number")?;
            }
            other if first => {
                command = other.to_string();
                first = false;
            }
            other => positional.push(other.to_string()),
        }
    }
    Ok(Args {
        command,
        positional,
        data_dir,
        limit,
    })
}
