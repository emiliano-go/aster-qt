// SPDX-License-Identifier: AGPL-3.0-or-later
use aster_bridge_core::db::CachedMessage;
use serde::Serialize;

use crate::error::{ClientError, ClientResult};
use crate::runtime::ClientRuntime;

#[derive(Debug, Clone, Serialize)]
pub struct MailboxView {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub role: Option<String>,
    pub sort_order: i32,
    pub folder_label: String,
    pub token: Option<String>,
    pub total: u32,
    pub unread: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageView {
    pub id: String,
    pub folder: String,
    pub subject: String,
    pub sender: String,
    pub recipients: String,
    pub date: Option<String>,
    pub size: i64,
    pub flags: i64,
    pub is_read: bool,
    pub is_starred: bool,
    pub has_attachments: bool,
    pub thread_id: Option<String>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CachedAttachmentView {
    pub seq: i64,
    pub name: String,
    pub content_type: String,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub size: i64,
    pub is_cached: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageDetail {
    #[serde(flatten)]
    pub message: MessageView,
    pub body: String,
    pub is_html: bool,
    pub body_html: String,
    pub sender_unverified: bool,
    pub not_end_to_end_encrypted: bool,
    pub attachments_unverified: bool,
    pub attachments: Vec<CachedAttachmentView>,
}

const FLAG_SEEN: i64 = 1;
const FLAG_FLAGGED: i64 = 4;

impl ClientRuntime {
    pub fn mailboxes(&self) -> ClientResult<Vec<MailboxView>> {
        let rows = self
            .db()
            .list_jmap_mailboxes()
            .map_err(ClientError::Storage)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let total = self
                .db()
                .count_cached_messages(&row.folder_label)
                .unwrap_or(0);
            let unread = self
                .db()
                .count_unread_messages(&row.folder_label)
                .unwrap_or(0);
            let token = row
                .folder_label
                .strip_prefix(aster_bridge_core::folders::LABEL_PREFIX)
                .map(str::to_string);
            out.push(MailboxView {
                id: row.id,
                name: row.name,
                parent_id: row.parent_id,
                role: row.role,
                sort_order: row.sort_order,
                folder_label: row.folder_label,
                token,
                total,
                unread,
            });
        }
        Ok(out)
    }

    /// Cached message metadata for a folder, newest first.
    pub fn messages(
        &self,
        folder: &str,
        limit: usize,
        offset: usize,
    ) -> ClientResult<Vec<MessageView>> {
        let mut rows = self
            .db()
            .list_cached_message_meta(folder)
            .map_err(ClientError::Storage)?;
        rows.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(rows
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|msg| view_of(&msg))
            .collect())
    }

    pub fn search(&self, query: &str, limit: usize) -> ClientResult<Vec<MessageView>> {
        let ids = self
            .db()
            .fts_search(query, limit as i64)
            .map_err(ClientError::Storage)?;
        let mut messages = self
            .db()
            .get_cached_messages(&ids, false)
            .map_err(ClientError::Storage)?;
        let mut ordered: Vec<MessageView> = ids
            .iter()
            .filter_map(|id| messages.remove(id))
            .map(|msg| view_of(&msg))
            .collect();
        ordered.truncate(limit);
        Ok(ordered)
    }

    pub fn message(&self, id: &str) -> ClientResult<MessageDetail> {
        let msg = self
            .db()
            .get_cached_message(id)
            .map_err(ClientError::Storage)?
            .ok_or_else(|| ClientError::Protocol(format!("message not found: {}", id)))?;
        let attachments = self
            .db()
            .get_message_attachment_meta(id)
            .map_err(ClientError::Storage)?
            .into_iter()
            .map(|a| CachedAttachmentView {
                seq: a.seq,
                name: a.name,
                content_type: a.content_type,
                content_id: a.content_id,
                is_inline: a.is_inline,
                size: a.size,
                is_cached: true,
            })
            .collect();
        let is_html = is_html(&msg);
        let body = msg.body_text.clone().unwrap_or_default();
        let body_html = safe_body_html(&body, is_html);
        let meta: serde_json::Value = msg
            .raw_headers
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or(serde_json::Value::Null);
        Ok(MessageDetail {
            message: view_of(&msg),
            body,
            is_html,
            body_html,
            sender_unverified: aster_bridge_core::message_render::sender_unverified(&meta),
            not_end_to_end_encrypted: aster_bridge_core::message_render::not_end_to_end_encrypted(&meta),
            attachments_unverified: aster_bridge_core::message_render::attachments_unverified(&meta),
            attachments,
        })
    }

    pub fn attachment(&self, id: &str, seq: i64) -> ClientResult<Vec<u8>> {
        self.db()
            .get_message_attachments(id)
            .map_err(ClientError::Storage)?
            .into_iter()
            .find(|a| a.seq == seq)
            .map(|a| a.data)
            .ok_or_else(|| ClientError::Protocol(format!("attachment {}:{} not found", id, seq)))
    }

    pub async fn send_identities(&self) -> Vec<SendIdentityView> {
        let session = self.session().read().await;
        session
            .send_identities
            .iter()
            .filter(|identity| identity.enabled)
            .map(|identity| SendIdentityView {
                address: identity.address.clone(),
                kind: identity.kind.as_str().to_string(),
                display_name: identity.display_name.clone(),
                is_default: session
                    .default_identity()
                    .is_some_and(|default| default.address == identity.address),
            })
            .collect()
    }

    pub async fn rule_runs(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .list_mail_rule_runs(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn rules(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .list_mail_rules(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn create_rule(&self, body: serde_json::Value) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .create_mail_rule(&token, &body)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn update_rule(
        &self,
        id: &str,
        body: serde_json::Value,
    ) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .update_mail_rule(&token, id, &body)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn delete_rule(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .delete_mail_rule(&token, id)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn run_rule_on_existing(
        &self,
        id: &str,
        include_trashed: bool,
    ) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .run_mail_rule_on_existing(&token, id, include_trashed)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn apply_rule_to_items(
        &self,
        id: &str,
        ids: &[String],
    ) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .apply_mail_rule_to_items(&token, id, ids)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn create_mailbox(&self, name: &str) -> ClientResult<String> {
        let (token, identity_key) = {
            let session = self.session().read().await;
            (
                session.access_token.to_string(),
                session.identity_key.clone(),
            )
        };
        let identity_key = identity_key.ok_or_else(|| {
            ClientError::Session("mailbox encryption keys are not unlocked yet".into())
        })?;
        aster_bridge_core::folder_ops::create(
            self.db(),
            self.client(),
            &token,
            &identity_key,
            name,
            None,
        )
        .await
        .map_err(|err| ClientError::Protocol(format!("{err:?}")))
    }

    /// Creates a dedicated mailbox and a server-side rule that routes mail for
    /// an alias into it. Returns (mailbox token, rule id).
    pub async fn route_alias_to_mailbox(
        &self,
        alias_address: &str,
        mailbox_name: &str,
    ) -> ClientResult<(String, String)> {
        let existing = self
            .mailboxes()?
            .into_iter()
            .find(|mailbox| mailbox.name.eq_ignore_ascii_case(mailbox_name));
        let token = match existing {
            Some(mailbox) => mailbox.token.ok_or_else(|| {
                ClientError::Protocol("system mailboxes cannot be rule targets".into())
            })?,
            None => self.create_mailbox(mailbox_name).await?,
        };
        let body = serde_json::json!({
            "name": format!("Route {} to {}", alias_address, mailbox_name),
            "color": "#d98e5c",
            "enabled": true,
            "match_mode": "all",
            "conditions": [{
                "field": "to",
                "op": "is",
                "value": alias_address,
                "case_sensitive": false
            }],
            "actions": [{
                "type": "move_to",
                "folder_token": token
            }]
        });
        let created = self.create_rule(body).await?;
        let rule_id = created
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        Ok((token, rule_id))
    }

    async fn access_token(&self) -> String {
        self.session().read().await.access_token.to_string()
    }

    pub fn unread_total(&self) -> u32 {
        self.mailboxes()
            .map(|rows| {
                rows.iter()
                    .filter(|m| m.folder_label != "drafts" && m.folder_label != "sent")
                    .map(|m| m.unread)
                    .sum()
            })
            .unwrap_or(0)
    }
}

fn view_of(msg: &CachedMessage) -> MessageView {
    MessageView {
        id: msg.aster_id.clone(),
        folder: msg.folder.clone(),
        subject: msg.subject.clone().unwrap_or_default(),
        sender: msg.sender.clone().unwrap_or_default(),
        recipients: msg.recipients.clone().unwrap_or_default(),
        date: msg.date.clone(),
        size: msg.size,
        flags: msg.flags,
        is_read: msg.flags & FLAG_SEEN != 0,
        is_starred: msg.flags & FLAG_FLAGGED != 0,
        has_attachments: has_attachments(msg),
        thread_id: msg.thread_id.clone(),
        preview: preview_of(msg.body_text.as_deref()),
    }
}

fn is_html(msg: &CachedMessage) -> bool {
    msg.raw_headers
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|meta| meta.get("is_html").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn has_attachments(msg: &CachedMessage) -> bool {
    msg.raw_headers
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .map(|meta| {
            meta.get("attachments")
                .and_then(|v| v.as_array())
                .is_some_and(|parts| !parts.is_empty())
                || meta.get("has_attachments").and_then(|v| v.as_bool()) == Some(true)
        })
        .unwrap_or(false)
}

fn preview_of(body: Option<&str>) -> String {
    let Some(body) = body else {
        return String::new();
    };
    let mut text = String::with_capacity(256);
    let mut in_tag = false;
    for ch in body.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(ch),
            _ => {}
        }
        if text.chars().count() >= 256 {
            break;
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}


#[derive(Debug, Clone, Serialize)]
pub struct SendIdentityView {
    pub address: String,
    pub kind: String,
    pub display_name: Option<String>,
    pub is_default: bool,
}


fn safe_body_html(body: &str, is_html: bool) -> String {
    if !is_html {
        return format!(
            "<pre style=\"white-space:pre-wrap;margin:0;font-family:Inter,system-ui,sans-serif;font-size:15px;line-height:1.5;color:inherit\">{}</pre>",
            escape_html(body)
        );
    }
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(&[
            "a", "b", "blockquote", "br", "caption", "code", "col", "colgroup", "dd", "div",
            "dl", "dt", "em", "figcaption", "figure", "font", "h1", "h2", "h3", "h4", "h5",
            "h6", "hr", "i", "img", "li", "ol", "p", "pre", "s", "small", "span", "strong",
            "sub", "sup", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "u", "ul",
        ])
        .add_generic_attributes(&[
            "href", "src", "alt", "title", "width", "height", "style", "class", "align",
            "valign", "colspan", "rowspan", "dir", "lang", "border", "cellpadding",
            "cellspacing", "bgcolor", "color", "face", "size",
        ])
        .add_url_schemes(&["http", "https", "mailto", "cid"]);
    let cleaned = builder.clean(body).to_string();
    format!(
        "<style>\
         html,body{{margin:0;padding:0;background:transparent;}}\
         body{{font-family:Inter,system-ui,sans-serif;font-size:15px;line-height:1.5;color:#1c1b19;word-break:break-word;}}\
         img{{max-width:100%;height:auto;}}\
         img[src^=\"http\"]{{display:none !important;}}\
         a{{color:#0b5fff;}}\
         pre{{white-space:pre-wrap;overflow-wrap:anywhere;}}\
         table{{border-collapse:collapse;max-width:100%;}}\
         td,th{{padding:4px;}}\
         blockquote{{margin:0 0 0 12px;padding-left:12px;border-left:2px solid #d0d0d0;color:#555;}}\
         </style>{}",
        cleaned
    )
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}
