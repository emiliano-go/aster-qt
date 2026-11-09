// SPDX-License-Identifier: AGPL-3.0-or-later
//! Actions the UI sends to the worker thread.

use std::path::PathBuf;

pub struct Outgoing {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub markdown: String,
    pub attachments: Vec<PathBuf>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub expires_at: Option<String>,
    pub obscure_subject: bool,
}

pub enum Action {
    SetFolder(String),
    Select(String),
    SetRead {
        ids: Vec<String>,
        read: bool,
    },
    SetPinned {
        ids: Vec<String>,
        pinned: bool,
    },
    SetSpam {
        ids: Vec<String>,
        spam: bool,
    },
    RestoreMessages(Vec<String>),
    SetStarred {
        ids: Vec<String>,
        starred: bool,
    },
    Archive(Vec<String>),
    Trash(Vec<String>),
    Refresh,
    SyncNow,
    LoadRemoteContent(String),
    Print(String),
    SaveAttachment {
        id: String,
        seq: i64,
    },
    SaveAllAttachments(String),
    Send(Outgoing),
    Schedule(Outgoing, String),
    Snooze {
        ids: Vec<String>,
        until: String,
    },
    UnsnoozeMessage(String),
    DeletePermanent(Vec<String>),
    PreviewAttachment {
        id: String,
        seq: i64,
    },
    LoadOutbox,
    ArchiveMailingList(String),
    SetAliasEnabled {
        id: String,
        enabled: bool,
    },
    ToggleAliasPin(String),
    RenameFolder {
        token: String,
        name: String,
    },
    MoveFolder {
        token: String,
        parent: Option<String>,
    },
    ReorderRules(Vec<String>),
    RetryOutbox(i64),
    CancelScheduled(String),
    SendScheduledNow(String),
    ResolveDelete {
        token: String,
        approved: bool,
    },
    Rpc {
        tool: String,
        args: serde_json::Value,
        reply: std::sync::mpsc::Sender<Result<serde_json::Value, String>>,
    },
    LoadSettingsData,
    LoadDiagnostics,
    CreateRule(serde_json::Value),
    DeleteRule(String),
    RunRule(String),
    CreateAlias {
        local: String,
        domain: String,
        display: Option<String>,
    },
    DeleteAlias(String),
    RouteAlias {
        alias: String,
        mailbox: String,
    },
    CreateMailbox(String),
    CreateFolder {
        name: String,
        parent: Option<String>,
    },
    DeleteFolder(String),
    SaveContact {
        uid: String,
        vcard: String,
    },
    DeleteContact(String),
    ImportGpgKey,
    PublishPgp(String),
    RepairCache,
    SignOut(bool),
}
