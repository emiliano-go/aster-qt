// SPDX-License-Identifier: AGPL-3.0-or-later
//! The single UI state, projected from the service by the worker.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use aster_client_core::service::{
    MailboxView, MessageDetail, MessageView, SendIdentityView, SignatureView, TemplateView,
};
use aster_client_core::settings;
use rune::{PointerPress, TextEdit};

use crate::action::{Action, Outgoing};
use crate::theme::ThemeState;

pub struct Compose {
    pub requested: bool,
    pub from: TextEdit,
    pub to: TextEdit,
    pub cc: TextEdit,
    pub bcc: TextEdit,
    pub subject: TextEdit,
    pub body: TextEdit,
    pub schedule_at: TextEdit,
    pub expires_at: TextEdit,
    pub obscure_subject: bool,
    pub attachments: Vec<PathBuf>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub heading: String,
}

impl Default for Compose {
    fn default() -> Self {
        Self {
            requested: false,
            from: TextEdit::new(""),
            to: TextEdit::new(""),
            cc: TextEdit::new(""),
            bcc: TextEdit::new(""),
            subject: TextEdit::new(""),
            body: TextEdit::new(""),
            schedule_at: TextEdit::new(""),
            expires_at: TextEdit::new(""),
            obscure_subject: false,
            attachments: Vec::new(),
            in_reply_to: None,
            references: None,
            heading: "New message".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Mail,
    Rules,
    Aliases,
    Contacts,
    Security,
    Mcp,
    Diagnostics,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 8] = [
        SettingsTab::Appearance,
        SettingsTab::Mail,
        SettingsTab::Rules,
        SettingsTab::Aliases,
        SettingsTab::Contacts,
        SettingsTab::Security,
        SettingsTab::Mcp,
        SettingsTab::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Mail => "Mail",
            SettingsTab::Rules => "Rules & filters",
            SettingsTab::Aliases => "Aliases & mailboxes",
            SettingsTab::Contacts => "Contacts",
            SettingsTab::Security => "Security & PGP",
            SettingsTab::Mcp => "MCP",
            SettingsTab::Diagnostics => "Diagnostics",
        }
    }
}

pub struct Row {
    pub message: MessageView,
    pub thread_count: usize,
}

pub struct Filters {
    pub from: TextEdit,
    pub subject: TextEdit,
    pub body: TextEdit,
    pub since: TextEdit,
    pub until: TextEdit,
    pub name: TextEdit,
    pub starred: bool,
    pub attachments: bool,
    pub unread_only: bool,
    pub category: TextEdit,
    pub tag: TextEdit,
    pub list_id: TextEdit,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            from: TextEdit::new(""),
            subject: TextEdit::new(""),
            body: TextEdit::new(""),
            since: TextEdit::new(""),
            until: TextEdit::new(""),
            name: TextEdit::new(""),
            starred: false,
            attachments: false,
            unread_only: false,
            category: TextEdit::new(""),
            tag: TextEdit::new(""),
            list_id: TextEdit::new(""),
        }
    }
}

pub struct Forms {
    pub rule_name: TextEdit,
    pub rule_value: TextEdit,
    pub rule_action: TextEdit,
    pub alias_local: TextEdit,
    pub alias_domain: TextEdit,
    pub alias_display: TextEdit,
    pub alias_route: TextEdit,
    pub mailbox_name: TextEdit,
    pub folder_name: TextEdit,
    pub folder_parent: TextEdit,
    pub folder_token: TextEdit,
    pub folder_rename: TextEdit,
    pub folder_move_parent: TextEdit,
    pub contact_name: TextEdit,
    pub contact_email: TextEdit,
}

impl Default for Forms {
    fn default() -> Self {
        Self {
            rule_name: TextEdit::new(""),
            rule_value: TextEdit::new(""),
            rule_action: TextEdit::new(""),
            alias_local: TextEdit::new(""),
            alias_domain: TextEdit::new(""),
            alias_display: TextEdit::new(""),
            alias_route: TextEdit::new(""),
            mailbox_name: TextEdit::new(""),
            folder_name: TextEdit::new(""),
            folder_parent: TextEdit::new(""),
            folder_token: TextEdit::new(""),
            folder_rename: TextEdit::new(""),
            folder_move_parent: TextEdit::new(""),
            contact_name: TextEdit::new(""),
            contact_email: TextEdit::new(""),
        }
    }
}

pub struct AppState {
    pub actions: Option<Sender<Action>>,
    pub reader_open: bool,
    pub sidebar_width: f32,
    pub reader_width: f32,
    pub selection_mode: bool,
    pub email_dark: bool,
    pub folder: String,
    pub account: String,
    pub status: String,
    pub mailboxes: Vec<MailboxView>,
    pub counts: serde_json::Value,
    pub messages: Vec<MessageView>,
    pub visible: Vec<Row>,
    pub query: TextEdit,
    pub filters: Filters,
    pub filters_open: bool,
    pub selected: Option<String>,
    pub detail: Option<MessageDetail>,
    pub thread: Vec<MessageView>,
    pub identities: Vec<SendIdentityView>,
    pub templates: Vec<TemplateView>,
    pub signatures: Vec<SignatureView>,
    pub compose: Compose,
    pub selection: BTreeSet<String>,
    pub cursor: usize,
    pub toast: Option<String>,
    pub toast_at: Option<std::time::Instant>,
    pub images: Option<rune::ImageCache>,
    pub settings: serde_json::Value,
    pub data_dir: PathBuf,
    pub theme: ThemeState,
    pub settings_open: bool,
    pub settings_tab: SettingsTab,
    pub forms: Forms,
    pub rules: serde_json::Value,
    pub aliases: serde_json::Value,
    pub contacts: serde_json::Value,
    pub lists: serde_json::Value,
    pub tags: serde_json::Value,
    pub pgp: serde_json::Value,
    pub gpg: serde_json::Value,
    pub diagnostics: serde_json::Value,
    pub confirm_sign_out: bool,
    pub outbox: serde_json::Value,
    pub outbox_open: bool,
    pub outbox_pending: usize,
    pub menu_point: Option<PointerPress>,
    pub menu_message: Option<String>,
    pub command_open: bool,
    pub command_filter: TextEdit,
    pub snooze_open: bool,
    pub snooze_targets: Vec<String>,
    pub snooze_at: TextEdit,
    pub delete_open: bool,
    pub delete_targets: Vec<String>,
    pub delete_token: Option<String>,
    pub delete_word: TextEdit,
    pub preview_open: bool,
    pub preview_text: String,
}

impl Default for AppState {
    fn default() -> Self {
        let data_dir = aster_client_core::ClientConfig::default().data_dir;
        let settings = settings::load(&data_dir);
        let theme = ThemeState::load(&data_dir);
        Self {
            actions: None,
            reader_open: true,
            sidebar_width: 240.0,
            reader_width: 380.0,
            selection_mode: false,
            email_dark: false,
            folder: "inbox".into(),
            account: String::new(),
            status: "Starting...".into(),
            mailboxes: Vec::new(),
            counts: serde_json::json!({}),
            messages: Vec::new(),
            visible: Vec::new(),
            query: TextEdit::new(""),
            filters: Filters::default(),
            filters_open: false,
            selected: None,
            detail: None,
            thread: Vec::new(),
            identities: Vec::new(),
            templates: Vec::new(),
            signatures: Vec::new(),
            compose: Compose::default(),
            selection: BTreeSet::new(),
            cursor: 0,
            toast: None,
            toast_at: None,
            images: None,
            settings,
            data_dir,
            theme,
            settings_open: false,
            settings_tab: SettingsTab::Appearance,
            forms: Forms::default(),
            rules: serde_json::json!({}),
            aliases: serde_json::json!([]),
            contacts: serde_json::json!([]),
            lists: serde_json::json!([]),
            tags: serde_json::json!([]),
            pgp: serde_json::json!({}),
            gpg: serde_json::json!({}),
            diagnostics: serde_json::json!({}),
            confirm_sign_out: false,
            outbox: serde_json::json!([]),
            outbox_open: false,
            outbox_pending: 0,
            menu_point: None,
            menu_message: None,
            command_open: false,
            command_filter: TextEdit::new(""),
            snooze_open: false,
            snooze_targets: Vec::new(),
            snooze_at: TextEdit::new(""),
            delete_open: false,
            delete_targets: Vec::new(),
            delete_token: None,
            delete_word: TextEdit::new(""),
            preview_open: false,
            preview_text: String::new(),
        }
    }
}

pub fn setup(state: &mut AppState, waker: rune::Waker) {
    state.images = Some(rune::ImageCache::new(waker));
}

impl AppState {
    /// Recompute the visible, thread-grouped rows from the loaded messages and the
    /// current query/filters. Returns whether anything the list draws changed.
    pub fn refresh_visible(&mut self) -> bool {
        let parsed = parse_query(&self.query.text);
        let filters = &self.filters;
        let mut rows: Vec<Row> = Vec::new();
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for message in &self.messages {
            if !matches_query(message, &parsed, filters) {
                continue;
            }
            let key = message
                .thread_id
                .clone()
                .unwrap_or_else(|| subject_key(&message.subject));
            if let Some(index) = seen.get(&key) {
                rows[*index].thread_count += 1;
                continue;
            }
            seen.insert(key, rows.len());
            rows.push(Row {
                message: message.clone(),
                thread_count: 1,
            });
        }
        let changed = rows.len() != self.visible.len()
            || rows
                .iter()
                .zip(self.visible.iter())
                .any(|(a, b)| a.message.id != b.message.id || a.thread_count != b.thread_count);
        self.visible = rows;
        if let Some(selected) = &self.selected {
            if let Some(index) = self
                .visible
                .iter()
                .position(|row| &row.message.id == selected)
            {
                self.cursor = index;
            }
        }
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
        changed
    }

    pub fn cancel_delete(&mut self) {
        if let Some(token) = self.delete_token.take() {
            self.send(Action::ResolveDelete {
                token,
                approved: false,
            });
        }
        self.delete_open = false;
    }

    pub fn confirm_delete(&mut self) {
        if self.delete_word.text.trim() != "delete" {
            self.toast = Some("Type delete to confirm".into());
            return;
        }
        if let Some(token) = self.delete_token.take() {
            self.delete_open = false;
            self.send(Action::ResolveDelete {
                token,
                approved: true,
            });
        } else {
            let ids = std::mem::take(&mut self.delete_targets);
            self.delete_open = false;
            self.send(Action::DeletePermanent(ids));
        }
    }

    pub fn save_search(&mut self) {
        let name = self.filters.name.text.trim().to_string();
        let name = if name.is_empty() {
            "Saved search".to_string()
        } else {
            name
        };
        let entry = serde_json::json!({
            "name": name,
            "query": self.query.text,
            "from": self.filters.from.text,
            "subject": self.filters.subject.text,
            "body": self.filters.body.text,
            "since": self.filters.since.text,
            "until": self.filters.until.text,
            "starred": self.filters.starred,
            "attachments": self.filters.attachments,
            "unread": self.filters.unread_only,
            "category": self.filters.category.text,
            "tag": self.filters.tag.text,
            "list": self.filters.list_id.text,
        });
        let mut searches = self
            .settings
            .pointer("/mail/saved_searches")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        searches.push(entry);
        self.apply_settings(serde_json::json!({"mail": {"saved_searches": searches}}));
    }

    pub fn apply_saved_search(&mut self, value: &serde_json::Value) {
        self.query = TextEdit::new(
            value
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.from = TextEdit::new(
            value
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.subject = TextEdit::new(
            value
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.body = TextEdit::new(
            value
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.since = TextEdit::new(
            value
                .get("since")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.until = TextEdit::new(
            value
                .get("until")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.starred = value
            .get("starred")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.filters.attachments = value
            .get("attachments")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.filters.unread_only = value
            .get("unread")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.filters.category = TextEdit::new(
            value
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.tag = TextEdit::new(
            value
                .get("tag")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters.list_id = TextEdit::new(
            value
                .get("list")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        self.filters_open = false;
        self.refresh_visible();
    }

    pub fn clear_filters(&mut self) {
        self.query = TextEdit::new("");
        self.filters = Filters::default();
        self.refresh_visible();
    }

    pub fn open_settings(&mut self, tab: SettingsTab) {
        self.settings_open = true;
        self.settings_tab = tab;
        self.send(Action::LoadSettingsData);
        self.send(Action::LoadDiagnostics);
    }

    pub fn close_settings(&mut self) {
        self.settings_open = false;
    }

    pub fn apply_settings(&mut self, patch: serde_json::Value) {
        if let Ok(settings) = settings::apply_patch(&self.data_dir, &patch) {
            self.settings = settings;
        }
    }

    pub fn set_theme_mode(&mut self, mode: &str) {
        self.theme.mode = match mode {
            "light" | "dark" => mode.to_string(),
            _ => "system".to_string(),
        };
        self.theme.persist();
    }

    pub fn set_theme_slot(&mut self, slot: &str, id: String) {
        if slot == "light" {
            self.theme.light = id;
        } else {
            self.theme.dark = id;
        }
        self.theme.persist();
    }

    pub fn reload_themes(&mut self) {
        self.theme.reload();
    }

    pub fn create_theme_from_template(&mut self) {
        let dir = aster_theme::themes_dir(&self.data_dir);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("my-theme.css");
        if !path.exists() {
            let _ = std::fs::write(&path, aster_theme::template_css());
        }
        self.theme.reload();
    }

    pub fn open_themes_folder(&self) {
        let dir = aster_theme::themes_dir(&self.data_dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }

    pub fn send(&self, action: Action) {
        if let Some(tx) = &self.actions {
            let _ = tx.send(action);
        }
    }

    pub fn cursor_id(&self) -> Option<String> {
        self.visible
            .get(self.cursor)
            .map(|row| row.message.id.clone())
    }

    /// The ids a batch action applies to: the selection, or the row under the cursor.
    pub fn target_ids(&self) -> Vec<String> {
        if !self.selection.is_empty() {
            return self.selection.iter().cloned().collect();
        }
        self.cursor_id().into_iter().collect()
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.visible.len() as isize - 1) as usize;
        if let Some(id) = self.cursor_id() {
            self.select(id);
        }
    }

    pub fn select(&mut self, id: String) {
        self.selected = Some(id.clone());
        self.send(Action::Select(id));
    }

    pub fn step_thread(&mut self, delta: isize) {
        if self.thread.len() < 2 {
            return;
        }
        let index = self
            .thread
            .iter()
            .position(|message| Some(&message.id) == self.selected.as_ref())
            .unwrap_or(0);
        let next = (index as isize + delta).clamp(0, self.thread.len() as isize - 1) as usize;
        if let Some(id) = self.thread.get(next).map(|message| message.id.clone()) {
            self.select(id);
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(id) = self.cursor_id() {
            if !self.selection.remove(&id) {
                self.selection.insert(id);
            }
        }
    }

    pub fn batch_read(&mut self, read: bool) {
        let ids = self.target_ids();
        if ids.is_empty() {
            return;
        }
        self.selection.clear();
        self.send(Action::SetRead { ids, read });
    }

    pub fn batch_star(&mut self, starred: bool) {
        let ids = self.target_ids();
        if ids.is_empty() {
            return;
        }
        self.selection.clear();
        self.send(Action::SetStarred { ids, starred });
    }

    pub fn batch_archive(&mut self) {
        let ids = self.target_ids();
        if ids.is_empty() {
            return;
        }
        self.selection.clear();
        self.send(Action::Archive(ids));
    }

    pub fn batch_trash(&mut self) {
        let ids = self.target_ids();
        if ids.is_empty() {
            return;
        }
        self.selection.clear();
        self.send(Action::Trash(ids));
    }

    pub fn open_folder(&mut self, folder: String) {
        if self.folder == folder {
            return;
        }
        self.folder = folder.clone();
        self.selected = None;
        self.detail = None;
        self.cursor = 0;
        self.selection.clear();
        self.send(Action::SetFolder(folder));
    }

    fn default_from(&self) -> String {
        self.identities
            .iter()
            .find(|identity| identity.is_default)
            .or_else(|| self.identities.first())
            .map(|identity| identity.address.clone())
            .unwrap_or_else(|| self.account.clone())
    }

    pub fn open_new_mail(&mut self) {
        self.compose = Compose {
            requested: true,
            from: TextEdit::new(self.default_from()),
            heading: "New message".into(),
            ..Compose::default()
        };
        self.insert_default_signature();
    }

    pub fn open_reply(&mut self, all: bool) {
        let Some(detail) = &self.detail else { return };
        let message = detail.message.clone();
        let cc = if all {
            let own = self.account.to_ascii_lowercase();
            let sender = email_of(&message.sender).to_ascii_lowercase();
            split_addresses(&message.recipients)
                .into_iter()
                .filter(|address| {
                    let address = address.to_ascii_lowercase();
                    address != own && address != sender
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            String::new()
        };
        self.compose = Compose {
            requested: true,
            from: TextEdit::new(self.default_from()),
            to: TextEdit::new(email_of(&message.sender)),
            cc: TextEdit::new(cc.clone()),
            subject: TextEdit::new(reply_subject(&message.subject)),
            in_reply_to: message.message_id.clone(),
            references: references_for(&message),
            heading: if all {
                "Reply all".into()
            } else {
                "Reply".into()
            },
            ..Compose::default()
        };
        self.insert_default_signature();
    }

    pub fn open_forward(&mut self) {
        let Some(detail) = &self.detail else { return };
        let message = detail.message.clone();
        self.compose = Compose {
            requested: true,
            from: TextEdit::new(self.default_from()),
            subject: TextEdit::new(format!("Fwd: {}", message.subject)),
            heading: "Forward".into(),
            body: TextEdit::new(forward_body(detail)),
            ..Compose::default()
        };
    }

    pub fn close_compose(&mut self) {
        self.compose.requested = false;
    }

    pub fn insert_default_signature(&mut self) {
        let signature = self
            .signatures
            .iter()
            .find(|signature| signature.is_default)
            .or_else(|| self.signatures.first());
        if let Some(signature) = signature {
            let mut text = self.compose.body.text.clone();
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str("-- \n");
            text.push_str(&signature.content);
            self.compose.body = TextEdit::new(text);
        }
    }

    #[cfg(test)]
    pub fn identity_for_test(&mut self) {
        self.identities = vec![SendIdentityView {
            address: "me@x.org".into(),
            kind: "primary".into(),
            display_name: None,
            is_default: true,
        }];
    }

    pub fn outgoing(&self) -> Outgoing {
        let expires_at = self.compose.expires_at.text.trim();
        Outgoing {
            from: self.compose.from.text.trim().to_string(),
            to: split_addresses(&self.compose.to.text),
            cc: split_addresses(&self.compose.cc.text),
            bcc: split_addresses(&self.compose.bcc.text),
            subject: self.compose.subject.text.trim().to_string(),
            markdown: self.compose.body.text.clone(),
            attachments: self.compose.attachments.clone(),
            in_reply_to: self.compose.in_reply_to.clone(),
            references: self.compose.references.clone(),
            expires_at: (!expires_at.is_empty()).then(|| expires_at.to_string()),
            obscure_subject: self.compose.obscure_subject,
        }
    }
}

pub fn split_addresses(value: &str) -> Vec<String> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|value| value.contains('@'))
        .map(str::to_string)
        .collect()
}

pub fn email_of(value: &str) -> String {
    let value = value.trim();
    if let (Some(start), Some(end)) = (value.find('<'), value.rfind('>')) {
        if end > start {
            return value[start + 1..end].trim().to_string();
        }
    }
    value.to_string()
}

fn reply_subject(subject: &str) -> String {
    let lower = subject.trim().to_ascii_lowercase();
    if lower.starts_with("re:") {
        subject.trim().to_string()
    } else {
        format!("Re: {}", subject.trim())
    }
}

fn references_for(message: &MessageView) -> Option<String> {
    let mut references = message.references.clone().unwrap_or_default();
    if let Some(id) = &message.message_id {
        if !references.is_empty() {
            references.push(' ');
        }
        references.push_str(id);
    }
    (!references.is_empty()).then_some(references)
}

fn forward_body(detail: &MessageDetail) -> String {
    let message = &detail.message;
    let body = if detail.is_html {
        strip_tags(&detail.body)
    } else {
        detail.body.clone()
    };
    format!(
        "\n\n---------- Forwarded message ----------\nFrom: {}\nDate: {}\nSubject: {}\n\n{}",
        message.sender,
        message.date.as_deref().unwrap_or(""),
        message.subject,
        body.trim()
    )
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_split_and_unwrap() {
        assert_eq!(
            split_addresses("a@x.org, Name <b@y.org>; c@z.org"),
            vec!["a@x.org", "Name <b@y.org>", "c@z.org",]
        );
        assert_eq!(email_of("Name <a@x.org>"), "a@x.org");
        assert_eq!(email_of("a@x.org"), "a@x.org");
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn outgoing_uses_compose_fields() {
        let mut state = AppState::default();
        state.identity_for_test();
        state.compose.to = TextEdit::new("a@x.org");
        state.compose.subject = TextEdit::new("Hi");
        state.compose.body = TextEdit::new("Body");
        state.compose.expires_at = TextEdit::new("2026-12-01T00:00:00Z");
        let outgoing = state.outgoing();
        assert_eq!(outgoing.to, vec!["a@x.org"]);
        assert_eq!(outgoing.subject, "Hi");
        assert_eq!(outgoing.markdown, "Body");
        assert_eq!(outgoing.expires_at.as_deref(), Some("2026-12-01T00:00:00Z"));
        assert!(!outgoing.obscure_subject);
    }
}

#[derive(Default)]
struct ParsedQuery {
    text: Vec<String>,
    from: String,
    to: String,
    subject: String,
    folder: String,
    unread: bool,
    read: bool,
    starred: bool,
    attachments: bool,
    before: String,
    after: String,
}

fn parse_query(value: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    for token in value.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if let Some((key, raw)) = lower.split_once(':') {
            let val = raw.trim_matches('"').to_string();
            match key {
                "from" => parsed.from = val,
                "to" => parsed.to = val,
                "subject" => parsed.subject = val,
                "in" => parsed.folder = val,
                "before" => parsed.before = val,
                "after" => parsed.after = val,
                "is" if val == "unread" => parsed.unread = true,
                "is" if val == "read" => parsed.read = true,
                "is" if val == "starred" => parsed.starred = true,
                "has" if val == "attachment" => parsed.attachments = true,
                _ => parsed.text.push(lower),
            }
        } else {
            parsed.text.push(lower);
        }
    }
    parsed
}

fn matches_query(message: &MessageView, query: &ParsedQuery, filters: &Filters) -> bool {
    let lower = |value: &str| value.to_ascii_lowercase();
    let date = message.date.as_deref().unwrap_or("");
    if !filters.list_id.text.is_empty()
        && message.list_id.as_deref() != Some(filters.list_id.text.as_str())
    {
        return false;
    }
    if filters.unread_only && message.is_read {
        return false;
    }
    if query.unread && message.is_read {
        return false;
    }
    if query.read && !message.is_read {
        return false;
    }
    if (query.starred || filters.starred) && !message.is_starred {
        return false;
    }
    if (query.attachments || filters.attachments) && !message.has_attachments {
        return false;
    }
    let from = if !filters.from.text.is_empty() {
        lower(&filters.from.text)
    } else {
        query.from.clone()
    };
    if !from.is_empty() && !lower(&message.sender).contains(&from) {
        return false;
    }
    let to = query.to.as_str();
    if !to.is_empty() && !lower(&message.recipients).contains(to) {
        return false;
    }
    let subject = if !filters.subject.text.is_empty() {
        lower(&filters.subject.text)
    } else {
        query.subject.clone()
    };
    if !subject.is_empty() && !lower(&message.subject).contains(&subject) {
        return false;
    }
    if !query.folder.is_empty() && !lower(&message.folder).contains(&query.folder) {
        return false;
    }
    if !filters.body.text.is_empty()
        && !lower(&message.preview).contains(&lower(&filters.body.text))
    {
        return false;
    }
    if !filters.category.text.is_empty()
        && message.category.as_deref() != Some(filters.category.text.as_str())
    {
        return false;
    }
    if !filters.tag.text.is_empty() && !message.tags.iter().any(|tag| tag == &filters.tag.text) {
        return false;
    }
    let since = if !filters.since.text.is_empty() {
        filters.since.text.as_str()
    } else {
        query.after.as_str()
    };
    if !since.is_empty() && date.get(..10).unwrap_or(date) < since.get(..10).unwrap_or(since) {
        return false;
    }
    let until = if !filters.until.text.is_empty() {
        filters.until.text.as_str()
    } else {
        query.before.as_str()
    };
    if !until.is_empty() && date.get(..10).unwrap_or(date) > until.get(..10).unwrap_or(until) {
        return false;
    }
    if !query.text.is_empty() {
        let hay = format!("{} {} {}", message.sender, message.subject, message.preview)
            .to_ascii_lowercase();
        if !query.text.iter().all(|term| hay.contains(term)) {
            return false;
        }
    }
    true
}

fn subject_key(subject: &str) -> String {
    subject
        .trim()
        .trim_start_matches("Re:")
        .trim_start_matches("RE:")
        .trim_start_matches("Fwd:")
        .trim_start_matches("FWD:")
        .trim()
        .to_ascii_lowercase()
}
