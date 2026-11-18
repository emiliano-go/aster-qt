// SPDX-License-Identifier: AGPL-3.0-or-later
//! Worker thread: owns the tokio runtime and `ClientRuntime`, receives `Action`s
//! from the UI and pushes typed snapshots back with `AppHandle::update`.

use std::sync::mpsc::{self, Sender};

use aster_client_core::settings as app_settings;
use aster_client_core::{ClientConfig, ClientRuntime, RuntimeOptions};
use rune::AppHandle;

use crate::action::Action;
use crate::state::AppState;

type PendingDelete = (
    String,
    Vec<String>,
    Sender<Result<serde_json::Value, String>>,
);

pub fn start(handle: AppHandle<AppState>) -> Sender<Action> {
    let (tx, rx) = mpsc::channel();
    let run_handle = handle.clone();
    let pending_delete: std::sync::Arc<std::sync::Mutex<Option<PendingDelete>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let pending_delete_worker = pending_delete.clone();
    std::thread::spawn(move || {
        let run = move || -> Result<(), String> {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .build()
                .map_err(|err| format!("runtime: {err}"))?;
            let mut runtime = rt
                .block_on(ClientRuntime::start_with(
                    ClientConfig::default(),
                    RuntimeOptions {
                        check_plan: false,
                        start_workers: true,
                    },
                ))
                .map_err(|err| err.to_string())?;
            let mut folder = "inbox".to_string();
            let mut selected: Option<String> = None;
            emit(
                &rt,
                &runtime,
                &run_handle,
                &folder,
                selected.take(),
                None,
                None,
            );
            emit_compose_data(&rt, &runtime, &run_handle);
            emit_settings(&rt, &runtime, &run_handle);
            loop {
                let Ok(action) = rx.recv() else { return Ok(()) };
                let mut error = None;
                let mut notice = None;
                match action {
                    Action::SetFolder(next) => {
                        folder = next;
                        selected = None;
                    }
                    Action::Select(id) => {
                        let _ = rt.block_on(runtime.ensure_attachments(&id));
                        selected = Some(id);
                    }
                    Action::SetRead { ids, read } => {
                        if let Err(err) = rt.block_on(runtime.set_read(&ids, read)) {
                            error = Some(err.to_string());
                        }
                    }
                    Action::SetPinned { ids, pinned } => {
                        match rt.block_on(runtime.set_pinned(&ids, pinned)) {
                            Ok(()) => {
                                notice = Some(if pinned { "Pinned" } else { "Unpinned" }.into())
                            }
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::SetSpam { ids, spam } => {
                        match rt.block_on(runtime.set_spam(&ids, spam)) {
                            Ok(()) => {
                                notice =
                                    Some(if spam { "Marked as spam" } else { "Not spam" }.into())
                            }
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::RestoreMessages(ids) => {
                        match rt.block_on(runtime.restore_messages(&ids)) {
                            Ok(()) => notice = Some("Restored".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::SetStarred { ids, starred } => {
                        if let Err(err) = rt.block_on(runtime.set_starred(&ids, starred)) {
                            error = Some(err.to_string());
                        }
                    }
                    Action::Archive(ids) => {
                        if let Err(err) = rt.block_on(runtime.move_messages(&ids, "archive")) {
                            error = Some(err.to_string());
                        }
                    }
                    Action::Trash(ids) => {
                        if let Err(err) = rt.block_on(runtime.trash_messages(&ids)) {
                            error = Some(err.to_string());
                        }
                    }
                    Action::SyncNow => {
                        if let Err(err) = rt.block_on(runtime.sync_now()) {
                            error = Some(err.to_string());
                        }
                    }
                    Action::LoadRemoteContent(id) => {
                        match rt.block_on(runtime.message_with_remote_content(&id)) {
                            Ok(_) => selected = Some(id),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::Print(id) => {
                        match runtime
                            .export_message_pdf(&id)
                            .and_then(|path| runtime.open_printed_pdf(&path))
                        {
                            Ok(()) => notice = Some("Print ready".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::SaveAttachment { id, seq } => match runtime.save_attachment(&id, seq) {
                        Ok(path) => notice = Some(format!("Saved {}", path.display())),
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::SaveAllAttachments(id) => match runtime.save_all_attachments(&id) {
                        Ok(paths) => notice = Some(format!("Saved {} attachments", paths.len())),
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::Send(outgoing) => {
                        let result = rt.block_on(runtime.send_markdown_with_options(
                            Some(outgoing.from.as_str()),
                            &outgoing.to,
                            &outgoing.cc,
                            &outgoing.bcc,
                            &outgoing.subject,
                            &outgoing.markdown,
                            &outgoing.attachments,
                            outgoing.in_reply_to.as_deref(),
                            outgoing.references.as_deref(),
                            outgoing.expires_at.as_deref(),
                            outgoing.obscure_subject,
                        ));
                        match result {
                            Ok(()) => notice = Some("Message sent".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::Schedule(outgoing, at) => {
                        let result = rt.block_on(runtime.schedule_markdown_with_attachments(
                            Some(outgoing.from.as_str()),
                            &outgoing.to,
                            &outgoing.cc,
                            &outgoing.bcc,
                            &outgoing.subject,
                            &outgoing.markdown,
                            &outgoing.attachments,
                            &at,
                        ));
                        match result {
                            Ok(_) => notice = Some("Message scheduled".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::LoadSettingsData => {
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::LoadDiagnostics => match runtime.diagnostics() {
                        Ok(value) => {
                            let handle = run_handle.clone();
                            handle.update(move |state, _| state.diagnostics = value);
                        }
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::CreateRule(body) => {
                        match rt.block_on(runtime.create_rule(body)) {
                            Ok(_) => notice = Some("Rule created".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::DeleteRule(id) => {
                        match rt.block_on(runtime.delete_rule(&id)) {
                            Ok(()) => notice = Some("Rule deleted".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::RunRule(id) => {
                        match rt.block_on(runtime.run_rule_on_existing(&id, false)) {
                            Ok(_) => notice = Some("Rule run started".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::CreateAlias {
                        local,
                        domain,
                        display,
                    } => {
                        match rt.block_on(runtime.create_alias(&local, &domain, display.as_deref()))
                        {
                            Ok(_) => notice = Some("Alias created".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::DeleteAlias(id) => {
                        match rt.block_on(runtime.delete_alias(&id)) {
                            Ok(()) => notice = Some("Alias deleted".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::RouteAlias { alias, mailbox } => {
                        match rt.block_on(runtime.route_alias_to_mailbox(&alias, &mailbox)) {
                            Ok(_) => notice = Some("Alias routed".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::CreateMailbox(name) => {
                        match rt.block_on(runtime.create_mailbox(&name)) {
                            Ok(_) => notice = Some("Mailbox created".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::CreateFolder { name, parent } => {
                        match rt.block_on(runtime.create_folder(&name, parent.as_deref())) {
                            Ok(_) => notice = Some("Folder created".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::DeleteFolder(token) => {
                        match rt.block_on(runtime.delete_folder(&token)) {
                            Ok(()) => notice = Some("Folder deleted".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::SaveContact { uid, vcard } => {
                        match rt.block_on(runtime.save_contact(&uid, &vcard)) {
                            Ok(_) => notice = Some("Contact saved".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::DeleteContact(uid) => {
                        match rt.block_on(runtime.delete_contact(&uid)) {
                            Ok(()) => notice = Some("Contact deleted".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::ImportGpgKey => match rt.block_on(runtime.import_pgp_to_gpg_agent()) {
                        Ok(status) => {
                            notice = Some("Key imported into gpg-agent".into());
                            let value = serde_json::to_value(status).unwrap_or_default();
                            let handle = run_handle.clone();
                            handle.update(move |state, _| state.gpg = value);
                        }
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::PublishPgp(kind) => {
                        let result = if kind == "wkd" {
                            rt.block_on(runtime.publish_wkd()).map(|_| ())
                        } else {
                            let addresses = rt
                                .block_on(runtime.send_identities())
                                .into_iter()
                                .map(|identity| identity.address)
                                .collect::<Vec<_>>();
                            rt.block_on(runtime.publish_keyserver(&addresses))
                                .map(|_| ())
                        };
                        match result {
                            Ok(()) => notice = Some("Key published".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::RepairCache => match runtime.repair_cache() {
                        Ok(()) => notice = Some("Cache repaired".into()),
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::SignOut(keep) => match runtime.sign_out(keep) {
                        Ok(()) => {
                            rt.block_on(runtime.stop());
                            std::process::exit(0);
                        }
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::Snooze { ids, until } => {
                        match rt.block_on(runtime.snooze_messages(&ids, &until)) {
                            Ok(()) => notice = Some("Messages snoozed".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::UnsnoozeMessage(id) => {
                        match rt.block_on(runtime.unsnooze_message(&id)) {
                            Ok(()) => notice = Some("Unsnoozed".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::DeletePermanent(ids) => {
                        match rt.block_on(runtime.delete_messages_permanent(&ids)) {
                            Ok(()) => notice = Some("Messages deleted".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::PreviewAttachment { id, seq } => {
                        match runtime.attachment_preview(&id, seq) {
                            Ok(value) => {
                                let text = serde_json::to_string_pretty(&value)
                                    .unwrap_or_else(|_| value.to_string());
                                let handle = run_handle.clone();
                                handle.update(move |state, _| {
                                    state.preview_text = text;
                                    state.preview_open = true;
                                });
                            }
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::Rpc { tool, args, reply } => {
                        if tool == "mail_delete" {
                            let ids = args
                                .get("ids")
                                .and_then(|value| value.as_array())
                                .map(|items| {
                                    items
                                        .iter()
                                        .filter_map(|item| item.as_str().map(str::to_string))
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            if ids.is_empty() {
                                let _ = reply.send(Err("ids is required".into()));
                                continue;
                            }
                            if args
                                .get("__runtime_approved")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false)
                            {
                                let result = match rt.block_on(runtime.delete_messages_permanent(&ids)) {
                                    Ok(()) => Ok(serde_json::json!({ "ok": true })),
                                    Err(err) => Err(err.to_string()),
                                };
                                let _ = reply.send(result);
                                continue;
                            }
                            let token = uuid::Uuid::new_v4().to_string();
                            if let Ok(mut guard) = pending_delete_worker.lock() {
                                *guard = Some((token.clone(), ids.clone(), reply));
                            }
                            let handle = run_handle.clone();
                            handle.update(move |state, _| {
                                state.delete_targets = ids;
                                state.delete_token = Some(token);
                                state.delete_word = rune::TextEdit::new("");
                                state.delete_open = true;
                            });
                            continue;
                        }
                        let data_dir = runtime.data_dir().to_path_buf();
                        let result = rt.block_on(handle_rpc(&runtime, &data_dir, &tool, &args));
                        let _ = reply.send(result);
                        continue;
                    }
                    Action::LoadOutbox => match runtime.outbox_messages() {
                        Ok(rows) => {
                            let value = serde_json::to_value(rows)
                                .unwrap_or_else(|_| serde_json::json!([]));
                            let handle = run_handle.clone();
                            handle.update(move |state, _| state.outbox = value);
                        }
                        Err(err) => error = Some(err.to_string()),
                    },
                    Action::RetryOutbox(id) => {
                        match rt.block_on(runtime.retry_outbox_message(id)) {
                            Ok(()) => notice = Some("Outbox retry queued".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        match runtime.outbox_messages() {
                            Ok(rows) => {
                                let value = serde_json::to_value(rows)
                                    .unwrap_or_else(|_| serde_json::json!([]));
                                let handle = run_handle.clone();
                                handle.update(move |state, _| state.outbox = value);
                            }
                            Err(err) => error = Some(err.to_string()),
                        }
                    }
                    Action::CancelScheduled(id) => {
                        match rt.block_on(runtime.cancel_scheduled(&id)) {
                            Ok(()) => notice = Some("Scheduled message cancelled".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::SendScheduledNow(id) => {
                        match rt.block_on(runtime.send_scheduled_now(&id)) {
                            Ok(()) => notice = Some("Scheduled message sent".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::ArchiveMailingList(list_id) => {
                        match rt.block_on(runtime.archive_mailing_list(&list_id)) {
                            Ok(count) => notice = Some(format!("{count} messages archived")),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::SetAliasEnabled { id, enabled } => {
                        match rt.block_on(runtime.set_alias_enabled(&id, enabled)) {
                            Ok(_) => notice = Some("Alias updated".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::ToggleAliasPin(id) => {
                        match rt.block_on(runtime.toggle_alias_pin(&id)) {
                            Ok(_) => notice = Some("Alias pin updated".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::RenameFolder { token, name } => {
                        match rt.block_on(runtime.rename_folder(&token, &name)) {
                            Ok(()) => notice = Some("Folder renamed".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::MoveFolder { token, parent } => {
                        match rt.block_on(runtime.move_folder(&token, parent.as_deref())) {
                            Ok(()) => notice = Some("Folder moved".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit(
                            &rt,
                            &runtime,
                            &run_handle,
                            &folder,
                            selected.clone(),
                            None,
                            None,
                        );
                    }
                    Action::ReorderRules(ids) => {
                        match rt.block_on(runtime.reorder_rules(&ids)) {
                            Ok(()) => notice = Some("Rules reordered".into()),
                            Err(err) => error = Some(err.to_string()),
                        }
                        emit_settings(&rt, &runtime, &run_handle);
                    }
                    Action::ResolveDelete { token, approved } => {
                        let pending = pending_delete_worker
                            .lock()
                            .ok()
                            .and_then(|mut guard| guard.take());
                        if let Some((pending_token, ids, reply)) = pending {
                            if pending_token == token {
                                if approved {
                                    match rt.block_on(runtime.delete_messages_permanent(&ids)) {
                                        Ok(()) => {
                                            let _ = reply.send(Ok(serde_json::json!({"ok": true})));
                                        }
                                        Err(err) => {
                                            let _ = reply.send(Err(err.to_string()));
                                        }
                                    }
                                } else {
                                    let _ = reply.send(Err("delete denied by the user".into()));
                                }
                                emit(
                                    &rt,
                                    &runtime,
                                    &run_handle,
                                    &folder,
                                    selected.clone(),
                                    None,
                                    None,
                                );
                            }
                        }
                    }
                    Action::Refresh => {}
                }
                emit(
                    &rt,
                    &runtime,
                    &run_handle,
                    &folder,
                    selected.take(),
                    error,
                    notice,
                );
            }
        };
        if let Err(err) = run() {
            handle.update(move |state, _| {
                state.status = err.clone();
                state.toast = Some(err);
            });
        }
    });
    tx
}

async fn handle_rpc(
    runtime: &ClientRuntime,
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
        "__revision" => {
            let revision: u64 = runtime
                .db()
                .with_conn(|conn| conn.query_row("SELECT total_changes()", [], |row| row.get::<_, u64>(0)))
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "revision": revision }))
        }
        "status" => Ok(serde_json::to_value(runtime.status().await).map_err(|e| e.to_string())?),
        "mailboxes_list" => Ok(serde_json::to_value(
            runtime.mailboxes().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "identities" => {
            Ok(serde_json::to_value(runtime.send_identities().await).map_err(|e| e.to_string())?)
        }
        "mail_list" => {
            let folder = args
                .get("folder")
                .and_then(|value| value.as_str())
                .unwrap_or("inbox");
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(50) as usize;
            if folder == "scheduled" {
                runtime
                    .refresh_scheduled()
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Ok(serde_json::to_value(
                runtime
                    .messages(folder, limit, 0)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "mail_export" => {
            let folder = args
                .get("folder")
                .and_then(|value| value.as_str())
                .ok_or("folder is required")?;
            let path = args
                .get("path")
                .and_then(|value| value.as_str())
                .ok_or("path is required")?;
            let count = runtime
                .export_mailbox(folder, std::path::Path::new(path))
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "exported": count }))
        }
        "mail_get_remote" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            Ok(serde_json::to_value(
                runtime
                    .message_with_remote_content(id)
                    .await
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
            Ok(
                serde_json::to_value(runtime.search(query, limit).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            )
        }
        "mailing_lists_list" => Ok(serde_json::to_value(
            runtime.mailing_lists().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "thread_get" => {
            let thread_id = args
                .get("thread_id")
                .and_then(|value| value.as_str())
                .ok_or("thread_id is required")?;
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(50) as usize;
            Ok(serde_json::to_value(
                runtime
                    .thread_messages(thread_id, limit)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "mail_get" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            Ok(
                serde_json::to_value(runtime.message(id).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            )
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
        "mail_bulk_unsubscribe" => {
            let ids = args
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|i| i.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(serde_json::to_value(
                runtime
                    .unsubscribe_messages(&ids)
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "mail_archive_newsletters" => Ok(
            serde_json::json!({"archived": runtime.archive_newsletters().await.map_err(|e| e.to_string())?}),
        ),
        "mail_unsubscribe_info" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("id is required")?;
            let detail = runtime.message(id).map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "list_id": detail.message.list_id,
                "list_name": detail.message.list_name,
                "list_unsubscribe": detail.message.list_unsubscribe,
                "list_unsubscribe_post": detail.message.list_unsubscribe_post
            }))
        }
        "mailing_list_archive" => {
            let list_id = args
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or("list_id is required")?;
            Ok(
                serde_json::json!({"archived": runtime.archive_mailing_list(list_id).await.map_err(|e| e.to_string())?}),
            )
        }
        "mail_pin" => {
            let pinned = args
                .get("pinned")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            runtime
                .set_pinned(&ids(), pinned)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_from_sender_list" => {
            let sender = args
                .get("sender")
                .and_then(|v| v.as_str())
                .ok_or("sender is required")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            Ok(serde_json::to_value(
                runtime
                    .messages_from_sender(sender, limit)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
        }
        "mail_from_sender_archive" => {
            let sender = args
                .get("sender")
                .and_then(|v| v.as_str())
                .ok_or("sender is required")?;
            let rows = runtime
                .messages_from_sender(sender, 1000)
                .map_err(|e| e.to_string())?;
            let ids: Vec<String> = rows.into_iter().map(|m| m.id).collect();
            runtime
                .move_messages(&ids, "archive")
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "count": ids.len() }))
        }
        "mail_from_sender_trash" => {
            let sender = args
                .get("sender")
                .and_then(|v| v.as_str())
                .ok_or("sender is required")?;
            let rows = runtime
                .messages_from_sender(sender, 1000)
                .map_err(|e| e.to_string())?;
            let ids: Vec<String> = rows.into_iter().map(|m| m.id).collect();
            runtime
                .trash_messages(&ids)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "count": ids.len() }))
        }
        "mail_from_sender_move" => {
            let sender = args
                .get("sender")
                .and_then(|v| v.as_str())
                .ok_or("sender is required")?;
            let folder = args
                .get("folder_token")
                .and_then(|v| v.as_str())
                .ok_or("folder_token is required")?;
            let rows = runtime
                .messages_from_sender(sender, 1000)
                .map_err(|e| e.to_string())?;
            let ids: Vec<String> = rows.into_iter().map(|m| m.id).collect();
            runtime
                .move_messages(&ids, folder)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "count": ids.len() }))
        }
        "mail_empty_folder" => {
            let folder = args
                .get("folder")
                .and_then(|value| value.as_str())
                .ok_or("folder is required")?;
            Ok(serde_json::json!({
                "deleted": runtime.empty_folder(folder).await.map_err(|e| e.to_string())?
            }))
        }
        "mail_trash" => {
            runtime
                .trash_messages(&ids())
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_schedule" => {
            let parse = |key: &str| -> Vec<String> {
                args.get(key)
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            let subject = args
                .get("subject")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let markdown = args
                .get("markdown")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let scheduled_at = args
                .get("scheduled_at")
                .and_then(|value| value.as_str())
                .ok_or("scheduled_at is required")?;
            let id = runtime
                .schedule_markdown_with_attachments(
                    None,
                    &parse("to"),
                    &parse("cc"),
                    &parse("bcc"),
                    subject,
                    markdown,
                    &[],
                    scheduled_at,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "scheduled_id": id }))
        }
        "mail_scheduled_cancel" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            runtime
                .cancel_scheduled(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_spam" => {
            let spam = args
                .get("spam")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            runtime
                .set_spam(&ids(), spam)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_restore" => {
            runtime
                .restore_messages(&ids())
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_delete" => Err(
            "permanent delete from MCP requires the app UI; use the message menu in Aster".into(),
        ),
        "rules_list" => Ok(runtime.rules().await.map_err(|e| e.to_string())?),
        "rules_create" => {
            let rule = args.get("rule").cloned().ok_or("rule is required")?;
            Ok(runtime.create_rule(rule).await.map_err(|e| e.to_string())?)
        }
        "rules_update" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("id is required")?;
            let rule = args.get("rule").cloned().ok_or("rule is required")?;
            runtime
                .update_rule(id, rule)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "rules_reorder" => {
            let order: Vec<String> = args
                .get("order")
                .and_then(|v| v.as_array())
                .ok_or("order is required")?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            runtime
                .reorder_rules(&order)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "draft_save" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let parse = |name: &str| -> Vec<String> {
                args.get(name)
                    .and_then(|v| v.as_str())
                    .map(|value| {
                        value
                            .split(',')
                            .map(str::trim)
                            .filter(|p| !p.is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let attachments: Vec<std::path::PathBuf> = args
                .get("attachments")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(std::path::PathBuf::from)
                        .collect()
                })
                .unwrap_or_default();
            let id = runtime
                .save_draft(
                    if id.is_empty() { None } else { Some(id) },
                    &parse("to"),
                    &parse("cc"),
                    &parse("bcc"),
                    args.get("subject").and_then(|v| v.as_str()).unwrap_or(""),
                    args.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                    &attachments,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "id": id }))
        }
        "recipient_key_show" => runtime
            .recipient_key_show(
                args.get("address")
                    .and_then(|v| v.as_str())
                    .ok_or("address is required")?,
            )
            .await
            .map_err(|e| e.to_string()),
        "recipient_key_accept" => {
            let address = args
                .get("address")
                .and_then(|v| v.as_str())
                .ok_or("address is required")?;
            let fingerprint = args
                .get("fingerprint")
                .and_then(|v| v.as_str())
                .ok_or("fingerprint is required")?;
            runtime
                .recipient_key_accept(address, fingerprint)
                .await
                .map_err(|e| e.to_string())
        }
        "folder_create" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("name is required")?;
            let parent = args.get("parent_token").and_then(|v| v.as_str());
            let token = runtime
                .create_folder(name, parent)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "folder_token": token }))
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
        "templates_list" => Ok(serde_json::to_value(
            runtime
                .server_templates()
                .await
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "template_create" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("name is required")?;
            let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("");
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("content is required")?;
            Ok(
                serde_json::json!({"id": runtime.create_server_template(name, category, content).await.map_err(|e| e.to_string())?}),
            )
        }
        "template_delete" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("id is required")?;
            runtime
                .delete_server_template(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"ok": true}))
        }
        "signatures_list" => Ok(serde_json::to_value(
            runtime
                .server_signatures()
                .await
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "signature_create" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("name is required")?;
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("content is required")?;
            Ok(
                serde_json::json!({"id": runtime.create_server_signature(name, content).await.map_err(|e| e.to_string())?}),
            )
        }
        "signature_delete" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("id is required")?;
            runtime
                .delete_server_signature(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"ok": true}))
        }
        "signature_set_default" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("id is required")?;
            runtime
                .set_default_signature(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"ok": true}))
        }
        "tags_list" => Ok(
            serde_json::to_value(runtime.tags().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?,
        ),
        "tag_create" => {
            let name = args
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or("name is required")?;
            Ok(
                serde_json::to_value(runtime.create_tag(name).await.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            )
        }
        "tag_delete" => {
            let token = args
                .get("tag_token")
                .and_then(|value| value.as_str())
                .ok_or("tag_token is required")?;
            runtime.delete_tag(token).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "tag_assign" => {
            let token = args
                .get("tag_token")
                .and_then(|value| value.as_str())
                .ok_or("tag_token is required")?;
            runtime
                .assign_tag(&ids(), token)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "tag_remove" => {
            let token = args
                .get("tag_token")
                .and_then(|value| value.as_str())
                .ok_or("tag_token is required")?;
            runtime
                .remove_tag(&ids(), token)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "folder_rename" => {
            let token = args
                .get("folder_token")
                .and_then(|value| value.as_str())
                .ok_or("folder_token is required")?;
            let name = args
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or("name is required")?;
            runtime
                .rename_folder(token, name)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "folder_delete" => {
            let token = args
                .get("folder_token")
                .and_then(|value| value.as_str())
                .ok_or("folder_token is required")?;
            runtime
                .delete_folder(token)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "folder_move" => {
            let token = args
                .get("folder_token")
                .and_then(|value| value.as_str())
                .ok_or("folder_token is required")?;
            let parent = args.get("parent_token").and_then(|value| value.as_str());
            runtime
                .move_folder(token, parent)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_snooze" => {
            let until = args
                .get("until_iso8601")
                .and_then(|value| value.as_str())
                .ok_or("until_iso8601 is required")?;
            runtime
                .snooze_messages(&ids(), until)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_unsnooze" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            runtime
                .unsnooze_message(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "attachment_fetch" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or("id is required")?;
            runtime
                .ensure_attachments(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
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
        "gpg_status" => Ok(serde_json::to_value(runtime.gpg_status()).map_err(|e| e.to_string())?),
        "gpg_import_key" => Ok(serde_json::to_value(
            runtime
                .import_pgp_to_gpg_agent()
                .await
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "pgp_key_info" => Ok(runtime.pgp_key().await.map_err(|e| e.to_string())?),
        "pgp_status" => {
            let wkd = runtime
                .wkd_status()
                .await
                .unwrap_or_else(|_| serde_json::json!({}));
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
        "mail_send_queued" => {
            let to = args
                .get("to")
                .and_then(|v| v.as_str())
                .ok_or("to is required")?;
            let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            let markdown = args.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
            let delay = args
                .get("delay_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(10);
            let parse = |value: String| -> Vec<String> {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            let id = runtime
                .queue_markdown_with_attachments(
                    None,
                    &parse(to.to_string()),
                    &[],
                    &[],
                    subject,
                    markdown,
                    &[],
                    delay,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "queue_id": id }))
        }
        "mail_cancel_queued" => {
            let id = args
                .get("queue_id")
                .and_then(|v| v.as_str())
                .ok_or("queue_id is required")?;
            runtime
                .cancel_queued_send(id)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
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

fn emit_settings(
    rt: &tokio::runtime::Runtime,
    runtime: &ClientRuntime,
    handle: &AppHandle<AppState>,
) {
    let rules = rt
        .block_on(runtime.rules())
        .unwrap_or_else(|_| serde_json::json!({}));
    let aliases = rt
        .block_on(runtime.aliases())
        .map(|value| serde_json::to_value(value).unwrap_or_else(|_| serde_json::json!([])))
        .unwrap_or_else(|_| serde_json::json!([]));
    let pgp = rt
        .block_on(runtime.pgp_key())
        .unwrap_or_else(|_| serde_json::json!({}));
    let gpg = serde_json::to_value(runtime.gpg_status()).unwrap_or_default();
    let diagnostics = runtime.diagnostics().unwrap_or_default();
    handle.update(move |state, _| {
        state.rules = rules;
        state.aliases = aliases;
        state.pgp = pgp;
        state.gpg = gpg;
        state.diagnostics = diagnostics;
    });
}

fn emit_compose_data(
    rt: &tokio::runtime::Runtime,
    runtime: &ClientRuntime,
    handle: &AppHandle<AppState>,
) {
    let identities = rt.block_on(runtime.send_identities());
    let templates = rt.block_on(runtime.server_templates()).unwrap_or_default();
    let signatures = rt.block_on(runtime.server_signatures()).unwrap_or_default();
    handle.update(move |state, _| {
        state.identities = identities;
        state.templates = templates;
        state.signatures = signatures;
    });
}

fn emit(
    rt: &tokio::runtime::Runtime,
    runtime: &ClientRuntime,
    handle: &AppHandle<AppState>,
    folder: &str,
    selected: Option<String>,
    error: Option<String>,
    notice: Option<String>,
) {
    let mailboxes = runtime.mailboxes().unwrap_or_default();
    let unread = mailboxes
        .iter()
        .filter(|mailbox| mailbox.id == "inbox")
        .map(|mailbox| mailbox.unread)
        .sum::<u32>();
    crate::notify_linux::check_new_mail(unread);
    let counts = runtime
        .view_counts()
        .unwrap_or_else(|_| serde_json::json!({}));
    let messages = runtime.messages(folder, 100, 0).unwrap_or_default();
    let selected = selected
        .filter(|id| messages.iter().any(|message| &message.id == id))
        .or_else(|| messages.first().map(|message| message.id.clone()));
    let detail = selected.as_ref().and_then(|id| runtime.message(id).ok());
    let thread = detail
        .as_ref()
        .and_then(|detail| detail.message.thread_id.clone())
        .and_then(|thread_id| runtime.thread_messages(&thread_id, 50).ok())
        .unwrap_or_default();
    let status = rt.block_on(runtime.status());
    let folder = folder.to_string();
    handle.update(move |state, _| {
        state.mailboxes = mailboxes;
        state.counts = counts;
        state.messages = messages;
        state.selected = selected;
        state.detail = detail;
        state.thread = thread;
        state.refresh_visible();
        state.account = status.default_sender.clone();
        state.outbox_pending = status.outbox_pending;
        state.status = if status.outbox_pending == 0 {
            "Ready".to_string()
        } else {
            format!("{} queued", status.outbox_pending)
        };
        if let Some(message) = error.or(notice) {
            state.toast = Some(message);
        }
        if state.folder != folder {
            state.selection.clear();
        }
        state.folder = folder;
    });
}
