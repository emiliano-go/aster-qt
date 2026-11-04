// SPDX-License-Identifier: AGPL-3.0-or-later
use aster_bridge_core::dav::store::{ContactsStore, DeleteOutcome, Preconditions, PutOutcome};
use std::path::PathBuf;
use aster_bridge_core::db::CachedMessage;
use base64::Engine as _;
use mail_builder::headers::address::Address;
use mail_builder::MessageBuilder;
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
    pub list_id: Option<String>,
    pub list_name: Option<String>,
    pub list_unsubscribe: Option<String>,
    pub list_unsubscribe_post: Option<String>,
    pub is_pinned: bool,
    pub category: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub thread_id: Option<String>,
    pub tags: Vec<String>,
    pub snoozed_until: Option<String>,
    pub cc: Option<String>,
    pub bcc: Option<String>,
    pub reply_to: Option<String>,
    pub routing_token: Option<String>,
    pub spf_result: Option<String>,
    pub dkim_result: Option<String>,
    pub dmarc_result: Option<String>,
    pub sender_verified: bool,
    pub sender_verified_domain: Option<String>,
    pub sender_aligned: bool,
    pub phishing_level: Option<String>,
    pub system_origin: bool,
    pub scheduled_at: Option<String>,
    pub send_status: Option<String>,
    pub expires_at: Option<String>,
    pub expiry_type: Option<String>,
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

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExternalContentReport {
    pub blocked_count: u32,
    pub has_remote_images: bool,
    pub has_tracking_pixels: bool,
    pub has_remote_fonts: bool,
    pub has_remote_css: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageDetail {
    #[serde(flatten)]
    pub message: MessageView,
    pub body: String,
    pub is_html: bool,
    pub body_html: String,
    pub external_content: ExternalContentReport,
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

    /// Cached message metadata for a folder or virtual view, newest first.
    pub fn messages(
        &self,
        folder: &str,
        limit: usize,
        offset: usize,
    ) -> ClientResult<Vec<MessageView>> {
        let mut rows = if let Some(tag) = folder.strip_prefix("tag:") {
            self.db()
                .list_tag_message_meta(tag)
                .map_err(ClientError::Storage)?
        } else if let Some(alias) = folder.strip_prefix("alias:") {
            let alias = alias.to_lowercase();
            self.db()
                .list_message_meta_for_view("all")
                .map_err(ClientError::Storage)?
                .into_iter()
                .filter(|row| {
                    row.sender
                        .as_deref()
                        .is_some_and(|value| value.to_lowercase().contains(&alias))
                        || row
                            .recipients
                            .as_deref()
                            .is_some_and(|value| value.to_lowercase().contains(&alias))
                })
                .collect()
        } else {
            self.db()
                .list_message_meta_for_view(folder)
                .map_err(ClientError::Storage)?
        };
        rows.sort_by(|a, b| b.date.cmp(&a.date));
        let mut out: Vec<MessageView> = rows
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|msg| view_of(&msg))
            .collect();
        self.attach_tags(&mut out)?;
        self.attach_snoozes(&mut out)?;
        Ok(out)
    }

    pub fn view_counts(&self) -> ClientResult<serde_json::Value> {
        let mut out = serde_json::Map::new();
        for view in ["inbox", "starred", "all", "scheduled", "snoozed", "sent", "drafts", "archive", "spam", "trash"] {
            if let Ok((total, unread)) = self.db().count_messages_for_view(view) {
                out.insert(
                    view.to_string(),
                    serde_json::json!({ "total": total, "unread": unread }),
                );
            }
        }
        Ok(serde_json::Value::Object(out))
    }

    fn attach_snoozes(&self, messages: &mut [MessageView]) -> ClientResult<()> {
        if messages.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = messages.iter().map(|message| message.id.clone()).collect();
        let mut snoozes = self
            .db()
            .snoozed_until_for_ids(&ids)
            .map_err(ClientError::Storage)?;
        for message in messages {
            message.snoozed_until = snoozes.remove(&message.id);
        }
        Ok(())
    }

    fn attach_tags(&self, messages: &mut [MessageView]) -> ClientResult<()> {
        if messages.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = messages.iter().map(|message| message.id.clone()).collect();
        let mut tags = self
            .db()
            .message_tags(&ids)
            .map_err(ClientError::Storage)?;
        for message in messages {
            message.tags = tags.remove(&message.id).unwrap_or_default();
        }
        Ok(())
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
        self.attach_tags(&mut ordered)?;
        Ok(ordered)
    }

    pub fn mailing_lists(&self) -> ClientResult<Vec<MailingListView>> {
        let mut seen: std::collections::HashMap<String, MailingListView> =
            std::collections::HashMap::new();
        let folders = self.mailboxes()?;
        for folder in folders {
            let rows = self
                .db()
                .list_cached_message_meta(&folder.folder_label)
                .unwrap_or_default();
            for row in rows {
                let meta: serde_json::Value = row
                    .raw_headers
                    .as_deref()
                    .and_then(|raw| serde_json::from_str(raw).ok())
                    .unwrap_or(serde_json::Value::Null);
                let list_id = meta
                    .get("list_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or_else(|| infer_list_id(row.recipients.as_deref()));
                let Some(list_id) = list_id else {
                    continue;
                };
                let name = meta
                    .get("list_name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| list_name_from_id(&list_id));
                let entry = seen
                    .entry(list_id.clone())
                    .or_insert_with(|| MailingListView {
                        list_id: list_id.clone(),
                        name,
                        total: 0,
                        unread: 0,
                    });
                entry.total += 1;
                if row.flags & FLAG_SEEN == 0 {
                    entry.unread += 1;
                }
            }
        }
        let mut out = seen.into_values().collect::<Vec<_>>();
        out.sort_by(|a, b| b.unread.cmp(&a.unread).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    pub fn thread_messages(&self, thread_id: &str, limit: usize) -> ClientResult<Vec<MessageView>> {
        let ids = self
            .db()
            .with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT aster_id FROM message_cache WHERE thread_id = ?1 ORDER BY julianday(date) DESC LIMIT ?2",
                )?;
                let rows = stmt.query_map(rusqlite::params![thread_id, limit as i64], |row| {
                    row.get::<_, String>(0)
                })?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
            })
            .map_err(ClientError::Storage)?;
        let mut map = self
            .db()
            .get_cached_messages(&ids, false)
            .map_err(ClientError::Storage)?;
        let mut out: Vec<MessageView> = ids
            .iter()
            .filter_map(|id| map.remove(id))
            .map(|msg| view_of(&msg))
            .collect();
        self.attach_tags(&mut out)?;
        Ok(out)
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
        let mut body_html = safe_body_html(&body, is_html);
        if body_html.contains("cid:") {
            if let Ok(parts) = self.db().get_message_attachments(id) {
                for part in parts {
                    let Some(content_id) = part.content_id.as_deref() else {
                        continue;
                    };
                    if part.data.is_empty() {
                        continue;
                    }
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&part.data);
                    let data_url = format!("data:{};base64,{}", part.content_type, encoded);
                    body_html = body_html.replace(
                        &format!("cid:{}", content_id.trim_matches(['<', '>'])),
                        &data_url,
                    );
                }
            }
        }
        let meta: serde_json::Value = msg
            .raw_headers
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or(serde_json::Value::Null);
        let external_content = external_content_report(&body, is_html);
        let mut detail = MessageDetail {
            message: view_of(&msg),
            body,
            is_html,
            body_html,
            external_content,
            sender_unverified: aster_bridge_core::message_render::sender_unverified(&meta),
            not_end_to_end_encrypted: aster_bridge_core::message_render::not_end_to_end_encrypted(&meta),
            attachments_unverified: aster_bridge_core::message_render::attachments_unverified(&meta),
            attachments,
        };
        self.attach_tags(std::slice::from_mut(&mut detail.message))?;
        self.attach_snoozes(std::slice::from_mut(&mut detail.message))?;
        Ok(detail)
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

    pub fn attachment_preview(&self, id: &str, seq: i64) -> ClientResult<serde_json::Value> {
        let attachment = self
            .db()
            .get_message_attachments(id)
            .map_err(ClientError::Storage)?
            .into_iter()
            .find(|a| a.seq == seq)
            .ok_or_else(|| ClientError::Protocol(format!("attachment {}:{} not found", id, seq)))?;
        let mut value = serde_json::json!({
            "name": attachment.name,
            "content_type": attachment.content_type,
            "size": attachment.size,
        });
        if attachment.content_type.starts_with("image/") {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&attachment.data);
            value["data_url"] = serde_json::json!(format!(
                "data:{};base64,{}",
                attachment.content_type, encoded
            ));
        } else if attachment.content_type.starts_with("text/")
            || attachment.content_type == "application/json"
        {
            value["text"] = serde_json::json!(String::from_utf8_lossy(&attachment.data).to_string());
        }
        Ok(value)
    }

    pub async fn send_markdown(
        &self,
        from: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        markdown: &str,
    ) -> ClientResult<i64> {
        self.send_markdown_with_attachments(from, to, cc, bcc, subject, markdown, &[])
            .await
    }

    pub async fn send_markdown_with_attachments(
        &self,
        from: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        markdown: &str,
        attachments: &[PathBuf],
    ) -> ClientResult<i64> {
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            return Err(ClientError::Protocol("add at least one recipient".into()));
        }
        let from = match from {
            Some(value) if !value.trim().is_empty() => value.to_string(),
            _ => self.session().read().await.default_sender_address(),
        };
        let (plain, mut html) = crate::compose::markdown_to_email(markdown);
        let mut builder = MessageBuilder::new();
        builder = builder.from(Address::new_address(None::<String>, from.clone()));
        builder = builder.subject(subject.to_string());
        if !to.is_empty() {
            builder = builder.to(Address::new_list(
                to.iter()
                    .map(|email| Address::new_address(None::<String>, email.clone()))
                    .collect(),
            ));
        }
        if !cc.is_empty() {
            builder = builder.cc(Address::new_list(
                cc.iter()
                    .map(|email| Address::new_address(None::<String>, email.clone()))
                    .collect(),
            ));
        }
        if !bcc.is_empty() {
            builder = builder.bcc(Address::new_list(
                bcc.iter()
                    .map(|email| Address::new_address(None::<String>, email.clone()))
                    .collect(),
            ));
        }
        for path in attachments {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| ClientError::Protocol(format!("invalid attachment path: {}", path.display())))?;
            let data = std::fs::read(path)
                .map_err(|err| ClientError::Protocol(format!("read {}: {}", path.display(), err)))?;
            let content_type = attachment_content_type(name);
            let quoted = format!("src=\"{}\"", name);
            let dotted = format!("src=\"./{}\"", name);
            if content_type.starts_with("image/")
                && (html.contains(&quoted) || html.contains(&dotted))
            {
                let cid = format!("{}@aster", uuid::Uuid::new_v4().simple());
                let cid_url = format!("src=\"cid:{}\"", cid);
                html = html.replace(&quoted, &cid_url).replace(&dotted, &cid_url);
                builder = builder.inline(content_type, cid, data);
            } else {
                builder = builder.attachment(content_type, name, data);
            }
        }
        builder = builder.text_body(plain).html_body(html);
        let mut raw = builder
            .write_to_vec()
            .map_err(|e| ClientError::Protocol(format!("build message: {e}")))?;
        let settings = crate::settings::load(self.data_dir());
        if crate::settings::boolean(&settings, &["security", "gpg_sign_outgoing"], false) {
            let fingerprint = crate::settings::string(
                &settings,
                &["security", "gpg_key_fingerprint"],
                "",
            );
            if !fingerprint.is_empty() {
                raw = crate::pgp_agent::sign_mime(&raw, &fingerprint)
                    .map_err(|err| ClientError::Protocol(format!("gpg signing failed: {err}")))?;
            }
        }
        let envelope_to = to
            .iter()
            .chain(cc.iter())
            .chain(bcc.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let id = self
            .db()
            .outbox_insert(&raw, &from, &envelope_to)
            .map_err(ClientError::Storage)?;
        self.outbox_tx
            .send(id)
            .await
            .map_err(|_| ClientError::Session("outbox worker is not running".into()))?;
        Ok(id)
    }

    pub async fn send_markdown_with_headers(
        &self,
        from: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        markdown: &str,
        attachments: &[PathBuf],
        in_reply_to: Option<&str>,
        references: Option<&str>,
    ) -> ClientResult<i64> {
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            return Err(ClientError::Protocol("add at least one recipient".into()));
        }
        let from = match from {
            Some(value) if !value.trim().is_empty() => value.to_string(),
            _ => self.session().read().await.default_sender_address(),
        };
        let message_id = in_reply_to;
        let reference_chain = references;
        let raw = build_markdown_mime(
            &from,
            to,
            cc,
            bcc,
            subject,
            markdown,
            attachments,
            message_id,
            reference_chain,
        )?;
        let envelope_to = to
            .iter()
            .chain(cc.iter())
            .chain(bcc.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let id = self
            .db()
            .outbox_insert(&raw, &from, &envelope_to)
            .map_err(ClientError::Storage)?;
        self.outbox_tx
            .send(id)
            .await
            .map_err(|_| ClientError::Session("outbox worker is not running".into()))?;
        Ok(id)
    }

    pub async fn queue_markdown_with_attachments(
        &self,
        from: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        markdown: &str,
        attachments: &[PathBuf],
        delay_seconds: u64,
    ) -> ClientResult<String> {
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            return Err(ClientError::Protocol("add at least one recipient".into()));
        }
        let from = match from {
            Some(value) if !value.trim().is_empty() => value.to_string(),
            _ => self.session().read().await.default_sender_address(),
        };
        let (plain, mut html) = crate::compose::markdown_to_email(markdown);
        let mut builder = MessageBuilder::new();
        builder = builder.from(Address::new_address(None::<String>, from.clone()));
        builder = builder.subject(subject.to_string());
        if !to.is_empty() {
            builder = builder.to(Address::new_list(
                to.iter().map(|email| Address::new_address(None::<String>, email.clone())).collect(),
            ));
        }
        if !cc.is_empty() {
            builder = builder.cc(Address::new_list(
                cc.iter().map(|email| Address::new_address(None::<String>, email.clone())).collect(),
            ));
        }
        if !bcc.is_empty() {
            builder = builder.bcc(Address::new_list(
                bcc.iter().map(|email| Address::new_address(None::<String>, email.clone())).collect(),
            ));
        }
        for path in attachments {
            let name = path.file_name().and_then(|value| value.to_str()).ok_or_else(|| {
                ClientError::Protocol(format!("invalid attachment path: {}", path.display()))
            })?;
            let data = std::fs::read(path)
                .map_err(|err| ClientError::Protocol(format!("read {}: {}", path.display(), err)))?;
            let content_type = attachment_content_type(name);
            let quoted = format!("src=\"{}\"", name);
            let dotted = format!("src=\"./{}\"", name);
            if content_type.starts_with("image/") && (html.contains(&quoted) || html.contains(&dotted)) {
                let cid = format!("{}@aster", uuid::Uuid::new_v4().simple());
                let cid_url = format!("src=\"cid:{}\"", cid);
                html = html.replace(&quoted, &cid_url).replace(&dotted, &cid_url);
                builder = builder.inline(content_type, cid, data);
            } else {
                builder = builder.attachment(content_type, name, data);
            }
        }
        builder = builder.text_body(plain).html_body(html);
        let mut raw = builder
            .write_to_vec()
            .map_err(|e| ClientError::Protocol(format!("build message: {e}")))?;
        let settings = crate::settings::load(self.data_dir());
        if crate::settings::boolean(&settings, &["security", "gpg_sign_outgoing"], false) {
            let fingerprint = crate::settings::string(&settings, &["security", "gpg_key_fingerprint"], "");
            if !fingerprint.is_empty() {
                raw = crate::pgp_agent::sign_mime(&raw, &fingerprint)
                    .map_err(|err| ClientError::Protocol(format!("gpg signing failed: {err}")))?;
            }
        }
        let recipients: Vec<String> = to.iter().chain(cc.iter()).chain(bcc.iter()).cloned().collect();
        let (payload, token) = aster_bridge_core::smtp::server::build_threaded_send_payload(
            &raw,
            Some(from),
            recipients,
            self.session(),
            self.client(),
            self.db(),
        )
        .await
        .map_err(ClientError::from_bridge)?;
        let value = self
            .client()
            .queue_undo_send(&token, &payload, delay_seconds)
            .await
            .map_err(ClientError::from_bridge)?;
        value
            .get("queue_id")
            .and_then(|id| id.as_str())
            .map(str::to_string)
            .ok_or_else(|| ClientError::Protocol("queue response had no queue_id".into()))
    }

    pub async fn cancel_queued_send(&self, queue_id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .cancel_queued_send(&token, queue_id)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn schedule_markdown_with_attachments(
        &self,
        from: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        markdown: &str,
        attachments: &[PathBuf],
        scheduled_at: &str,
    ) -> ClientResult<String> {
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            return Err(ClientError::Protocol("add at least one recipient".into()));
        }
        let (token, identity_key, default_from, identities) = {
            let session = self.session().read().await;
            (
                session.access_token.to_string(),
                session.identity_key.clone().ok_or_else(|| {
                    ClientError::Session("scheduled keys are not unlocked".into())
                })?,
                session.default_sender_address(),
                session.send_identities.clone(),
            )
        };
        let from = from
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or(default_from);
        let sender_identity = identities
            .iter()
            .find(|identity| identity.address.eq_ignore_ascii_case(&from));
        let (_plain, html) = crate::compose::markdown_to_email(markdown);
        let all_recipients: Vec<String> = to.iter().chain(cc.iter()).chain(bcc.iter()).cloned().collect();
        let has_external = all_recipients
            .iter()
            .any(|address| !aster_bridge_core::crypto::internal_send::is_internal_address(address));
        let has_internal = all_recipients
            .iter()
            .any(|address| aster_bridge_core::crypto::internal_send::is_internal_address(address));
        let content = serde_json::json!({
            "to_recipients": to,
            "cc_recipients": cc,
            "bcc_recipients": bcc,
            "subject": subject,
            "body": html,
            "plain_body": markdown,
            "scheduled_at": scheduled_at,
            "from": {
                "email": from,
                "name": sender_identity.and_then(|identity| identity.display_name.clone()),
            },
        });
        let recipients = serde_json::to_vec(&all_recipients)
            .map_err(|e| ClientError::Protocol(format!("scheduled recipients: {e}")))?;
        let envelope = serde_json::to_vec(&content)
            .map_err(|e| ClientError::Protocol(format!("scheduled content: {e}")))?;
        let (encrypted_envelope, envelope_nonce, ephemeral_key, base_nonce, recipients_enc) =
            if has_external {
                let (env, env_nonce, key, base) =
                    aster_bridge_core::crypto::scheduled::encrypt_ephemeral(&envelope)
                        .map_err(|e| ClientError::Protocol(format!("scheduled envelope: {e}")))?;
                let (rec, rec_nonce, _, _) =
                    aster_bridge_core::crypto::scheduled::encrypt_ephemeral(&recipients)
                        .map_err(|e| ClientError::Protocol(format!("scheduled recipients: {e}")))?;
                (env, env_nonce, Some(key), Some(base), (rec, rec_nonce))
            } else {
                let (env, env_nonce) =
                    aster_bridge_core::crypto::scheduled::encrypt_local(&identity_key, &envelope)
                        .map_err(|e| ClientError::Protocol(format!("scheduled envelope: {e}")))?;
                let (rec, rec_nonce) =
                    aster_bridge_core::crypto::scheduled::encrypt_local(&identity_key, &recipients)
                        .map_err(|e| ClientError::Protocol(format!("scheduled recipients: {e}")))?;
                (env, env_nonce, None, None, (rec, rec_nonce))
            };
        let mut body = serde_json::json!({
            "encrypted_envelope": encrypted_envelope,
            "envelope_nonce": envelope_nonce,
            "encrypted_recipients": recipients_enc.0,
            "recipients_nonce": recipients_enc.1,
            "recipient_count": all_recipients.len(),
            "scheduled_at": scheduled_at,
            "is_external": has_external,
        });
        body["sender_email"] = serde_json::json!(from);
        if let Some(identity) = sender_identity {
            if let Some(hash) = identity.auth_hash_b64.as_ref() {
                body["sender_alias_hash"] = serde_json::json!(hash);
            }
            if let Some(name) = identity.display_name.as_ref() {
                body["sender_display_name"] = serde_json::json!(name);
            }
        }
        if let Some(key) = ephemeral_key {
            body["ephemeral_key"] = serde_json::json!(key);
        }
        if let Some(base) = base_nonce {
            body["base_nonce"] = serde_json::json!(base);
        }
        if has_internal {
            let raw = build_markdown_mime(&from, to, cc, bcc, subject, markdown, attachments, None, None)?;
            let (payload, _) = aster_bridge_core::smtp::server::build_threaded_send_payload(
                &raw,
                Some(from.clone()),
                all_recipients.clone(),
                self.session(),
                self.client(),
                self.db(),
            )
            .await
            .map_err(ClientError::from_bridge)?;
            let internal_body = payload
                .get("internal_encrypted_body")
                .or_else(|| payload.get("body"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            body["delivery"] = serde_json::json!({
                "to": to,
                "cc": cc,
                "bcc": bcc,
                "sender_email": payload.get("sender_email").cloned().unwrap_or(serde_json::Value::Null),
                "sender_display_name": payload.get("sender_display_name").cloned().unwrap_or(serde_json::Value::Null),
                "internal_encrypted_body": internal_body,
                "recipient_bodies": payload.get("recipient_bodies").cloned().unwrap_or_else(|| serde_json::json!({})),
                "hosted_recipients": payload.get("hosted_recipients").cloned().unwrap_or_else(|| serde_json::json!([])),
            });
            if let Some(hash) = payload.get("sender_alias_hash") {
                body["sender_alias_hash"] = hash.clone();
            }
        }
        let response = self
            .client()
            .create_scheduled(&token, &body)
            .await
            .map_err(ClientError::from_bridge)?;
        let value = response.get("data").unwrap_or(&response);
        value
            .get("id")
            .or_else(|| value.get("scheduled_email_id"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| ClientError::Protocol("scheduled response had no id".into()))
    }

    pub async fn refresh_scheduled(&self) -> ClientResult<()> {
        let (token, identity_key, previous_keys) = {
            let session = self.session().read().await;
            (
                session.access_token.to_string(),
                session.identity_key.clone().ok_or_else(|| {
                    ClientError::Session("scheduled keys are not unlocked".into())
                })?,
                session.previous_keys.to_vec(),
            )
        };
        let listing = self
            .client()
            .list_scheduled(&token, 100, 0)
            .await
            .map_err(ClientError::from_bridge)?;
        let value = listing.get("data").unwrap_or(&listing);
        let items = value
            .get("items")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let mut current_ids = std::collections::HashSet::new();
        for item in items {
            let Some(id) = item.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            if !matches!(
                item.get("status").and_then(|value| value.as_str()).unwrap_or("pending"),
                "pending" | "failed" | "sending"
            ) {
                continue;
            }
            let Ok(full) = self.client().get_scheduled(&token, id).await else {
                continue;
            };
            let full = full.get("data").unwrap_or(&full);
            let Some(encrypted) = full.get("encrypted_envelope").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(nonce) = full.get("envelope_nonce").and_then(|value| value.as_str()) else {
                continue;
            };
            let plaintext = if full.get("is_external").and_then(|value| value.as_bool()) == Some(true) {
                full.get("ephemeral_key")
                    .and_then(|value| value.as_str())
                    .and_then(|key| {
                        aster_bridge_core::crypto::scheduled::decrypt_ephemeral(
                            encrypted, nonce, key,
                        )
                        .ok()
                    })
            } else {
                aster_bridge_core::crypto::scheduled::decrypt_local(
                    &identity_key,
                    &previous_keys,
                    encrypted,
                    nonce,
                )
                .ok()
            };
            let Some(plaintext) = plaintext else {
                continue;
            };
            let Ok(content) = serde_json::from_slice::<serde_json::Value>(&plaintext) else {
                continue;
            };
            let subject = content
                .get("subject")
                .and_then(|value| value.as_str())
                .unwrap_or("(no subject)");
            let recipients = content
                .get("to_recipients")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|value| value.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let body = content
                .get("body")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let scheduled_at = full
                .get("scheduled_at")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let meta = serde_json::json!({
                "is_html": true,
                "scheduled_at": scheduled_at,
                "send_status": item.get("status").and_then(|value| value.as_str()).unwrap_or("pending"),
                "cc": content.get("cc_recipients").cloned().unwrap_or_else(|| serde_json::json!([])),
                "bcc": content.get("bcc_recipients").cloned().unwrap_or_else(|| serde_json::json!([])),
            })
            .to_string();
            let _ = self.db().upsert_cached_message(
                id,
                "scheduled",
                Some(subject),
                None,
                Some(&recipients),
                Some(scheduled_at),
                body.len() as i64,
                Some(body),
                Some(&meta),
            );
            current_ids.insert(id.to_string());
        }
        if let Ok(rows) = self.db().list_cached_message_meta("scheduled") {
            for row in rows {
                if !current_ids.contains(&row.aster_id) {
                    let _ = self.db().delete_message_by_aster_id(&row.aster_id);
                }
            }
        }
        Ok(())
    }

    pub async fn cancel_scheduled(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .cancel_scheduled(&token, id)
            .await
            .map_err(ClientError::from_bridge)?;
        let _ = self.db().delete_message_by_aster_id(id);
        Ok(())
    }

    pub async fn send_scheduled_now(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .send_scheduled_now(&token, id)
            .await
            .map_err(ClientError::from_bridge)?;
        let _ = self.db().delete_message_by_aster_id(id);
        Ok(())
    }

    pub async fn reschedule(&self, id: &str, scheduled_at: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .update_scheduled(&token, id, scheduled_at)
            .await
            .map_err(ClientError::from_bridge)?;
        Ok(())
    }

    pub async fn server_templates(&self) -> ClientResult<Vec<TemplateView>> {
        let (token, key) = self.encryption_material().await?;
        let value = self
            .client()
            .list_templates(&token)
            .await
            .map_err(ClientError::from_bridge)?;
        let items = value
            .get("templates")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for item in items {
            let name = decrypt_field(&key, &item, "encrypted_name", "name_nonce")?;
            let category = decrypt_field(&key, &item, "encrypted_category", "category_nonce")?;
            let content = decrypt_field(&key, &item, "encrypted_content", "content_nonce")?;
            out.push(TemplateView {
                id: item["id"].as_str().unwrap_or_default().to_string(),
                name,
                category,
                content,
                sort_order: item["sort_order"].as_i64().unwrap_or(0),
            });
        }
        Ok(out)
    }

    pub async fn create_server_template(
        &self,
        name: &str,
        category: &str,
        content: &str,
    ) -> ClientResult<String> {
        let (token, key) = self.encryption_material().await?;
        let (enc_name, name_nonce) = encrypt_field(&key, name)?;
        let (enc_category, category_nonce) = encrypt_field(&key, category)?;
        let (enc_content, content_nonce) = encrypt_field(&key, content)?;
        let body = serde_json::json!({
            "encrypted_name": enc_name,
            "name_nonce": name_nonce,
            "encrypted_category": enc_category,
            "category_nonce": category_nonce,
            "encrypted_content": enc_content,
            "content_nonce": content_nonce,
            "sort_order": 0
        });
        let value = self
            .client()
            .create_template(&token, &body)
            .await
            .map_err(ClientError::from_bridge)?;
        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }

    pub async fn delete_server_template(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .delete_template(&token, id)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn server_signatures(&self) -> ClientResult<Vec<SignatureView>> {
        let (token, key) = self.encryption_material().await?;
        let value = self
            .client()
            .list_signatures(&token)
            .await
            .map_err(ClientError::from_bridge)?;
        let items = value
            .get("signatures")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for item in items {
            let name = decrypt_field(&key, &item, "encrypted_name", "name_nonce")?;
            let content = decrypt_field(&key, &item, "encrypted_content", "content_nonce")?;
            out.push(SignatureView {
                id: item["id"].as_str().unwrap_or_default().to_string(),
                name,
                content,
                is_html: item["is_html"].as_bool().unwrap_or(false),
                is_default: item["is_default"].as_bool().unwrap_or(false),
                alias_id: item["alias_id"].as_str().map(str::to_string),
            });
        }
        Ok(out)
    }

    pub async fn create_server_signature(&self, name: &str, content: &str) -> ClientResult<String> {
        let (token, key) = self.encryption_material().await?;
        let (enc_name, name_nonce) = encrypt_field(&key, name)?;
        let (enc_content, content_nonce) = encrypt_field(&key, content)?;
        let body = serde_json::json!({
            "encrypted_name": enc_name,
            "name_nonce": name_nonce,
            "encrypted_content": enc_content,
            "content_nonce": content_nonce,
            "is_default": false,
            "is_html": false,
            "alias_id": serde_json::Value::Null,
            "placement": serde_json::Value::Null
        });
        let value = self
            .client()
            .create_signature(&token, &body)
            .await
            .map_err(ClientError::from_bridge)?;
        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }

    pub async fn delete_server_signature(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .delete_signature(&token, id)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn set_default_signature(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .set_default_signature(&token, id)
            .await
            .map(|_| ())
            .map_err(ClientError::from_bridge)
    }

    async fn encryption_material(&self) -> ClientResult<(String, [u8; 32])> {
        let session = self.session().read().await;
        let token = session.access_token.to_string();
        let data_kek = session
            .data_kek
            .as_ref()
            .ok_or_else(|| ClientError::Session("encryption key is not unlocked".into()))?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data_kek.as_bytes())
            .map_err(|e| ClientError::Protocol(format!("data key decode: {e}")))?;
        let key: [u8; 32] = decoded
            .try_into()
            .map_err(|_| ClientError::Protocol("data key must be 32 bytes".into()))?;
        Ok((token, key))
    }

    pub async fn recipient_key_show(&self, address: &str) -> ClientResult<serde_json::Value> {
        let (token, username) = {
            let session = self.session().read().await;
            (session.access_token.to_string(), session.username.clone())
        };
        let key = aster_bridge_core::crypto::recipient_trust::fetch_recipient_account_key(
            self.client(),
            &token,
            &username,
            address,
        )
        .await
        .map_err(ClientError::from_bridge)?;
        let fingerprint = aster_bridge_core::crypto::recipient_trust::key_fingerprint(key.as_deref())
            .ok_or_else(|| ClientError::Protocol(format!("{address} has no published key")))?;
        Ok(serde_json::json!({
            "address": address,
            "fingerprint": aster_bridge_core::crypto::recipient_trust::format_fingerprint(&fingerprint),
        }))
    }

    pub async fn recipient_key_accept(
        &self,
        address: &str,
        fingerprint: &str,
    ) -> ClientResult<serde_json::Value> {
        let (token, username, account_id) = {
            let session = self.session().read().await;
            (session.access_token.to_string(), session.username.clone(), session.user_id.to_string())
        };
        let key = aster_bridge_core::crypto::recipient_trust::fetch_recipient_account_key(
            self.client(),
            &token,
            &username,
            address,
        )
        .await
        .map_err(ClientError::from_bridge)?;
        let outcome = aster_bridge_core::crypto::recipient_trust::accept_recipient_key(
            self.db(),
            &account_id,
            address,
            key.as_deref(),
            fingerprint,
        )
        .map_err(ClientError::Protocol)?;
        match outcome {
            aster_bridge_core::crypto::recipient_trust::KeyAcceptance::Accepted { fingerprint } => {
                Ok(serde_json::json!({
                    "address": address,
                    "accepted": true,
                    "fingerprint": aster_bridge_core::crypto::recipient_trust::format_fingerprint(&fingerprint),
                }))
            }
            aster_bridge_core::crypto::recipient_trust::KeyAcceptance::Mismatch { .. } => Err(
                ClientError::Protocol(format!("the key published for {address} does not match the fingerprint")),
            ),
        }
    }

    pub fn sign_out(&self, keep_cache: bool) -> ClientResult<()> {
        if !keep_cache {
            self.db().clear_user_data().map_err(ClientError::Storage)?;
        }
        aster_bridge_core::auth::device_identity::clear_identity(self.data_dir());
        aster_bridge_core::auth::device_identity::clear_passphrase(self.data_dir());
        Ok(())
    }

    pub fn repair_cache(&self) -> ClientResult<()> {
        self.db().repair_cache().map_err(ClientError::Storage)
    }

    pub fn diagnostics(&self) -> ClientResult<serde_json::Value> {
        let (messages, bytes, db_path) = self.db().db_stats().map_err(ClientError::Storage)?;
        let outbox = self.db().outbox_stats().map_err(ClientError::Storage)?;
        Ok(serde_json::json!({
            "messages": messages,
            "bytes": bytes,
            "db_path": db_path,
            "outbox": {
                "pending": outbox.pending,
                "failed": outbox.failed,
                "sent_24h": outbox.sent_24h,
            }
        }))
    }

    pub fn tags(&self) -> ClientResult<Vec<TagView>> {
        let tags = self.db().list_custom_tags().map_err(ClientError::Storage)?;
        Ok(tags
            .into_iter()
            .map(|tag| TagView {
                tag_token: tag.tag_token,
                name: tag.name,
                keyword: tag.keyword,
                position: tag.position,
            })
            .collect())
    }

    pub async fn create_tag(&self, name: &str) -> ClientResult<TagView> {
        let (token, identity_key) = self.auth_material().await;
        let tag = aster_bridge_core::tag_ops::create_label(
            self.db(),
            self.client(),
            &token,
            identity_key.as_deref(),
            name,
        )
        .await
        .map_err(|err| ClientError::Protocol(format!("{err:?}")))?;
        Ok(TagView {
            tag_token: tag.tag_token,
            name: tag.name,
            keyword: tag.keyword,
            position: tag.position,
        })
    }

    pub async fn delete_tag(&self, tag_token: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        let tags = self.db().list_custom_tags().map_err(ClientError::Storage)?;
        let tag = tags
            .into_iter()
            .find(|tag| tag.tag_token == tag_token)
            .ok_or_else(|| ClientError::Protocol("tag not found".into()))?;
        self.client()
            .delete_tag(&token, &tag.server_id)
            .await
            .map_err(ClientError::from_bridge)?;
        self.db()
            .delete_custom_tag(tag_token)
            .map_err(ClientError::Storage)?;
        Ok(())
    }

    pub async fn assign_tag(&self, ids: &[String], tag_token: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .add_tag(&token, ids, tag_token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn remove_tag(&self, ids: &[String], tag_token: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .remove_tag(&token, ids, tag_token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn create_folder(&self, name: &str, parent: Option<&str>) -> ClientResult<String> {
        let (access, identity_key) = self.auth_material().await;
        aster_bridge_core::folder_ops::create(
            self.db(),
            self.client(),
            &access,
            &identity_key.ok_or_else(|| ClientError::Session("folder keys are not unlocked".into()))?,
            name,
            parent,
        )
        .await
        .map_err(|err| ClientError::Protocol(format!("{err:?}")))
    }

    pub async fn rename_folder(&self, token: &str, name: &str) -> ClientResult<()> {
        let (access, identity_key) = self.auth_material().await;
        aster_bridge_core::folder_ops::update(
            self.db(),
            self.client(),
            &access,
            &identity_key.ok_or_else(|| ClientError::Session("folder keys are not unlocked".into()))?,
            token,
            Some(name),
            None,
        )
        .await
        .map_err(|err| ClientError::Protocol(format!("{err:?}")))
    }

    pub async fn delete_folder(&self, token: &str) -> ClientResult<()> {
        let access = self.access_token().await;
        aster_bridge_core::folder_ops::delete(self.db(), self.client(), &access, token)
            .await
            .map(|_| ())
            .map_err(|err| ClientError::Protocol(format!("{err:?}")))
    }

    pub async fn move_folder(&self, token: &str, parent: Option<&str>) -> ClientResult<()> {
        let (access, identity_key) = self.auth_material().await;
        aster_bridge_core::folder_ops::update(
            self.db(),
            self.client(),
            &access,
            &identity_key.ok_or_else(|| ClientError::Session("folder keys are not unlocked".into()))?,
            token,
            None,
            Some(parent),
        )
        .await
        .map_err(|err| ClientError::Protocol(format!("{err:?}")))
    }

    pub async fn snooze_messages(&self, ids: &[String], until_iso8601: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .snooze_mail(&token, ids, until_iso8601)
            .await
            .map_err(ClientError::from_bridge)?;
        for id in ids {
            self.db()
                .set_message_snooze(id, until_iso8601)
                .map_err(ClientError::Storage)?;
        }
        Ok(())
    }

    pub async fn unsnooze_message(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .unsnooze_mail(&token, id)
            .await
            .map_err(ClientError::from_bridge)?;
        self.db()
            .clear_message_snooze(id)
            .map_err(ClientError::Storage)
    }

    pub async fn ensure_attachments(&self, id: &str) -> ClientResult<()> {
        let _ = aster_bridge_core::sync::poller::on_demand::ensure_attachments(
            self.db(),
            self.client(),
            self.session(),
            id,
            None,
        )
        .await;
        Ok(())
    }

    async fn auth_material(&self) -> (String, Option<String>) {
        let session = self.session().read().await;
        (
            session.access_token.to_string(),
            session.identity_key.clone(),
        )
    }

    pub async fn move_messages(&self, ids: &[String], folder_token: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .move_to_folder(&token, ids, folder_token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn set_read(&self, ids: &[String], read: bool) -> ClientResult<()> {
        let token = self.access_token().await;
        for id in ids {
            self.client()
                .set_read_status(&token, id, read)
                .await
                .map_err(ClientError::from_bridge)?;
        }
        Ok(())
    }

    pub async fn set_starred(&self, ids: &[String], starred: bool) -> ClientResult<()> {
        let token = self.access_token().await;
        for id in ids {
            self.client()
                .set_mailbox_flags(&token, id, serde_json::json!({ "is_starred": starred }))
                .await
                .map_err(ClientError::from_bridge)?;
        }
        Ok(())
    }

    pub fn messages_from_sender(&self, sender: &str, limit: usize) -> ClientResult<Vec<MessageView>> {
        let ids = self
            .db()
            .with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT aster_id FROM message_cache WHERE lower(sender) LIKE lower(?1) ORDER BY julianday(date) DESC LIMIT ?2",
                )?;
                let pattern = format!("%{}%", sender);
                let rows = stmt.query_map(rusqlite::params![pattern, limit as i64], |row| {
                    row.get::<_, String>(0)
                })?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
            })
            .map_err(ClientError::Storage)?;
        let mut map = self
            .db()
            .get_cached_messages(&ids, false)
            .map_err(ClientError::Storage)?;
        Ok(ids
            .iter()
            .filter_map(|id| map.remove(id))
            .map(|msg| view_of(&msg))
            .collect())
    }

    pub async fn archive_newsletters(&self) -> ClientResult<usize> {
        let rows = self
            .db()
            .list_cached_message_meta("inbox")
            .map_err(ClientError::Storage)?;
        let ids: Vec<String> = rows
            .into_iter()
            .filter(|row| {
                row.raw_headers
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                    .map(|meta| {
                        meta.get("list_id").is_some() || meta.get("list_unsubscribe").is_some()
                    })
                    .unwrap_or(false)
            })
            .map(|row| row.aster_id)
            .collect();
        if ids.is_empty() {
            return Ok(0);
        }
        self.move_messages(&ids, "archive").await?;
        Ok(ids.len())
    }

    pub async fn unsubscribe_messages(&self, ids: &[String]) -> ClientResult<UnsubscribeReport> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ClientError::Protocol(e.to_string()))?;
        let mut attempted = 0;
        let mut succeeded = 0;
        for id in ids {
            let Some(msg) = self.db().get_cached_message(id).map_err(ClientError::Storage)? else {
                continue;
            };
            let Some(meta) = msg
                .raw_headers
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            else {
                continue;
            };
            let Some(header) = meta.get("list_unsubscribe").and_then(|v| v.as_str()) else {
                continue;
            };
            let post = meta
                .get("list_unsubscribe_post")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            for url in unsubscribe_urls(header) {
                if !url.starts_with("http") {
                    continue;
                }
                attempted += 1;
                let one_click = post.to_ascii_lowercase().contains("one-click");
                let result = if one_click {
                    client
                        .post(&url)
                        .header("content-type", "application/x-www-form-urlencoded")
                        .body("List-Unsubscribe=One-Click")
                        .send()
                        .await
                } else {
                    client.get(&url).send().await
                };
                if result.map(|resp| resp.status().is_success()).unwrap_or(false) {
                    succeeded += 1;
                }
            }
        }
        Ok(UnsubscribeReport { attempted, succeeded })
    }

    pub async fn archive_mailing_list(&self, list_id: &str) -> ClientResult<usize> {
        let rows = self
            .db()
            .list_cached_message_meta("inbox")
            .map_err(ClientError::Storage)?;
        let ids: Vec<String> = rows
            .into_iter()
            .filter(|row| {
                row.raw_headers
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                    .and_then(|meta| meta.get("list_id").and_then(|v| v.as_str()).map(str::to_string))
                    .as_deref()
                    == Some(list_id)
            })
            .map(|row| row.aster_id)
            .collect();
        if ids.is_empty() {
            return Ok(0);
        }
        self.move_messages(&ids, "archive").await?;
        Ok(ids.len())
    }

    pub async fn set_pinned(&self, ids: &[String], pinned: bool) -> ClientResult<()> {
        let token = self.access_token().await;
        for id in ids {
            self.client()
                .set_mailbox_flags(&token, id, serde_json::json!({ "is_pinned": pinned }))
                .await
                .map_err(ClientError::from_bridge)?;
        }
        Ok(())
    }

    pub async fn trash_messages(&self, ids: &[String]) -> ClientResult<()> {
        self.move_messages(ids, "trash").await
    }

    pub async fn set_spam(&self, ids: &[String], spam: bool) -> ClientResult<()> {
        let token = self.access_token().await;
        let flags = if spam {
            serde_json::json!({ "is_spam": true })
        } else {
            serde_json::json!({ "is_spam": false, "is_trashed": false })
        };
        for id in ids {
            self.client()
                .set_mailbox_flags(&token, id, flags.clone())
                .await
                .map_err(ClientError::from_bridge)?;
        }
        Ok(())
    }

    pub async fn restore_messages(&self, ids: &[String]) -> ClientResult<()> {
        let token = self.access_token().await;
        for id in ids {
            self.client()
                .set_mailbox_flags(
                    &token,
                    id,
                    serde_json::json!({ "is_trashed": false, "is_spam": false }),
                )
                .await
                .map_err(ClientError::from_bridge)?;
        }
        Ok(())
    }

    pub async fn save_draft(
        &self,
        draft_id: Option<&str>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        message: &str,
        attachments: &[PathBuf],
    ) -> ClientResult<String> {
        let (token, identity_key) = self.auth_material().await;
        let identity_key = identity_key
            .ok_or_else(|| ClientError::Session("draft keys are not unlocked".into()))?;
        let mut draft_attachments = Vec::new();
        for path in attachments {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| ClientError::Protocol(format!("invalid attachment path: {}", path.display())))?;
            let data = std::fs::read(path)
                .map_err(|err| ClientError::Protocol(format!("read {}: {}", path.display(), err)))?;
            draft_attachments.push(aster_bridge_core::crypto::draft::DraftAttachment {
                id: String::new(),
                name: name.to_string(),
                size: format!("{}", data.len()),
                size_bytes: data.len() as i64,
                mime_type: attachment_content_type(name).to_string(),
                data_base64: base64::engine::general_purpose::STANDARD.encode(&data),
                content_id: None,
            });
        }
        let content = aster_bridge_core::crypto::draft::DraftContent {
            to_recipients: to.to_vec(),
            cc_recipients: cc.to_vec(),
            bcc_recipients: bcc.to_vec(),
            subject: subject.to_string(),
            message: message.to_string(),
            attachments: if draft_attachments.is_empty() { None } else { Some(draft_attachments) },
        };
        let (encrypted, nonce) = aster_bridge_core::crypto::draft::encrypt_draft_content(
            &content,
            &identity_key,
        )
        .map_err(|err| ClientError::Protocol(format!("encrypt draft: {err}")))?;
        let hash = aster_bridge_core::crypto::draft::draft_content_hash(&encrypted);
        let size = serde_json::to_vec(&content).map(|v| v.len() as i64).unwrap_or(0);
        let body = aster_bridge_core::api_client::CreateDraftBody {
            draft_type: "draft",
            encrypted_content: &encrypted,
            content_nonce: &nonce,
            content_hash: &hash,
            size_bytes: size,
            has_attachments: content.attachments.is_some(),
            attachment_count: content.attachments.as_ref().map(|v| v.len() as i64).unwrap_or(0),
            reply_to_id: None,
        };
        let created = self
            .client()
            .create_draft(&token, &body)
            .await
            .map_err(ClientError::from_bridge)?;
        if let Some(old) = draft_id {
            let _ = self.client().delete_draft(&token, old).await;
        }
        Ok(created.id)
    }

    pub async fn delete_draft(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .delete_draft(&token, id)
            .await
            .map_err(ClientError::from_bridge)?;
        let _ = self.db().delete_message_by_aster_id(id);
        Ok(())
    }

    pub async fn empty_folder(&self, folder: &str) -> ClientResult<usize> {
        let rows = self
            .db()
            .list_cached_message_meta(folder)
            .map_err(ClientError::Storage)?;
        let ids: Vec<String> = rows.into_iter().map(|row| row.aster_id).collect();
        if ids.is_empty() {
            return Ok(0);
        }
        self.delete_messages_permanent(&ids).await?;
        Ok(ids.len())
    }

    pub async fn delete_messages_permanent(&self, ids: &[String]) -> ClientResult<()> {
        let token = self.access_token().await;
        for id in ids {
            self.client()
                .delete_mail_item_permanent(&token, id)
                .await
                .map_err(ClientError::from_bridge)?;
            let _ = self.db().delete_message_by_aster_id(id);
        }
        Ok(())
    }

    pub fn gpg_status(&self) -> crate::pgp_agent::GpgStatus {
        crate::pgp_agent::detect()
    }

    pub async fn import_pgp_to_gpg_agent(&self) -> ClientResult<crate::pgp_agent::GpgStatus> {
        let key = self
            .session()
            .read()
            .await
            .identity_key
            .clone()
            .ok_or_else(|| ClientError::Session("PGP key is not unlocked".into()))?;
        crate::pgp_agent::import_secret_key(&key)
            .map_err(|err| ClientError::Protocol(format!("gpg-agent import failed: {err}")))
    }

    pub async fn pgp_key(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .get_pgp_key(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn wkd_status(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .wkd_status(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn keyserver_status(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .keyserver_status(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn publish_wkd(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .publish_wkd(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn unpublish_wkd(&self) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .unpublish_wkd(&token)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn publish_keyserver(&self, addresses: &[String]) -> ClientResult<serde_json::Value> {
        let token = self.access_token().await;
        self.client()
            .publish_keyserver(&token, addresses)
            .await
            .map_err(ClientError::from_bridge)
    }

    pub async fn contacts(&self) -> ClientResult<Vec<ContactView>> {
        let store = ContactsStore::new(self.client().clone(), self.session().clone());
        let entries = store.list().await.map_err(ClientError::from_bridge)?;
        Ok(entries
            .into_iter()
            .map(|entry| ContactView {
                uid: entry.uid,
                contact_id: entry.contact_id,
                etag: entry.etag,
                vcard: entry.vcard,
            })
            .collect())
    }

    pub async fn save_contact(&self, uid: &str, vcard: &str) -> ClientResult<ContactView> {
        let store = ContactsStore::new(self.client().clone(), self.session().clone());
        match store
            .put(uid, vcard, &Preconditions::default())
            .await
            .map_err(ClientError::from_bridge)?
        {
            PutOutcome::Stored(entry, _) => Ok(ContactView {
                uid: entry.uid,
                contact_id: entry.contact_id,
                etag: entry.etag,
                vcard: entry.vcard,
            }),
            PutOutcome::PreconditionFailed => {
                Err(ClientError::Protocol("contact changed elsewhere".into()))
            }
        }
    }

    pub async fn delete_contact(&self, uid: &str) -> ClientResult<()> {
        let store = ContactsStore::new(self.client().clone(), self.session().clone());
        match store
            .delete(uid, &Preconditions::default())
            .await
            .map_err(ClientError::from_bridge)?
        {
            DeleteOutcome::Deleted => Ok(()),
            DeleteOutcome::NotFound => Err(ClientError::Protocol("contact not found".into())),
            DeleteOutcome::PreconditionFailed => {
                Err(ClientError::Protocol("contact changed elsewhere".into()))
            }
        }
    }

    pub fn save_attachment(&self, id: &str, seq: i64) -> ClientResult<std::path::PathBuf> {
        let attachment = self
            .db()
            .get_message_attachments(id)
            .map_err(ClientError::Storage)?
            .into_iter()
            .find(|item| item.seq == seq)
            .ok_or_else(|| ClientError::Protocol(format!("attachment {id}:{seq} not found")))?;
        let dir = downloads_dir();
        std::fs::create_dir_all(&dir).map_err(|e| ClientError::Storage(e.to_string()))?;
        let name = safe_filename(&attachment.name, seq);
        let mut path = dir.join(&name);
        let mut counter = 1;
        while path.exists() {
            let stem = std::path::Path::new(&name)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("attachment");
            let ext = std::path::Path::new(&name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!(".{value}"))
                .unwrap_or_default();
            path = dir.join(format!("{stem} ({counter}){ext}"));
            counter += 1;
        }
        std::fs::write(&path, &attachment.data).map_err(|e| ClientError::Storage(e.to_string()))?;
        Ok(path)
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

    pub async fn reorder_rules(&self, ids: &[String]) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .reorder_mail_rules(&token, ids)
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
    let meta: serde_json::Value = msg
        .raw_headers
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or(serde_json::Value::Null);
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
        list_id: meta
            .get("list_id")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| infer_list_id(msg.recipients.as_deref())),
        list_name: meta
            .get("list_name")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| {
                infer_list_id(msg.recipients.as_deref()).map(|id| list_name_from_id(&id))
            }),
        list_unsubscribe: meta
            .get("list_unsubscribe")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        list_unsubscribe_post: meta
            .get("list_unsubscribe_post")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        is_pinned: meta.get("is_pinned").and_then(|value| value.as_bool()).unwrap_or(false),
        category: meta
            .get("category")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        message_id: meta
            .get("message_id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        in_reply_to: meta
            .get("in_reply_to")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        references: meta
            .get("references")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        thread_id: msg.thread_id.clone(),
        tags: Vec::new(),
        snoozed_until: None,
        cc: meta.get("cc").and_then(|value| value.as_str()).map(str::to_string),
        bcc: meta.get("bcc").and_then(|value| value.as_str()).map(str::to_string),
        reply_to: meta
            .get("reply_to")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        routing_token: meta
            .get("routing_token")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        spf_result: meta
            .get("spf_result")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        dkim_result: meta
            .get("dkim_result")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        dmarc_result: meta
            .get("dmarc_result")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        sender_verified: meta
            .get("sender_verified")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        sender_verified_domain: meta
            .get("sender_verified_domain")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        sender_aligned: meta
            .get("sender_aligned")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        phishing_level: meta
            .get("phishing_level")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        system_origin: meta
            .get("system_origin")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        scheduled_at: meta
            .get("scheduled_at")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        send_status: meta
            .get("send_status")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        expires_at: meta
            .get("expires_at")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        expiry_type: meta
            .get("expiry_type")
            .and_then(|value| value.as_str())
            .map(str::to_string),
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


#[derive(Debug, Clone, Serialize)]
pub struct OutboxView {
    pub id: i64,
    pub envelope_from: String,
    pub envelope_to: String,
    pub queued_at: i64,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub status: String,
}

impl ClientRuntime {
    pub fn outbox_messages(&self) -> ClientResult<Vec<OutboxView>> {
        let rows = self
            .db()
            .outbox_list_pending()
            .map_err(ClientError::Storage)?;
        Ok(rows
            .into_iter()
            .map(|row| OutboxView {
                id: row.id,
                envelope_from: row.envelope_from,
                envelope_to: row.envelope_to,
                queued_at: row.queued_at,
                attempts: row.attempts,
                last_error: row.last_error,
                status: row.status,
            })
            .collect())
    }

    pub async fn retry_outbox_message(&self, id: i64) -> ClientResult<()> {
        self.retry_send(id).await
    }
}

fn build_markdown_mime(
    from: &str,
    to: &[String],
    cc: &[String],
    bcc: &[String],
    subject: &str,
    markdown: &str,
    attachments: &[PathBuf],
    in_reply_to: Option<&str>,
    references: Option<&str>,
) -> ClientResult<Vec<u8>> {
    let (plain, mut html) = crate::compose::markdown_to_email(markdown);
    let mut builder = MessageBuilder::new();
    builder = builder.from(Address::new_address(None::<String>, from.to_string()));
    builder = builder.subject(subject.to_string());
    if let Some(value) = in_reply_to.filter(|value| !value.is_empty()) {
        builder = builder.in_reply_to(value.to_string());
    }
    if let Some(value) = references.filter(|value| !value.is_empty()) {
        builder = builder.references(value.to_string());
    }
    if !to.is_empty() {
        builder = builder.to(Address::new_list(
            to.iter()
                .map(|email| Address::new_address(None::<String>, email.clone()))
                .collect(),
        ));
    }
    if !cc.is_empty() {
        builder = builder.cc(Address::new_list(
            cc.iter()
                .map(|email| Address::new_address(None::<String>, email.clone()))
                .collect(),
        ));
    }
    if !bcc.is_empty() {
        builder = builder.bcc(Address::new_list(
            bcc.iter()
                .map(|email| Address::new_address(None::<String>, email.clone()))
                .collect(),
        ));
    }
    for path in attachments {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ClientError::Protocol(format!("invalid attachment path: {}", path.display())))?;
        let data = std::fs::read(path)
            .map_err(|err| ClientError::Protocol(format!("read {}: {}", path.display(), err)))?;
        let content_type = attachment_content_type(name);
        let quoted = format!("src=\"{}\"", name);
        let dotted = format!("src=\"./{}\"", name);
        if content_type.starts_with("image/") && (html.contains(&quoted) || html.contains(&dotted)) {
            let cid = format!("{}@aster", uuid::Uuid::new_v4().simple());
            let cid_url = format!("src=\"cid:{}\"", cid);
            html = html.replace(&quoted, &cid_url).replace(&dotted, &cid_url);
            builder = builder.inline(content_type, cid, data);
        } else {
            builder = builder.attachment(content_type, name, data);
        }
    }
    builder = builder.text_body(plain).html_body(html);
    builder
        .write_to_vec()
        .map_err(|e| ClientError::Protocol(format!("build message: {e}")))
}

fn attachment_content_type(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "log" | "md" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "json" => "application/json",
        "zip" => "application/zip",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

fn external_content_report(body: &str, is_html: bool) -> ExternalContentReport {
    if !is_html {
        return ExternalContentReport::default();
    }
    let lower = body.to_ascii_lowercase();
    let mut report = ExternalContentReport::default();
    for img in lower.split("<img").skip(1) {
        let tag = img.split('>').next().unwrap_or_default();
        let remote = tag.contains("src=\"http") || tag.contains("src='http") || tag.contains("src=http");
        if !remote {
            continue;
        }
        report.has_remote_images = true;
        report.blocked_count += 1;
        let tiny = tag.contains("width=\"1\"")
            || tag.contains("height=\"1\"")
            || tag.contains("width='1'")
            || tag.contains("height='1'")
            || tag.contains("width:1px")
            || tag.contains("height:1px")
            || tag.contains("width: 1px")
            || tag.contains("height: 1px");
        if tiny {
            report.has_tracking_pixels = true;
        }
    }
    if lower.contains("@font-face") || lower.contains(".woff") || lower.contains(".ttf") {
        report.has_remote_fonts = true;
        report.blocked_count += 1;
    }
    if lower.contains("@import") || lower.contains("<link") {
        report.has_remote_css = true;
        report.blocked_count += 1;
    }
    report
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
            "h6", "hr", "i", "li", "ol", "p", "pre", "s", "small", "span", "strong",
            "sub", "sup", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "u", "ul",
        ])
        .add_generic_attributes(&[
            "href", "alt", "title", "width", "height", "style", "class", "align",
            "valign", "colspan", "rowspan", "dir", "lang", "border", "cellpadding",
            "cellspacing", "bgcolor", "color", "face", "size",
        ])
        .add_url_schemes(&["http", "https", "mailto"]);
    let inlined = css_inline::inline(body).unwrap_or_else(|_| body.to_string());
    builder.clean(&inlined).to_string()
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


#[derive(Debug, Clone, Serialize)]
pub struct ContactView {
    pub uid: String,
    pub contact_id: String,
    pub etag: String,
    pub vcard: String,
}

fn downloads_dir() -> std::path::PathBuf {
    match std::env::var_os("XDG_DOWNLOAD_DIR") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Downloads"),
    }
}

fn safe_filename(name: &str, seq: i64) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        format!("attachment-{seq}")
    } else {
        trimmed.chars().take(180).collect()
    }
}

#[cfg(test)]
mod attachment_tests {
    use super::safe_filename;

    #[test]
    fn filenames_are_sanitized() {
        assert_eq!(safe_filename("../../etc/passwd", 1), "_.._etc_passwd");
        assert_eq!(safe_filename("", 7), "attachment-7");
        assert!(!safe_filename("a/b\\c:d", 1).contains('/'));
    }
}


#[derive(Debug, Clone, Serialize)]
pub struct MailingListView {
    pub list_id: String,
    pub name: String,
    pub total: u32,
    pub unread: u32,
}


fn infer_list_id(recipients: Option<&str>) -> Option<String> {
    let recipients = recipients?;
    for part in recipients.split([',', ';']) {
        let lower = part.to_lowercase();
        let email = lower
            .split('<')
            .nth(1)
            .and_then(|value| value.split('>').next())
            .unwrap_or_else(|| lower.trim());
        let email = email.trim();
        if !email.contains('@') {
            continue;
        }
        let domain = email.rsplit_once('@').map(|(_, domain)| domain).unwrap_or("");
        let local = email.rsplit_once('@').map(|(local, _)| local).unwrap_or("");
        if domain.contains("lists.")
            || domain.contains("list.")
            || domain.contains("googlegroups.com")
            || domain.contains("groups.io")
            || domain.contains("yahoogroups")
            || local.contains("list")
        {
            return Some(email.to_string());
        }
    }
    None
}

fn list_name_from_id(id: &str) -> String {
    let local = id.rsplit_once('@').map(|(local, _)| local).unwrap_or(id);
    local
        .trim_matches('.')
        .replace(['-', '_', '.'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}


#[derive(Debug, Clone, Serialize)]
pub struct TagView {
    pub tag_token: String,
    pub name: String,
    pub keyword: Option<String>,
    pub position: i64,
}


fn encrypt_field(key: &[u8; 32], plaintext: &str) -> ClientResult<(String, String)> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::Aes256Gcm;
    use aes_gcm::Nonce;
    use rand::RngCore;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| ClientError::Protocol(e.to_string()))?;
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let encrypted = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| ClientError::Protocol(format!("encrypt: {e}")))?;
    Ok((
        base64::engine::general_purpose::STANDARD.encode(encrypted),
        base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
    ))
}

fn decrypt_field(
    key: &[u8; 32],
    item: &serde_json::Value,
    encrypted_key: &str,
    nonce_key: &str,
) -> ClientResult<String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::Aes256Gcm;
    use aes_gcm::Nonce;
    let encrypted = item[encrypted_key]
        .as_str()
        .ok_or_else(|| ClientError::Protocol(format!("missing {encrypted_key}")))?;
    let nonce_b64 = item[nonce_key]
        .as_str()
        .ok_or_else(|| ClientError::Protocol(format!("missing {nonce_key}")))?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| ClientError::Protocol(e.to_string()))?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(encrypted)
        .map_err(|e| ClientError::Protocol(format!("decode ciphertext: {e}")))?;
    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(nonce_b64)
        .map_err(|e| ClientError::Protocol(format!("decode nonce: {e}")))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| ClientError::Protocol(format!("decrypt: {e}")))?;
    String::from_utf8(plaintext).map_err(|e| ClientError::Protocol(e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateView {
    pub id: String,
    pub name: String,
    pub category: String,
    pub content: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignatureView {
    pub id: String,
    pub name: String,
    pub content: String,
    pub is_html: bool,
    pub is_default: bool,
    pub alias_id: Option<String>,
}


#[derive(Debug, Clone, Serialize)]
pub struct UnsubscribeReport {
    pub attempted: usize,
    pub succeeded: usize,
}

fn unsubscribe_urls(header: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = header;
    while let Some(open) = rest.find('<') {
        if let Some(close) = rest[open..].find('>') {
            let url = rest[open + 1..open + close].trim();
            if !url.is_empty() {
                out.push(url.to_string());
            }
            rest = &rest[open + close + 1..];
        } else {
            break;
        }
    }
    if out.is_empty() {
        for part in header.split(',') {
            let trimmed = part.trim();
            if trimmed.starts_with("http") || trimmed.starts_with("mailto:") {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}
