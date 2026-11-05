// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::bridge::Command;

const PROTOCOL_VERSION: &str = "2024-11-05";

pub fn start(tx: Sender<Command>, data_dir: PathBuf) {
    thread::spawn(move || {
        let server = match Server::http("127.0.0.1:0") {
            Ok(server) => server,
            Err(err) => {
                tracing::warn!("MCP server could not bind: {err}");
                return;
            }
        };
        let port = match server.server_addr().to_ip() {
            Some(addr) => addr.port(),
            None => return,
        };
        let token = uuid::Uuid::new_v4().to_string();
        let path = data_dir.join("mcp.json");
        let body = json!({
            "url": format!("http://127.0.0.1:{port}/mcp"),
            "port": port,
            "token": token,
        });
        if let Ok(text) = serde_json::to_string_pretty(&body) {
            let _ = std::fs::write(&path, text);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
        tracing::info!("MCP listening on 127.0.0.1:{port}");

        let server = Arc::new(server);
        for request in server.incoming_requests() {
            let tx = tx.clone();
            let token = token.clone();
            let audit_dir = data_dir.clone();
            thread::spawn(move || handle(request, tx, token, audit_dir));
        }
    });
}

fn handle(mut request: Request, tx: Sender<Command>, token: String, audit_dir: PathBuf) {
    if request.method() != &Method::Post || request.url() != "/mcp" {
        let _ = request.respond(Response::from_string("not found").with_status_code(404));
        return;
    }
    let authorized = request.headers().iter().any(|header| {
        let name = header.field.as_str().as_str().to_ascii_lowercase();
        let value = header.value.as_str();
        (name == "authorization" && value == format!("Bearer {token}"))
            || (name == "x-mcp-token" && value == token)
    });
    if !authorized {
        let _ = request.respond(Response::from_string("unauthorized").with_status_code(401));
        return;
    }
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        let _ = request.respond(Response::from_string("bad request").with_status_code(400));
        return;
    }
    let message: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(_) => {
            let _ = request.respond(json_response(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": "parse error" }
            })));
            return;
        }
    };
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let method = message
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "aster-qt", "version": "0.1.0" }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        "notifications/initialized" => Ok(json!({})),
        "tools/call" => call_tool(&message, &tx),
        other => Err(format!("unknown method: {other}")),
    };
    let response = match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err(err) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": err }
        }),
    };
    if method == "tools/call" {
        audit(&audit_dir, &message, &response);
    }
    let _ = request.respond(json_response(response));
}

fn call_tool(message: &Value, tx: &Sender<Command>) -> Result<Value, String> {
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or("tool name is required")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(Command::Rpc {
        tool: name.to_string(),
        args,
        reply: reply_tx,
    })
    .map_err(|_| "mail engine is not running")?;
    let result = reply_rx
        .recv_timeout(Duration::from_secs(180))
        .map_err(|_| "tool timed out")?;
    match result {
        Ok(value) => Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
            "isError": false
        })),
        Err(err) => Ok(json!({
            "content": [{ "type": "text", "text": err }],
            "isError": true
        })),
    }
}

fn json_response(value: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&value).unwrap_or_default();
    let mut response = Response::from_data(body);
    if let Ok(header) = Header::from_bytes("content-type", "application/json") {
        response.add_header(header);
    }
    response
}

fn tools() -> Value {
    json!([
        { "name": "status", "description": "Account and sync status.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "mailboxes_list", "description": "List mailboxes and unread counts.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "identities", "description": "List send identities and aliases.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "mail_list", "description": "List cached mail in a folder.", "inputSchema": { "type": "object", "properties": { "folder": { "type": "string" }, "limit": { "type": "number" } } } },
        { "name": "mail_search", "description": "Search cached mail.", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" }, "limit": { "type": "number" } }, "required": ["query"] } },
        { "name": "thread_get", "description": "List messages in a thread.", "inputSchema": { "type": "object", "properties": { "thread_id": { "type": "string" }, "limit": { "type": "number" } }, "required": ["thread_id"] } },
        { "name": "mail_get", "description": "Read one message.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "mail_export", "description": "Export a mailbox view to an mbox file.", "inputSchema": { "type": "object", "properties": { "folder": { "type": "string" }, "path": { "type": "string" } }, "required": ["folder", "path"] } },
        { "name": "mail_get_remote", "description": "Read one message with remote images allowed.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "mail_move", "description": "Move messages to a folder token.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "folder_token": { "type": "string" } }, "required": ["ids", "folder_token"] } },
        { "name": "mail_set_read", "description": "Mark messages read or unread.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "read": { "type": "boolean" } }, "required": ["ids"] } },
        { "name": "mail_star", "description": "Star or unstar messages.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "starred": { "type": "boolean" } }, "required": ["ids"] } },
        { "name": "mail_bulk_unsubscribe", "description": "Attempt one-click or GET unsubscribe for messages with List-Unsubscribe headers.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } } }, "required": ["ids"] } },
        { "name": "mail_archive_newsletters", "description": "Archive every cached inbox message that looks like a newsletter or mailing list message.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "mail_unsubscribe_info", "description": "Return List-Unsubscribe metadata for a message.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "mailing_list_archive", "description": "Archive every cached inbox message from a mailing list.", "inputSchema": { "type": "object", "properties": { "list_id": { "type": "string" } }, "required": ["list_id"] } },
        { "name": "mail_pin", "description": "Pin or unpin messages.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "pinned": { "type": "boolean" } }, "required": ["ids"] } },
        { "name": "mail_from_sender_list", "description": "List cached messages from a sender.", "inputSchema": { "type": "object", "properties": { "sender": { "type": "string" }, "limit": { "type": "number" } }, "required": ["sender"] } },
        { "name": "mail_from_sender_archive", "description": "Archive all cached messages from a sender.", "inputSchema": { "type": "object", "properties": { "sender": { "type": "string" } }, "required": ["sender"] } },
        { "name": "mail_from_sender_trash", "description": "Move all cached messages from a sender to Trash.", "inputSchema": { "type": "object", "properties": { "sender": { "type": "string" } }, "required": ["sender"] } },
        { "name": "mail_from_sender_move", "description": "Move all cached messages from a sender to a folder token.", "inputSchema": { "type": "object", "properties": { "sender": { "type": "string" }, "folder_token": { "type": "string" } }, "required": ["sender", "folder_token"] } },
        { "name": "mail_empty_folder", "description": "Permanently delete every cached message in a folder, for example trash or spam.", "inputSchema": { "type": "object", "properties": { "folder": { "type": "string" } }, "required": ["folder"] } },
        { "name": "mail_trash", "description": "Move messages to Trash.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } } }, "required": ["ids"] } },
        { "name": "mail_schedule", "description": "Create a server-side scheduled message.", "inputSchema": { "type": "object", "properties": { "to": { "type": "string" }, "cc": { "type": "string" }, "bcc": { "type": "string" }, "subject": { "type": "string" }, "markdown": { "type": "string" }, "scheduled_at": { "type": "string" } }, "required": ["to", "subject", "markdown", "scheduled_at"] } },
        { "name": "mail_scheduled_cancel", "description": "Cancel a scheduled message.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "mail_spam", "description": "Mark messages as spam or not spam.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "spam": { "type": "boolean" } }, "required": ["ids", "spam"] } },
        { "name": "mail_restore", "description": "Restore messages from Trash or Spam.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } } }, "required": ["ids"] } },
        { "name": "mail_delete", "description": "Permanently delete messages. Always requires explicit confirmation in the Aster Qt UI.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } } }, "required": ["ids"] } },
        { "name": "rules_list", "description": "List server side mail rules.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "rules_create", "description": "Create a server side mail rule.", "inputSchema": { "type": "object", "properties": { "rule": { "type": "object" } }, "required": ["rule"] } },
        { "name": "rules_delete", "description": "Delete a mail rule.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "rules_update", "description": "Update a mail rule.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "rule": { "type": "object" } }, "required": ["id", "rule"] } },
        { "name": "rules_reorder", "description": "Reorder mail rules.", "inputSchema": { "type": "object", "properties": { "order": { "type": "array", "items": { "type": "string" } } }, "required": ["order"] } },
        { "name": "draft_save", "description": "Create or replace a server draft.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "to": { "type": "string" }, "cc": { "type": "string" }, "bcc": { "type": "string" }, "subject": { "type": "string" }, "message": { "type": "string" }, "attachments": { "type": "array", "items": { "type": "string" } } } } },
        { "name": "recipient_key_show", "description": "Show a recipient's published key fingerprint.", "inputSchema": { "type": "object", "properties": { "address": { "type": "string" } }, "required": ["address"] } },
        { "name": "recipient_key_accept", "description": "Accept a recipient's changed key after confirming the fingerprint.", "inputSchema": { "type": "object", "properties": { "address": { "type": "string" }, "fingerprint": { "type": "string" } }, "required": ["address", "fingerprint"] } },
        { "name": "folder_create", "description": "Create a custom folder, optionally under a parent token.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "parent_token": { "type": "string" } }, "required": ["name"] } },
        { "name": "mailbox_create", "description": "Create a custom mailbox.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] } },
        { "name": "alias_route", "description": "Create a mailbox and a rule that routes an alias into it.", "inputSchema": { "type": "object", "properties": { "alias": { "type": "string" }, "mailbox": { "type": "string" } }, "required": ["alias", "mailbox"] } },
        { "name": "sync_now", "description": "Trigger a sync pass.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "contacts_list", "description": "List contacts.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "contacts_save", "description": "Create or update a contact from a vCard.", "inputSchema": { "type": "object", "properties": { "uid": { "type": "string" }, "vcard": { "type": "string" } }, "required": ["uid", "vcard"] } },
        { "name": "contacts_delete", "description": "Delete a contact.", "inputSchema": { "type": "object", "properties": { "uid": { "type": "string" } }, "required": ["uid"] } },
        { "name": "attachment_list", "description": "List attachments for a message.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "attachment_save", "description": "Save an attachment to Downloads and return the path.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "seq": { "type": "number" } }, "required": ["id", "seq"] } },
        { "name": "templates_list", "description": "List server-synced templates.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "template_create", "description": "Create a server-synced template.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "category": { "type": "string" }, "content": { "type": "string" } }, "required": ["name", "content"] } },
        { "name": "template_delete", "description": "Delete a server-synced template.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "signatures_list", "description": "List server-synced signatures.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "signature_create", "description": "Create a server-synced signature.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "content": { "type": "string" } }, "required": ["name", "content"] } },
        { "name": "signature_delete", "description": "Delete a server-synced signature.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "signature_set_default", "description": "Set the default signature.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "tags_list", "description": "List tags.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "tag_create", "description": "Create a tag.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] } },
        { "name": "tag_delete", "description": "Delete a tag.", "inputSchema": { "type": "object", "properties": { "tag_token": { "type": "string" } }, "required": ["tag_token"] } },
        { "name": "tag_assign", "description": "Assign a tag to messages.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "tag_token": { "type": "string" } }, "required": ["ids", "tag_token"] } },
        { "name": "tag_remove", "description": "Remove a tag from messages.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "tag_token": { "type": "string" } }, "required": ["ids", "tag_token"] } },
        { "name": "folder_rename", "description": "Rename a custom folder.", "inputSchema": { "type": "object", "properties": { "folder_token": { "type": "string" }, "name": { "type": "string" } }, "required": ["folder_token", "name"] } },
        { "name": "folder_delete", "description": "Delete a custom folder.", "inputSchema": { "type": "object", "properties": { "folder_token": { "type": "string" } }, "required": ["folder_token"] } },
        { "name": "folder_move", "description": "Move a custom folder under a parent.", "inputSchema": { "type": "object", "properties": { "folder_token": { "type": "string" }, "parent_token": { "type": "string" } }, "required": ["folder_token"] } },
        { "name": "mail_snooze", "description": "Snooze messages until an ISO timestamp.", "inputSchema": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } }, "until_iso8601": { "type": "string" } }, "required": ["ids", "until_iso8601"] } },
        { "name": "mail_unsnooze", "description": "Unsnooze a message.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "attachment_fetch", "description": "Fetch missing attachments for a message on demand.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "mailing_lists_list", "description": "List mailing lists discovered from List-Id headers.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "drafts_list", "description": "List drafts.", "inputSchema": { "type": "object", "properties": { "limit": { "type": "number" } } } },
        { "name": "mail_send_queued", "description": "Queue a Markdown message with a server-side undo delay. Returns a queue id.", "inputSchema": { "type": "object", "properties": { "to": { "type": "string" }, "subject": { "type": "string" }, "markdown": { "type": "string" }, "delay_seconds": { "type": "number" } }, "required": ["to", "subject", "markdown"] } },
        { "name": "mail_cancel_queued", "description": "Cancel a queued send by queue id.", "inputSchema": { "type": "object", "properties": { "queue_id": { "type": "string" } }, "required": ["queue_id"] } },
        { "name": "mail_send", "description": "Send a Markdown message. Requires Send from MCP to be enabled in Settings.", "inputSchema": { "type": "object", "properties": { "to": { "type": "string" }, "cc": { "type": "string" }, "bcc": { "type": "string" }, "subject": { "type": "string" }, "markdown": { "type": "string" } }, "required": ["to", "subject", "markdown"] } },
        { "name": "gpg_status", "description": "Report system gpg and gpg-agent status plus secret key fingerprints.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "gpg_import_key", "description": "Import the Aster PGP key into the system gpg-agent. No export file is written.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "pgp_key_info", "description": "Return the account PGP key metadata and public key.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "pgp_status", "description": "Return WKD and keyserver publication status.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "pgp_publish", "description": "Publish the PGP key to WKD or a keyserver.", "inputSchema": { "type": "object", "properties": { "kind": { "type": "string" }, "addresses": { "type": "array", "items": { "type": "string" } } }, "required": ["kind"] } }
    ])
}


fn audit(dir: &std::path::Path, message: &Value, response: &Value) {
    let tool = message
        .pointer("/params/name")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let args = message
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let ok = response.get("error").is_none();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let line = json!({
        "ts": ts,
        "tool": tool,
        "args": args,
        "ok": ok
    });
    if let Ok(text) = serde_json::to_string(&line) {
        use std::io::Write as _;
        let _ = std::fs::create_dir_all(dir);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("mcp-audit.log"))
        {
            let _ = writeln!(file, "{text}");
        }
    }
}
