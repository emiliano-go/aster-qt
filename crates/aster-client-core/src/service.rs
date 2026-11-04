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
        Ok(ids
            .iter()
            .filter_map(|id| map.remove(id))
            .map(|msg| view_of(&msg))
            .collect())
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
            .map_err(ClientError::from_bridge)
    }

    pub async fn unsnooze_message(&self, id: &str) -> ClientResult<()> {
        let token = self.access_token().await;
        self.client()
            .unsnooze_mail(&token, id)
            .await
            .map_err(ClientError::from_bridge)
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
