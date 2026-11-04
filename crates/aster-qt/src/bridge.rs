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
        #[qproperty(QString, gpg_json, cxx_name = "gpgJson")]
        #[qproperty(QString, thread_json, cxx_name = "threadJson")]
        #[qproperty(QString, lists_json, cxx_name = "listsJson")]
        #[qproperty(QString, tags_json, cxx_name = "tagsJson")]
        #[qproperty(QString, templates_json, cxx_name = "templatesJson")]
        #[qproperty(QString, signatures_json, cxx_name = "signaturesJson")]
        #[qproperty(QString, outbox_json, cxx_name = "outboxJson")]
        #[qproperty(QString, recipient_key_json, cxx_name = "recipientKeyJson")]
        #[qproperty(QString, diagnostics_json, cxx_name = "diagnosticsJson")]
        #[qproperty(QString, view_counts_json, cxx_name = "viewCountsJson")]
        #[qproperty(QString, attachment_preview_json, cxx_name = "attachmentPreviewJson")]
        #[qproperty(QString, folder_id, cxx_name = "folderId")]
        #[qproperty(bool, loading, cxx_name = "loading")]
        #[qproperty(bool, ready, cxx_name = "ready")]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn boot(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "queueReady"]
        fn queue_ready(self: Pin<&mut Self>, id: QString);

        #[qsignal]
        #[cxx_name = "scheduledQueued"]
        fn scheduled_queued(self: Pin<&mut Self>, id: QString, at: QString, subject: QString);

        #[qinvokable]
        #[cxx_name = "scheduleMessage"]
        fn schedule_message(
            self: &Self,
            from: &QString,
            to: &QString,
            cc: &QString,
            bcc: &QString,
            subject: &QString,
            markdown: &QString,
            attachments: &QString,
            delay_seconds: u64,
            at_iso: &QString,
        );

        #[qinvokable]
        #[cxx_name = "sendQueued"]
        fn send_queued(
            self: &Self,
            from: &QString,
            to: &QString,
            cc: &QString,
            bcc: &QString,
            subject: &QString,
            markdown: &QString,
            attachments: &QString,
            delay_seconds: u64,
        );

        #[qinvokable]
        #[cxx_name = "cancelQueuedSend"]
        fn cancel_queued_send(self: &Self, queue_id: &QString);

        #[qinvokable]
        #[cxx_name = "cancelScheduled"]
        fn cancel_scheduled(self: &Self, scheduled_id: &QString);

        #[qinvokable]
        #[cxx_name = "sendScheduledNow"]
        fn send_scheduled_now(self: &Self, scheduled_id: &QString);

        #[qsignal]
        #[cxx_name = "confirmDelete"]
        fn confirm_delete(self: Pin<&mut Self>, token: QString, ids: QString);

        #[qsignal]
        #[cxx_name = "draftSaved"]
        fn draft_saved(self: Pin<&mut Self>, id: QString);

        #[qinvokable]
        #[cxx_name = "resolveDelete"]
        fn resolve_delete(self: &Self, token: &QString, approved: bool);

        #[qinvokable]
        #[cxx_name = "createTag"]
        fn create_tag(self: &Self, name: &QString);

        #[qinvokable]
        #[cxx_name = "deleteTag"]
        fn delete_tag(self: &Self, token: &QString);

        #[qinvokable]
        #[cxx_name = "createServerTemplate"]
        fn create_server_template(self: &Self, name: &QString, category: &QString, content: &QString);

        #[qinvokable]
        #[cxx_name = "deleteServerTemplate"]
        fn delete_server_template(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "createServerSignature"]
        fn create_server_signature(self: &Self, name: &QString, content: &QString);

        #[qinvokable]
        #[cxx_name = "deleteServerSignature"]
        fn delete_server_signature(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "setDefaultSignature"]
        fn set_default_signature(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "assignTag"]
        fn assign_tag(self: &Self, ids: &QString, token: &QString);

        #[qinvokable]
        #[cxx_name = "removeTag"]
        fn remove_tag(self: &Self, ids: &QString, token: &QString);

        #[qinvokable]
        #[cxx_name = "createFolder"]
        fn create_folder(self: &Self, name: &QString, parent: &QString);

        #[qinvokable]
        #[cxx_name = "renameFolder"]
        fn rename_folder(self: &Self, token: &QString, name: &QString);

        #[qinvokable]
        #[cxx_name = "moveFolder"]
        fn move_folder(self: &Self, token: &QString, parent: &QString);

        #[qinvokable]
        #[cxx_name = "deleteFolder"]
        fn delete_folder(self: &Self, token: &QString);

        #[qinvokable]
        #[cxx_name = "setPinned"]
        fn set_pinned(self: &Self, id: &QString, pinned: bool);

        #[qinvokable]
        #[cxx_name = "snoozeMessages"]
        fn snooze_messages(self: &Self, ids: &QString, until: &QString);

        #[qinvokable]
        #[cxx_name = "unsnoozeMessage"]
        fn unsnooze_message(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "saveDraft"]
        fn save_draft(
            self: &Self,
            id: &QString,
            to: &QString,
            cc: &QString,
            bcc: &QString,
            subject: &QString,
            message: &QString,
            attachments_json: &QString,
        );

        #[qinvokable]
        #[cxx_name = "deleteDraft"]
        fn delete_draft(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "bulkUnsubscribe"]
        fn bulk_unsubscribe(self: &Self, ids: &QString);

        #[qinvokable]
        #[cxx_name = "archiveNewsletters"]
        fn archive_newsletters(self: &Self);

        #[qinvokable]
        #[cxx_name = "archiveMailingList"]
        fn archive_mailing_list(self: &Self, list_id: &QString);

        #[qinvokable]
        #[cxx_name = "emptyFolder"]
        fn empty_folder(self: &Self, folder: &QString);

        #[qinvokable]
        #[cxx_name = "batchAction"]
        fn batch_action(self: &Self, action: &QString, ids: &QString);

        #[qinvokable]
        #[cxx_name = "notify"]
        fn notify(self: &Self, title: &QString, body: &QString);

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
        #[cxx_name = "setStarred"]
        fn set_starred(self: &Self, id: &QString, starred: bool);

        #[qinvokable]
        #[cxx_name = "setRead"]
        fn set_read(self: &Self, id: &QString, read: bool);

        #[qinvokable]
        #[cxx_name = "archiveMessage"]
        fn archive_message(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "trashMessage"]
        fn trash_message(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "setSpam"]
        fn set_spam(self: &Self, id: &QString, spam: bool);

        #[qinvokable]
        #[cxx_name = "restoreMessage"]
        fn restore_message(self: &Self, id: &QString);

        #[qinvokable]
        #[cxx_name = "moveMessage"]
        fn move_message(self: &Self, id: &QString, folder: &QString);

        #[qinvokable]
        #[cxx_name = "loadSettingsData"]
        fn load_settings_data(self: &Self);

        #[qinvokable]
        #[cxx_name = "loadOutbox"]
        fn load_outbox(self: &Self);

        #[qinvokable]
        #[cxx_name = "retryOutbox"]
        fn retry_outbox(self: &Self, id: i64);

        #[qinvokable]
        #[cxx_name = "showRecipientKey"]
        fn show_recipient_key(self: &Self, address: &QString);

        #[qinvokable]
        #[cxx_name = "acceptRecipientKey"]
        fn accept_recipient_key(self: &Self, address: &QString, fingerprint: &QString);

        #[qinvokable]
        #[cxx_name = "loadDiagnostics"]
        fn load_diagnostics(self: &Self);

        #[qinvokable]
        #[cxx_name = "repairCache"]
        fn repair_cache(self: &Self);

        #[qinvokable]
        #[cxx_name = "signOut"]
        fn sign_out(self: &Self, keep_cache: bool);

        #[qinvokable]
        #[cxx_name = "createRuleJson"]
        fn create_rule_json(self: &Self, body: &QString);

        #[qinvokable]
        #[cxx_name = "updateRuleJson"]
        fn update_rule_json(self: &Self, id: &QString, body: &QString);

        #[qinvokable]
        #[cxx_name = "reorderRules"]
        fn reorder_rules(self: &Self, ids: &QString);

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
            from: &QString,
            to: &QString,
            cc: &QString,
            bcc: &QString,
            subject: &QString,
            markdown: &QString,
            attachments_json: &QString,
        );

        #[qinvokable]
        #[cxx_name = "sendThreadedMessage"]
        fn send_threaded_message(
            self: &Self,
            from: &QString,
            to: &QString,
            cc: &QString,
            bcc: &QString,
            subject: &QString,
            markdown: &QString,
            attachments_json: &QString,
            in_reply_to: &QString,
            references: &QString,
        );

        #[qinvokable]
        #[cxx_name = "saveAttachment"]
        fn save_attachment(self: &Self, id: &QString, seq: i64);

        #[qinvokable]
        #[cxx_name = "previewAttachment"]
        fn preview_attachment(self: &Self, id: &QString, seq: i64);

        #[qinvokable]
        #[cxx_name = "saveContact"]
        fn save_contact(self: &Self, uid: &QString, vcard: &QString);

        #[qinvokable]
        #[cxx_name = "deleteContact"]
        fn delete_contact(self: &Self, uid: &QString);

        #[qinvokable]
        #[cxx_name = "importGpgKey"]
        fn import_gpg_key(self: &Self);

        #[qinvokable]
        #[cxx_name = "refreshGpgStatus"]
        fn refresh_gpg_status(self: Pin<&mut Self>);

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
    SetStarred(String, bool),
    SetRead(String, bool),
    MoveMessage(String, String),
    TrashMessage(String),
    SetSpam(String, bool),
    RestoreMessage(String),
    LoadSettingsData,
    LoadOutbox,
    RetryOutbox(i64),
    ShowRecipientKey(String),
    AcceptRecipientKey(String, String),
    LoadDiagnostics,
    RepairCache,
    SignOut(bool),
    CreateRule(String),
    UpdateRule { id: String, body: String },
    ReorderRules(Vec<String>),
    DeleteRule(String),
    RunRule(String),
    RouteAlias(String, String),
    CreateMailbox(String),
    SendMessage {
        from: String,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        markdown: String,
        attachments: String,
    },
    SendThreadedMessage {
        from: String,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        markdown: String,
        attachments: String,
        in_reply_to: String,
        references: String,
    },
    Rpc {
        tool: String,
        args: serde_json::Value,
        reply: std::sync::mpsc::Sender<Result<serde_json::Value, String>>,
    },
    PgpPublish(String),
    SaveContact { uid: String, vcard: String },
    DeleteContact(String),
    ImportGpgKey,
    BatchAction { action: String, ids: Vec<String> },
    EmptyFolder(String),
    SaveDraft {
        id: String,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        message: String,
        attachments: String,
    },
    DeleteDraft(String),
    ArchiveMailingList(String),
    SetPinned { id: String, pinned: bool },
    SnoozeMessages { ids: Vec<String>, until: String },
    UnsnoozeMessage(String),
    BulkUnsubscribe(Vec<String>),
    ArchiveNewsletters,
    QueueMessage {
        from: String,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        markdown: String,
        attachments: String,
        delay_seconds: u64,
        schedule: bool,
        at_iso: String,
    },
    CancelQueued(String),
    CancelScheduled(String),
    SendScheduledNow(String),
    CreateTag(String),
    DeleteTag(String),
    AssignTag { ids: Vec<String>, token: String },
    RemoveTag { ids: Vec<String>, token: String },
    CreateServerTemplate { name: String, category: String, content: String },
    DeleteServerTemplate(String),
    CreateServerSignature { name: String, content: String },
    DeleteServerSignature(String),
    SetDefaultSignature(String),
    CreateFolder { name: String, parent: String },
    RenameFolder { token: String, name: String },
    MoveFolder { token: String, parent: String },
    DeleteFolder(String),
    SaveAttachment { id: String, seq: i64 },
    PreviewAttachment { id: String, seq: i64 },
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
    gpg_json: QString,
    thread_json: QString,
    lists_json: QString,
    tags_json: QString,
    templates_json: QString,
    signatures_json: QString,
    outbox_json: QString,
    recipient_key_json: QString,
    diagnostics_json: QString,
    view_counts_json: QString,
    attachment_preview_json: QString,
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
            gpg_json: "{}".into(),
            thread_json: "[]".into(),
            lists_json: "[]".into(),
            tags_json: "[]".into(),
            templates_json: "[]".into(),
            signatures_json: "[]".into(),
            outbox_json: "[]".into(),
            recipient_key_json: "{}".into(),
            diagnostics_json: "{}".into(),
            view_counts_json: "{}".into(),
            attachment_preview_json: "{}".into(),
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
            rt.block_on(push_settings_data(&runtime, &qt));

            while let Ok(command) = rx.recv() {
                let mut snapshot_needed = true;
                match command {
                    Command::Refresh => {}
                    Command::OpenMailbox(next) => {
                        folder = next;
                        if folder == "scheduled" {
                            let _ = rt.block_on(runtime.refresh_scheduled());
                        }
                        selected = first_id(&runtime, &folder);
                    }
                    Command::Select(id) => {
                        let _ = rt.block_on(runtime.ensure_attachments(&id));
                        selected = Some(id);
                    }
                    Command::SetStarred(id, starred) => {
                        let result = rt.block_on(async {
                            runtime
                                .set_starred(std::slice::from_ref(&id), starred)
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, if starred { "Message starred" } else { "Message unstarred" });
                    }
                    Command::SetRead(id, read) => {
                        let result = rt.block_on(async {
                            runtime
                                .set_read(std::slice::from_ref(&id), read)
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, if read { "Marked read" } else { "Marked unread" });
                    }
                    Command::MoveMessage(id, folder) => {
                        let result = rt.block_on(async {
                            runtime
                                .move_messages(std::slice::from_ref(&id), &folder)
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message moved");
                    }
                    Command::TrashMessage(id) => {
                        let result = rt.block_on(async {
                            runtime
                                .trash_messages(std::slice::from_ref(&id))
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message moved to Trash");
                    }
                    Command::SetSpam(id, spam) => {
                        let result = rt.block_on(async {
                            runtime
                                .set_spam(std::slice::from_ref(&id), spam)
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, if spam { "Marked as spam" } else { "Marked as not spam" });
                    }
                    Command::RestoreMessage(id) => {
                        let result = rt.block_on(async {
                            runtime
                                .restore_messages(std::slice::from_ref(&id))
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message restored");
                    }
                    Command::LoadSettingsData => {
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::ShowRecipientKey(address) => {
                        match rt.block_on(runtime.recipient_key_show(&address)) {
                            Ok(value) => {
                                let json = value.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_recipient_key_json(json.into()));
                            }
                            Err(err) => {
                                let message = err.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
                            }
                        }
                        snapshot_needed = false;
                    }
                    Command::AcceptRecipientKey(address, fingerprint) => {
                        let result = rt.block_on(runtime.recipient_key_accept(&address, &fingerprint));
                        notify_result(&qt, result.map(|_| ()).map_err(|e| e.to_string()), "Recipient key accepted");
                        snapshot_needed = false;
                    }
                    Command::SignOut(keep_cache) => {
                        match runtime.sign_out(keep_cache) {
                            Ok(()) => {
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text("Signed out".into()));
                                rt.block_on(runtime.stop());
                                std::process::exit(0);
                            }
                            Err(err) => {
                                let message = err.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
                            }
                        }
                        snapshot_needed = false;
                    }
                    Command::RepairCache => {
                        let result = runtime.repair_cache().map_err(|e| e.to_string());
                        notify_result(&qt, result, "Cache repaired");
                        snapshot_needed = false;
                    }
                    Command::LoadDiagnostics => {
                        match runtime.diagnostics() {
                            Ok(value) => {
                                let json = value.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_diagnostics_json(json.into()));
                            }
                            Err(err) => {
                                let message = err.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
                            }
                        }
                        snapshot_needed = false;
                    }
                    Command::LoadOutbox => {
                        rt.block_on(push_outbox_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::RetryOutbox(id) => {
                        let result = rt.block_on(async {
                            runtime.retry_outbox_message(id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Outbox retry queued");
                        rt.block_on(push_outbox_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::UpdateRule { id, body } => {
                        let result = rt.block_on(async {
                            let value: serde_json::Value = serde_json::from_str(&body)
                                .map_err(|e| format!("invalid rule JSON: {e}"))?;
                            runtime.update_rule(&id, value).await.map(|_| ()).map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Rule updated");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::ReorderRules(ids) => {
                        let result = rt.block_on(async {
                            runtime.reorder_rules(&ids).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Rules reordered");
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
                        from,
                        to,
                        cc,
                        bcc,
                        subject,
                        markdown,
                        attachments,
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
                            let from = from.trim();
                            let attachments: Vec<std::path::PathBuf> =
                                serde_json::from_str::<Vec<String>>(&attachments)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(std::path::PathBuf::from)
                                    .collect();
                            runtime
                                .send_markdown_with_attachments(
                                    if from.is_empty() { None } else { Some(from) },
                                    &parse(to),
                                    &parse(cc),
                                    &parse(bcc),
                                    &subject,
                                    &markdown,
                                    &attachments,
                                )
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message queued for sending");
                        snapshot_needed = false;
                    }
                    Command::SendThreadedMessage {
                        from,
                        to,
                        cc,
                        bcc,
                        subject,
                        markdown,
                        attachments,
                        in_reply_to,
                        references,
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
                            let from = from.trim();
                            let attachments: Vec<std::path::PathBuf> =
                                serde_json::from_str::<Vec<String>>(&attachments)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(std::path::PathBuf::from)
                                    .collect();
                            runtime
                                .send_markdown_with_headers(
                                    if from.is_empty() { None } else { Some(from) },
                                    &parse(to),
                                    &parse(cc),
                                    &parse(bcc),
                                    &subject,
                                    &markdown,
                                    &attachments,
                                    if in_reply_to.is_empty() { None } else { Some(&in_reply_to) },
                                    if references.is_empty() { None } else { Some(&references) },
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
                    Command::PreviewAttachment { id, seq } => {
                        match runtime.attachment_preview(&id, seq) {
                            Ok(value) => {
                                let json = value.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_attachment_preview_json(json.into()));
                            }
                            Err(err) => {
                                let message = err.to_string();
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
                            }
                        }
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
                    Command::CreateServerTemplate { name, category, content } => {
                        let result = rt.block_on(async {
                            runtime.create_server_template(&name, &category, &content).await.map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Template created");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::DeleteServerTemplate(id) => {
                        let result = rt.block_on(async {
                            runtime.delete_server_template(&id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Template deleted");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::CreateServerSignature { name, content } => {
                        let result = rt.block_on(async {
                            runtime.create_server_signature(&name, &content).await.map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Signature created");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::DeleteServerSignature(id) => {
                        let result = rt.block_on(async {
                            runtime.delete_server_signature(&id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Signature deleted");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::SetDefaultSignature(id) => {
                        let result = rt.block_on(async {
                            runtime.set_default_signature(&id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Default signature updated");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::AssignTag { ids, token } => {
                        let result = rt.block_on(async {
                            runtime.assign_tag(&ids, &token).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Tag applied");
                    }
                    Command::RemoveTag { ids, token } => {
                        let result = rt.block_on(async {
                            runtime.remove_tag(&ids, &token).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Tag removed");
                    }
                    Command::CreateTag(name) => {
                        let result = rt.block_on(async {
                            runtime.create_tag(&name).await.map(|_| ()).map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Tag created");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::DeleteTag(token) => {
                        let result = rt.block_on(async {
                            runtime.delete_tag(&token).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Tag deleted");
                        rt.block_on(push_settings_data(&runtime, &qt));
                        snapshot_needed = false;
                    }
                    Command::CreateFolder { name, parent } => {
                        let result = rt.block_on(async {
                            runtime
                                .create_folder(&name, if parent.is_empty() { None } else { Some(&parent) })
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Folder created");
                    }
                    Command::MoveFolder { token, parent } => {
                        let result = rt.block_on(async {
                            runtime
                                .move_folder(&token, if parent.is_empty() { None } else { Some(&parent) })
                                .await
                                .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Folder moved");
                    }
                    Command::RenameFolder { token, name } => {
                        let result = rt.block_on(async {
                            runtime.rename_folder(&token, &name).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Folder renamed");
                        snapshot_needed = true;
                    }
                    Command::DeleteFolder(token) => {
                        let result = rt.block_on(async {
                            runtime.delete_folder(&token).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Folder deleted");
                        snapshot_needed = true;
                    }
                    Command::QueueMessage { from, to, cc, bcc, subject, markdown, attachments, delay_seconds, schedule, at_iso } => {
                        let result = rt.block_on(async {
                            let parse = |value: String| -> Vec<String> {
                                value.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect()
                            };
                            let paths: Vec<std::path::PathBuf> = serde_json::from_str::<Vec<String>>(&attachments)
                                .unwrap_or_default()
                                .into_iter()
                                .map(std::path::PathBuf::from)
                                .collect();
                            if schedule {
                                return runtime
                                    .schedule_markdown_with_attachments(
                                        if from.is_empty() { None } else { Some(&from) },
                                        &parse(to),
                                        &parse(cc),
                                        &parse(bcc),
                                        &subject,
                                        &markdown,
                                        &paths,
                                        &at_iso,
                                    )
                                    .await
                                    .map_err(|e| e.to_string());
                            }
                            runtime
                                .queue_markdown_with_attachments(
                                    if from.is_empty() { None } else { Some(&from) },
                                    &parse(to),
                                    &parse(cc),
                                    &parse(bcc),
                                    &subject,
                                    &markdown,
                                    &paths,
                                    delay_seconds,
                                )
                                .await
                                .map_err(|e| e.to_string())
                        });
                        match result {
                            Ok(id) => {
                                if schedule {
                                    let subject_copy = subject.clone();
                                    let _ = qt.queue(move |mut q| {
                                        q.as_mut().scheduled_queued(id.into(), at_iso.into(), subject_copy.into());
                                    });
                                } else {
                                    let _ = qt.queue(move |mut q| q.as_mut().queue_ready(id.into()));
                                }
                            }
                            Err(err) => {
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(err.into()));
                            }
                        }
                        snapshot_needed = false;
                    }
                    Command::CancelQueued(queue_id) => {
                        let result = rt.block_on(async {
                            runtime.cancel_queued_send(&queue_id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Queued send cancelled");
                        snapshot_needed = false;
                    }
                    Command::CancelScheduled(scheduled_id) => {
                        let result = rt.block_on(async {
                            runtime.cancel_scheduled(&scheduled_id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Scheduled send cancelled");
                        snapshot_needed = true;
                    }
                    Command::SendScheduledNow(scheduled_id) => {
                        let result = rt.block_on(async {
                            runtime.send_scheduled_now(&scheduled_id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Scheduled send released");
                        snapshot_needed = true;
                    }
                    Command::BulkUnsubscribe(ids) => {
                        let result = rt.block_on(async {
                            runtime.unsubscribe_messages(&ids).await.map(|report| {
                                serde_json::to_string(&report).unwrap_or_default()
                            }).map_err(|e| e.to_string())
                        });
                        match result {
                            Ok(json) => {
                                let message = format!("Unsubscribe: {json}");
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(message.into()));
                            }
                            Err(err) => {
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(err.into()));
                            }
                        }
                        snapshot_needed = false;
                    }
                    Command::ArchiveNewsletters => {
                        let result = rt.block_on(async {
                            runtime.archive_newsletters().await.map(|_| ()).map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Newsletters archived");
                        rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
                        snapshot_needed = false;
                    }
                    Command::SnoozeMessages { ids, until } => {
                        let result = rt.block_on(async {
                            runtime.snooze_messages(&ids, &until).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message snoozed");
                    }
                    Command::UnsnoozeMessage(id) => {
                        let result = rt.block_on(async {
                            runtime.unsnooze_message(&id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Message unsnoozed");
                    }
                    Command::SetPinned { id, pinned } => {
                        let result = rt.block_on(async {
                            runtime.set_pinned(&[id.clone()], pinned).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, if pinned { "Pinned" } else { "Unpinned" });
                        rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
                        snapshot_needed = false;
                    }
                    Command::ArchiveMailingList(list_id) => {
                        let result = rt.block_on(async {
                            runtime.archive_mailing_list(&list_id).await.map(|_| ()).map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Mailing list archived");
                        rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
                        snapshot_needed = false;
                    }
                    Command::SaveDraft { id, to, cc, bcc, subject, message, attachments } => {
                        let result = rt.block_on(async {
                            let parse = |value: String| -> Vec<String> {
                                value.split(',').map(str::trim).filter(|part| !part.is_empty()).map(str::to_string).collect()
                            };
                            let attachments: Vec<std::path::PathBuf> =
                                serde_json::from_str::<Vec<String>>(&attachments)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(std::path::PathBuf::from)
                                    .collect();
                            runtime
                                .save_draft(
                                    if id.is_empty() { None } else { Some(&id) },
                                    &parse(to),
                                    &parse(cc),
                                    &parse(bcc),
                                    &subject,
                                    &message,
                                    &attachments,
                                )
                                .await
                                .map_err(|e| e.to_string())
                        });
                        match result {
                            Ok(id) => {
                                let _ = qt.queue(move |mut q| q.as_mut().draft_saved(id.into()));
                            }
                            Err(err) => {
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(err.into()));
                            }
                        }
                        snapshot_needed = false;
                    }
                    Command::DeleteDraft(id) => {
                        let result = rt.block_on(async {
                            runtime.delete_draft(&id).await.map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Draft deleted");
                        snapshot_needed = false;
                    }
                    Command::EmptyFolder(folder_label) => {
                        let result = rt.block_on(async {
                            runtime.empty_folder(&folder_label).await.map(|_| ()).map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Folder emptied");
                        rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
                        snapshot_needed = false;
                    }
                    Command::BatchAction { action, ids } => {
                        let result = rt.block_on(async {
                            match action.as_str() {
                                "mark_read" => runtime.set_read(&ids, true).await,
                                "mark_unread" => runtime.set_read(&ids, false).await,
                                "star" => runtime.set_starred(&ids, true).await,
                                "unstar" => runtime.set_starred(&ids, false).await,
                                "archive" => runtime.move_messages(&ids, "archive").await,
                                "trash" => runtime.trash_messages(&ids).await,
                                "delete" => runtime.trash_messages(&ids).await,
                                "spam" => runtime.set_spam(&ids, true).await,
                                "not_spam" => runtime.set_spam(&ids, false).await,
                                "restore" => runtime.restore_messages(&ids).await,
                                other if other.starts_with("move:") => {
                                    runtime.move_messages(&ids, &other[5..]).await
                                }
                                _ => Err(aster_client_core::ClientError::Protocol(
                                    format!("unknown batch action: {action}"),
                                )),
                            }
                            .map_err(|e| e.to_string())
                        });
                        notify_result(&qt, result, "Batch action applied");
                        rt.block_on(snapshot(&runtime, &folder, selected.as_deref(), &qt));
                        snapshot_needed = false;
                    }
                    Command::ImportGpgKey => {
                        let result = rt.block_on(async {
                            runtime
                                .import_pgp_to_gpg_agent()
                                .await
                                .map(|status| serde_json::to_string(&status).unwrap_or_default())
                        });
                        match result {
                            Ok(json) => {
                                let _ = qt.queue(move |mut q| {
                                    q.as_mut().set_gpg_json(json.into());
                                    q.as_mut().set_toast_text("Aster key imported into gpg-agent".into());
                                });
                            }
                            Err(err) => {
                                let _ = qt.queue(move |mut q| q.as_mut().set_toast_text(err.to_string().into()));
                            }
                        }
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

    pub fn set_starred(self: &Self, id: &QString, starred: bool) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SetStarred(id.to_string(), starred));
        }
    }

    pub fn set_read(self: &Self, id: &QString, read: bool) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SetRead(id.to_string(), read));
        }
    }

    pub fn archive_message(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::MoveMessage(id.to_string(), "archive".to_string()));
        }
    }

    pub fn trash_message(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::TrashMessage(id.to_string()));
        }
    }

    pub fn set_spam(self: &Self, id: &QString, spam: bool) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SetSpam(id.to_string(), spam));
        }
    }

    pub fn restore_message(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::RestoreMessage(id.to_string()));
        }
    }

    pub fn move_message(self: &Self, id: &QString, folder: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::MoveMessage(id.to_string(), folder.to_string()));
        }
    }

    pub fn load_settings_data(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::LoadSettingsData);
        }
    }

    pub fn load_outbox(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::LoadOutbox);
        }
    }

    pub fn retry_outbox(self: &Self, id: i64) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::RetryOutbox(id));
        }
    }

    pub fn show_recipient_key(self: &Self, address: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::ShowRecipientKey(address.to_string()));
        }
    }

    pub fn accept_recipient_key(self: &Self, address: &QString, fingerprint: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::AcceptRecipientKey(address.to_string(), fingerprint.to_string()));
        }
    }

    pub fn load_diagnostics(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::LoadDiagnostics);
        }
    }

    pub fn repair_cache(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::RepairCache);
        }
    }

    pub fn sign_out(self: &Self, keep_cache: bool) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SignOut(keep_cache));
        }
    }

    pub fn create_rule_json(self: &Self, body: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CreateRule(body.to_string()));
        }
    }

    pub fn update_rule_json(self: &Self, id: &QString, body: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::UpdateRule { id: id.to_string(), body: body.to_string() });
        }
    }

    pub fn reorder_rules(self: &Self, ids: &QString) {
        if let Some(tx) = &self.tx {
            let ids: Vec<String> = serde_json::from_str(&ids.to_string()).unwrap_or_default();
            let _ = tx.send(Command::ReorderRules(ids));
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
        from: &QString,
        to: &QString,
        cc: &QString,
        bcc: &QString,
        subject: &QString,
        markdown: &QString,
        attachments_json: &QString,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SendMessage {
                from: from.to_string(),
                to: to.to_string(),
                cc: cc.to_string(),
                bcc: bcc.to_string(),
                subject: subject.to_string(),
                markdown: markdown.to_string(),
                attachments: attachments_json.to_string(),
            });
        }
    }

    pub fn create_tag(self: &Self, name: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::CreateTag(name.to_string())); }
    }

    pub fn delete_tag(self: &Self, token: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::DeleteTag(token.to_string())); }
    }

    pub fn assign_tag(self: &Self, ids: &QString, token: &QString) {
        if let Some(tx) = &self.tx {
            let ids: Vec<String> = serde_json::from_str(&ids.to_string()).unwrap_or_default();
            let _ = tx.send(Command::AssignTag { ids, token: token.to_string() });
        }
    }

    pub fn remove_tag(self: &Self, ids: &QString, token: &QString) {
        if let Some(tx) = &self.tx {
            let ids: Vec<String> = serde_json::from_str(&ids.to_string()).unwrap_or_default();
            let _ = tx.send(Command::RemoveTag { ids, token: token.to_string() });
        }
    }

    pub fn create_folder(self: &Self, name: &QString, parent: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CreateFolder { name: name.to_string(), parent: parent.to_string() });
        }
    }

    pub fn create_server_template(self: &Self, name: &QString, category: &QString, content: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CreateServerTemplate {
                name: name.to_string(),
                category: category.to_string(),
                content: content.to_string(),
            });
        }
    }

    pub fn delete_server_template(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::DeleteServerTemplate(id.to_string())); }
    }

    pub fn create_server_signature(self: &Self, name: &QString, content: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CreateServerSignature { name: name.to_string(), content: content.to_string() });
        }
    }

    pub fn delete_server_signature(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::DeleteServerSignature(id.to_string())); }
    }

    pub fn set_default_signature(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::SetDefaultSignature(id.to_string())); }
    }

    pub fn rename_folder(self: &Self, token: &QString, name: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::RenameFolder { token: token.to_string(), name: name.to_string() });
        }
    }

    pub fn move_folder(self: &Self, token: &QString, parent: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::MoveFolder { token: token.to_string(), parent: parent.to_string() });
        }
    }

    pub fn delete_folder(self: &Self, token: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::DeleteFolder(token.to_string())); }
    }

    pub fn send_queued(
        self: &Self,
        from: &QString,
        to: &QString,
        cc: &QString,
        bcc: &QString,
        subject: &QString,
        markdown: &QString,
        attachments: &QString,
        delay_seconds: u64,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::QueueMessage {
                from: from.to_string(),
                to: to.to_string(),
                cc: cc.to_string(),
                bcc: bcc.to_string(),
                subject: subject.to_string(),
                markdown: markdown.to_string(),
                attachments: attachments.to_string(),
                delay_seconds,
                schedule: false,
                at_iso: String::new(),
            });
        }
    }

    pub fn schedule_message(
        self: &Self,
        from: &QString,
        to: &QString,
        cc: &QString,
        bcc: &QString,
        subject: &QString,
        markdown: &QString,
        attachments: &QString,
        delay_seconds: u64,
        at_iso: &QString,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::QueueMessage {
                from: from.to_string(),
                to: to.to_string(),
                cc: cc.to_string(),
                bcc: bcc.to_string(),
                subject: subject.to_string(),
                markdown: markdown.to_string(),
                attachments: attachments.to_string(),
                delay_seconds,
                schedule: true,
                at_iso: at_iso.to_string(),
            });
        }
    }

    pub fn cancel_queued_send(self: &Self, queue_id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CancelQueued(queue_id.to_string()));
        }
    }

    pub fn cancel_scheduled(self: &Self, scheduled_id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::CancelScheduled(scheduled_id.to_string()));
        }
    }

    pub fn send_scheduled_now(self: &Self, scheduled_id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SendScheduledNow(scheduled_id.to_string()));
        }
    }

    pub fn bulk_unsubscribe(self: &Self, ids: &QString) {
        if let Some(tx) = &self.tx {
            let ids: Vec<String> = serde_json::from_str(&ids.to_string()).unwrap_or_default();
            let _ = tx.send(Command::BulkUnsubscribe(ids));
        }
    }

    pub fn archive_newsletters(self: &Self) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::ArchiveNewsletters); }
    }

    pub fn set_pinned(self: &Self, id: &QString, pinned: bool) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SetPinned { id: id.to_string(), pinned });
        }
    }

    pub fn snooze_messages(self: &Self, ids: &QString, until: &QString) {
        if let Some(tx) = &self.tx {
            let ids: Vec<String> = serde_json::from_str(&ids.to_string()).unwrap_or_default();
            let _ = tx.send(Command::SnoozeMessages { ids, until: until.to_string() });
        }
    }

    pub fn unsnooze_message(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::UnsnoozeMessage(id.to_string()));
        }
    }

    pub fn archive_mailing_list(self: &Self, list_id: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::ArchiveMailingList(list_id.to_string())); }
    }

    pub fn save_draft(
        self: &Self,
        id: &QString,
        to: &QString,
        cc: &QString,
        bcc: &QString,
        subject: &QString,
        message: &QString,
        attachments_json: &QString,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SaveDraft {
                id: id.to_string(),
                to: to.to_string(),
                cc: cc.to_string(),
                bcc: bcc.to_string(),
                subject: subject.to_string(),
                message: message.to_string(),
                attachments: attachments_json.to_string(),
            });
        }
    }

    pub fn delete_draft(self: &Self, id: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::DeleteDraft(id.to_string())); }
    }

    pub fn empty_folder(self: &Self, folder: &QString) {
        if let Some(tx) = &self.tx { let _ = tx.send(Command::EmptyFolder(folder.to_string())); }
    }

    pub fn batch_action(self: &Self, action: &QString, ids: &QString) {
        if let Some(tx) = &self.tx {
            let ids: Vec<String> = serde_json::from_str(&ids.to_string()).unwrap_or_default();
            let _ = tx.send(Command::BatchAction {
                action: action.to_string(),
                ids,
            });
        }
    }

    pub fn notify(self: &Self, title: &QString, body: &QString) {
        let _ = std::process::Command::new("notify-send")
            .arg(title.to_string())
            .arg(body.to_string())
            .spawn();
    }

    pub fn import_gpg_key(self: &Self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::ImportGpgKey);
        }
    }

    pub fn refresh_gpg_status(mut self: Pin<&mut Self>) {
        let status = aster_client_core::pgp_agent::detect();
        let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
        self.as_mut().set_gpg_json(json.into());
    }

    pub fn preview_attachment(self: &Self, id: &QString, seq: i64) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::PreviewAttachment { id: id.to_string(), seq });
        }
    }

    pub fn send_threaded_message(
        self: &Self,
        from: &QString,
        to: &QString,
        cc: &QString,
        bcc: &QString,
        subject: &QString,
        markdown: &QString,
        attachments_json: &QString,
        in_reply_to: &QString,
        references: &QString,
    ) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::SendThreadedMessage {
                from: from.to_string(),
                to: to.to_string(),
                cc: cc.to_string(),
                bcc: bcc.to_string(),
                subject: subject.to_string(),
                markdown: markdown.to_string(),
                attachments: attachments_json.to_string(),
                in_reply_to: in_reply_to.to_string(),
                references: references.to_string(),
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
    if folder == "scheduled" {
        let _ = runtime.refresh_scheduled().await;
    }
    let mailboxes = runtime
        .mailboxes()
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let messages = runtime
        .messages(folder, 100, 0)
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let detail_value = selected.and_then(|id| runtime.message(id).ok());
    let thread = detail_value
        .as_ref()
        .and_then(|detail| detail.message.thread_id.clone())
        .and_then(|thread_id| runtime.thread_messages(&thread_id, 50).ok())
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|| "[]".into());
    let detail = detail_value
        .and_then(|detail| serde_json::to_string(&detail).ok())
        .unwrap_or_else(|| "{}".into());
    let lists = runtime
        .mailing_lists()
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let view_counts = runtime
        .view_counts()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "{}".into());
    let status = runtime.status().await;

    let folder = folder.to_string();
    let _ = qt.queue(move |mut q| {
        q.as_mut().set_account_email(status.default_sender.into());
        q.as_mut().set_mailboxes_json(mailboxes.into());
        q.as_mut().set_messages_json(messages.into());
        q.as_mut().set_selected_json(detail.into());
        q.as_mut().set_thread_json(thread.into());
        q.as_mut().set_lists_json(lists.into());
        q.as_mut().set_view_counts_json(view_counts.into());
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


async fn push_outbox_data(
    runtime: &ClientRuntime,
    qt: &cxx_qt::CxxQtThread<qobject::AppController>,
) {
    let outbox = runtime
        .outbox_messages()
        .map(|rows| serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let _ = qt.queue(move |mut q| {
        q.as_mut().set_outbox_json(outbox.into());
    });
}

async fn push_settings_data(
    runtime: &ClientRuntime,
    qt: &cxx_qt::CxxQtThread<qobject::AppController>,
) {
    let identities = serde_json::to_string(&runtime.send_identities().await)
        .unwrap_or_else(|_| "[]".into());
    let _ = qt.queue(move |mut q| q.as_mut().set_identities_json(identities.into()));
    let rules = runtime
        .rules()
        .await
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "{}".into());
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
    let gpg = serde_json::to_string(&runtime.gpg_status()).unwrap_or_else(|_| "{}".into());
    let tags = runtime
        .tags()
        .map(|value| serde_json::to_string(&value).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let templates = runtime
        .server_templates()
        .await
        .map(|value| serde_json::to_string(&value).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let signatures = runtime
        .server_signatures()
        .await
        .map(|value| serde_json::to_string(&value).unwrap_or_else(|_| "[]".into()))
        .unwrap_or_else(|_| "[]".into());
    let _ = qt.queue(move |mut q| {
        q.as_mut().set_rules_json(rules.into());
        q.as_mut().set_pgp_json(pgp.into());
        q.as_mut().set_contacts_json(contacts.into());
        q.as_mut().set_gpg_json(gpg.into());
        q.as_mut().set_tags_json(tags.into());
        q.as_mut().set_templates_json(templates.into());
        q.as_mut().set_signatures_json(signatures.into());
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
            if folder == "scheduled" {
                runtime.refresh_scheduled().await.map_err(|e| e.to_string())?;
            }
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
        "mail_bulk_unsubscribe" => {
            let ids = args.get("ids").and_then(|v| v.as_array()).map(|items| items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
            Ok(serde_json::to_value(runtime.unsubscribe_messages(&ids).await.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?)
        }
        "mail_archive_newsletters" => {
            Ok(serde_json::json!({"archived": runtime.archive_newsletters().await.map_err(|e| e.to_string())?}))
        }
        "mail_unsubscribe_info" => {
            let id = args.get("id").and_then(|v| v.as_str()).ok_or("id is required")?;
            let detail = runtime.message(id).map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "list_id": detail.message.list_id,
                "list_name": detail.message.list_name,
                "list_unsubscribe": detail.message.list_unsubscribe,
                "list_unsubscribe_post": detail.message.list_unsubscribe_post
            }))
        }
        "mailing_list_archive" => {
            let list_id = args.get("list_id").and_then(|v| v.as_str()).ok_or("list_id is required")?;
            Ok(serde_json::json!({"archived": runtime.archive_mailing_list(list_id).await.map_err(|e| e.to_string())?}))
        }
        "mail_pin" => {
            let pinned = args
                .get("pinned")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            runtime.set_pinned(&ids(), pinned).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "mail_from_sender_list" => {
            let sender = args.get("sender").and_then(|v| v.as_str()).ok_or("sender is required")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            Ok(serde_json::to_value(runtime.messages_from_sender(sender, limit).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?)
        }
        "mail_from_sender_archive" => {
            let sender = args.get("sender").and_then(|v| v.as_str()).ok_or("sender is required")?;
            let rows = runtime.messages_from_sender(sender, 1000).map_err(|e| e.to_string())?;
            let ids: Vec<String> = rows.into_iter().map(|m| m.id).collect();
            runtime.move_messages(&ids, "archive").await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "count": ids.len() }))
        }
        "mail_from_sender_trash" => {
            let sender = args.get("sender").and_then(|v| v.as_str()).ok_or("sender is required")?;
            let rows = runtime.messages_from_sender(sender, 1000).map_err(|e| e.to_string())?;
            let ids: Vec<String> = rows.into_iter().map(|m| m.id).collect();
            runtime.trash_messages(&ids).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "count": ids.len() }))
        }
        "mail_from_sender_move" => {
            let sender = args.get("sender").and_then(|v| v.as_str()).ok_or("sender is required")?;
            let folder = args.get("folder_token").and_then(|v| v.as_str()).ok_or("folder_token is required")?;
            let rows = runtime.messages_from_sender(sender, 1000).map_err(|e| e.to_string())?;
            let ids: Vec<String> = rows.into_iter().map(|m| m.id).collect();
            runtime.move_messages(&ids, folder).await.map_err(|e| e.to_string())?;
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
            let subject = args.get("subject").and_then(|value| value.as_str()).unwrap_or("");
            let markdown = args.get("markdown").and_then(|value| value.as_str()).unwrap_or("");
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
            runtime.cancel_scheduled(id).await.map_err(|e| e.to_string())?;
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
        "rules_update" => {
            let id = args.get("id").and_then(|v| v.as_str()).ok_or("id is required")?;
            let rule = args.get("rule").cloned().ok_or("rule is required")?;
            runtime.update_rule(id, rule).await.map_err(|e| e.to_string())?;
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
            runtime.reorder_rules(&order).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "draft_save" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let parse = |name: &str| -> Vec<String> {
                args.get(name)
                    .and_then(|v| v.as_str())
                    .map(|value| value.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect())
                    .unwrap_or_default()
            };
            let attachments: Vec<std::path::PathBuf> = args
                .get("attachments")
                .and_then(|v| v.as_array())
                .map(|items| items.iter().filter_map(|v| v.as_str()).map(std::path::PathBuf::from).collect())
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
            .recipient_key_show(args.get("address").and_then(|v| v.as_str()).ok_or("address is required")?)
            .await
            .map_err(|e| e.to_string()),
        "recipient_key_accept" => {
            let address = args.get("address").and_then(|v| v.as_str()).ok_or("address is required")?;
            let fingerprint = args.get("fingerprint").and_then(|v| v.as_str()).ok_or("fingerprint is required")?;
            runtime
                .recipient_key_accept(address, fingerprint)
                .await
                .map_err(|e| e.to_string())
        }
        "folder_create" => {
            let name = args.get("name").and_then(|v| v.as_str()).ok_or("name is required")?;
            let parent = args.get("parent_token").and_then(|v| v.as_str());
            let token = runtime.create_folder(name, parent).await.map_err(|e| e.to_string())?;
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
            runtime.server_templates().await.map_err(|e| e.to_string())?,
        ).map_err(|e| e.to_string())?),
        "template_create" => {
            let name = args.get("name").and_then(|v| v.as_str()).ok_or("name is required")?;
            let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).ok_or("content is required")?;
            Ok(serde_json::json!({"id": runtime.create_server_template(name, category, content).await.map_err(|e| e.to_string())?}))
        }
        "template_delete" => {
            let id = args.get("id").and_then(|v| v.as_str()).ok_or("id is required")?;
            runtime.delete_server_template(id).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"ok": true}))
        }
        "signatures_list" => Ok(serde_json::to_value(
            runtime.server_signatures().await.map_err(|e| e.to_string())?,
        ).map_err(|e| e.to_string())?),
        "signature_create" => {
            let name = args.get("name").and_then(|v| v.as_str()).ok_or("name is required")?;
            let content = args.get("content").and_then(|v| v.as_str()).ok_or("content is required")?;
            Ok(serde_json::json!({"id": runtime.create_server_signature(name, content).await.map_err(|e| e.to_string())?}))
        }
        "signature_delete" => {
            let id = args.get("id").and_then(|v| v.as_str()).ok_or("id is required")?;
            runtime.delete_server_signature(id).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"ok": true}))
        }
        "signature_set_default" => {
            let id = args.get("id").and_then(|v| v.as_str()).ok_or("id is required")?;
            runtime.set_default_signature(id).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"ok": true}))
        }
        "tags_list" => Ok(serde_json::to_value(
            runtime.tags().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
        "tag_create" => {
            let name = args
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or("name is required")?;
            Ok(serde_json::to_value(
                runtime.create_tag(name).await.map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?)
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
        "gpg_status" => Ok(serde_json::to_value(runtime.gpg_status())
            .map_err(|e| e.to_string())?),
        "gpg_import_key" => Ok(serde_json::to_value(
            runtime
                .import_pgp_to_gpg_agent()
                .await
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?),
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
        "mail_send_queued" => {
            let to = args.get("to").and_then(|v| v.as_str()).ok_or("to is required")?;
            let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            let markdown = args.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
            let delay = args.get("delay_seconds").and_then(|v| v.as_u64()).unwrap_or(10);
            let parse = |value: String| -> Vec<String> {
                value.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect()
            };
            let id = runtime
                .queue_markdown_with_attachments(None, &parse(to.to_string()), &[], &[], subject, markdown, &[], delay)
                .await
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "queue_id": id }))
        }
        "mail_cancel_queued" => {
            let id = args.get("queue_id").and_then(|v| v.as_str()).ok_or("queue_id is required")?;
            runtime.cancel_queued_send(id).await.map_err(|e| e.to_string())?;
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
