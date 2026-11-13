// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared MCP server for Aster front-ends.
//!
//! One registry, one protocol implementation and one HTTP transport. The
//! front-end supplies a tool-call closure that forwards to its live mail
//! engine, so the MCP client and the GUI share the same process and state.

#![recursion_limit = "512"]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use serde_json::{json, Map, Value};
use tiny_http::{Header, Method, Request, Response, Server};

const SUPPORTED_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
const OUR_VERSION: &str = "2025-06-18";
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const AUDIT_ARG_BYTES: usize = 200;

pub type ToolCall = dyn Fn(&str, Value) -> Result<Value, String> + Send + Sync + 'static;

struct State {
    data_dir: PathBuf,
    call: Arc<ToolCall>,
    revision: AtomicU64,
    server_name: &'static str,
    server_version: &'static str,
}

/// Bind the loopback MCP endpoint, write `mcp.json`, and serve until the
/// process exits. `call` forwards a tool call to the front-end mail engine.
pub fn start<F>(data_dir: PathBuf, server_name: &'static str, server_version: &'static str, call: F)
where
    F: Fn(&str, Value) -> Result<Value, String> + Send + Sync + 'static,
{
    let call: Arc<ToolCall> = Arc::new(call);
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
        let body = json!({
            "url": format!("http://127.0.0.1:{port}/mcp"),
            "port": port,
            "token": token,
        });
        if let Ok(text) = serde_json::to_string_pretty(&body) {
            let path = data_dir.join("mcp.json");
            let _ = std::fs::write(&path, text);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
        tracing::info!("MCP listening on 127.0.0.1:{port}");

        let state = Arc::new(State {
            data_dir,
            call,
            revision: AtomicU64::new(0),
            server_name,
            server_version,
        });
        let server = Arc::new(server);
        for request in server.incoming_requests() {
            let state = state.clone();
            let token = token.clone();
            thread::spawn(move || handle_http(&state, request, &token));
        }
    });
}

fn handle_http(state: &State, mut request: Request, token: &str) {
    let url = request.url().split('?').next().unwrap_or("/").to_string();
    if url != "/mcp" && url != "/" {
        let _ = request.respond(Response::from_string("not found").with_status_code(404));
        return;
    }
    if let Some(origin) = header(&request, "origin") {
        let origin = origin.trim();
        if !origin.is_empty()
            && !origin.starts_with("http://localhost")
            && !origin.starts_with("http://127.0.0.1")
            && !origin.starts_with("https://localhost")
            && !origin.starts_with("https://127.0.0.1")
        {
            let _ =
                request.respond(Response::from_string("origin not allowed").with_status_code(403));
            return;
        }
    }
    let authorized = header(&request, "authorization")
        .map(|value| value == format!("Bearer {token}"))
        .unwrap_or(false)
        || header(&request, "x-mcp-token")
            .map(|value| value == token)
            .unwrap_or(false);
    if !authorized {
        let _ = request.respond(Response::from_string("unauthorized").with_status_code(401));
        return;
    }
    if request.method() != &Method::Post {
        let _ = request.respond(Response::from_string("POST only").with_status_code(405));
        return;
    }
    if let Some(version) = header(&request, "mcp-protocol-version") {
        if !SUPPORTED_VERSIONS.contains(&version.trim()) {
            let _ = request.respond(
                Response::from_string("unsupported MCP-Protocol-Version").with_status_code(400),
            );
            return;
        }
    }
    let mut body = String::new();
    if request
        .as_reader()
        .take(MAX_BODY_BYTES as u64)
        .read_to_string(&mut body)
        .is_err()
    {
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
    if message.get("id").is_none() || message.get("method").is_none() {
        let _ = request.respond(Response::from_string("").with_status_code(202));
        return;
    }
    let response = match handle(state, &message) {
        Some(response) => response,
        None => {
            let _ = request.respond(Response::from_string("").with_status_code(202));
            return;
        }
    };
    if message.get("method").and_then(Value::as_str) == Some("tools/call") {
        audit(&state.data_dir, &message, &response);
    }
    let _ = request.respond(json_response(response));
}

fn header(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().to_string())
}

fn handle(state: &State, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(initialize(state, &params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(state, &params),
        "resources/list" => Ok(json!({ "resources": resource_list() })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": resource_templates() })),
        "resources/read" => read_resource(state, &params),
        "prompts/list" => Ok(json!({ "prompts": prompt_list() })),
        "prompts/get" => get_prompt(&params),
        other => Err(format!("unknown method: {other}")),
    };
    Some(match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err(err) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": err }
        }),
    })
}

fn initialize(state: &State, params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|version| SUPPORTED_VERSIONS.contains(version))
        .unwrap_or(OUR_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": { "name": state.server_name, "version": state.server_version },
        "instructions": "Aster mail. Read mail with mail_list/mail_get, change state with focused tools, and enable sending or destructive actions in Settings > MCP first."
    })
}

fn call_tool(state: &State, params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("tool name is required")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mutating = is_mutating(name);
    if mutating {
        if let Some(expected) = expected_revision(params, &args) {
            let current = current_revision(state)
                .ok_or("REVISION_UNAVAILABLE: the mail engine did not report a revision")?;
            if expected != current {
                return Err(format!(
                    "REVISION_CONFLICT: expected {expected}, current {current}"
                ));
            }
        }
    }
    let value = (state.call)(name, args)?;
    let revision = if mutating {
        current_revision(state).unwrap_or_else(|| state.revision.fetch_add(1, Ordering::SeqCst) + 1)
    } else {
        current_revision(state).unwrap_or_else(|| state.revision.load(Ordering::SeqCst))
    };
    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
        "structuredContent": value,
        "isError": false,
        "_meta": { "revision": revision }
    }))
}

fn current_revision(state: &State) -> Option<u64> {
    (state.call)("__revision", json!({}))
        .ok()
        .and_then(|value| value.get("revision").and_then(Value::as_u64))
}

fn expected_revision(params: &Value, args: &Value) -> Option<u64> {
    params
        .get("expectedRevision")
        .or_else(|| args.get("expectedRevision"))
        .or_else(|| args.get("expected_revision"))
        .and_then(Value::as_u64)
}

fn read_resource(state: &State, params: &Value) -> Result<Value, String> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or("uri is required")?;
    let (value, mime) = resource_value(state, uri)?;
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": mime,
            "text": serde_json::to_string_pretty(&value).unwrap_or_default()
        }]
    }))
}

fn resource_value(state: &State, uri: &str) -> Result<(Value, &'static str), String> {
    let path = uri
        .strip_prefix("aster://")
        .ok_or_else(|| format!("unsupported resource: {uri}"))?;
    if path == "status" {
        return Ok(((state.call)("status", json!({}))?, "application/json"));
    }
    if path == "mailboxes" {
        return Ok((
            (state.call)("mailboxes_list", json!({}))?,
            "application/json",
        ));
    }
    if path == "settings" {
        let settings = std::fs::read_to_string(state.data_dir.join("settings.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .unwrap_or_else(|| json!({}));
        return Ok((redact(&settings), "application/json"));
    }
    if path == "themes" {
        return Ok((
            aster_theme::themes_json(&state.data_dir),
            "application/json",
        ));
    }
    if path == "revision" {
        let revision =
            current_revision(state).unwrap_or_else(|| state.revision.load(Ordering::SeqCst));
        return Ok((json!({ "revision": revision }), "application/json"));
    }
    if let Some(id) = path.strip_prefix("mail/") {
        return Ok((
            (state.call)("mail_get", json!({ "id": id }))?,
            "application/json",
        ));
    }
    if let Some(id) = path.strip_prefix("thread/") {
        return Ok((
            (state.call)("thread_get", json!({ "thread_id": id }))?,
            "application/json",
        ));
    }
    Err(format!("unsupported resource: {uri}"))
}

fn resource_list() -> Value {
    json!([
        { "uri": "aster://status", "name": "Status", "description": "Account and sync status.", "mimeType": "application/json" },
        { "uri": "aster://mailboxes", "name": "Mailboxes", "description": "Mailboxes and unread counts.", "mimeType": "application/json" },
        { "uri": "aster://settings", "name": "Settings", "description": "Persisted settings with secrets redacted.", "mimeType": "application/json" },
        { "uri": "aster://themes", "name": "Themes", "description": "Available themes and their tokens.", "mimeType": "application/json" },
        { "uri": "aster://revision", "name": "Revision", "description": "Current MCP mutation revision.", "mimeType": "application/json" }
    ])
}

fn resource_templates() -> Value {
    json!([
        { "uriTemplate": "aster://mail/{id}", "name": "Message", "description": "One cached message by id.", "mimeType": "application/json" },
        { "uriTemplate": "aster://thread/{id}", "name": "Thread", "description": "Messages in one thread.", "mimeType": "application/json" }
    ])
}

fn prompt_list() -> Value {
    json!([
        {
            "name": "triage-inbox",
            "title": "Triage inbox",
            "description": "Review unread inbox mail and suggest archive, reply or snooze actions.",
            "arguments": [
                { "name": "folder", "description": "Folder token or name.", "required": false },
                { "name": "limit", "description": "Maximum messages to review.", "required": false }
            ]
        },
        {
            "name": "summarize-thread",
            "title": "Summarize thread",
            "description": "Summarize a conversation and list open questions.",
            "arguments": [
                { "name": "thread_id", "description": "Thread identifier.", "required": true }
            ]
        },
        {
            "name": "draft-reply",
            "title": "Draft reply",
            "description": "Draft a reply to a cached message without sending it.",
            "arguments": [
                { "name": "message_id", "description": "Message identifier.", "required": true }
            ]
        },
        {
            "name": "weekly-review",
            "title": "Weekly review",
            "description": "Summarize account activity, unread mail and scheduled sends.",
            "arguments": []
        }
    ])
}

fn get_prompt(params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("prompt name is required")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let text = match name {
        "triage-inbox" => {
            let folder = args.get("folder").and_then(Value::as_str).unwrap_or("inbox");
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20);
            format!(
                "Triage up to {limit} messages in {folder}. Call mail_list first. For each unread message choose archive, reply, snooze, or leave it, explain the choice briefly, and only act after I confirm. Use mail_archive_newsletters for obvious newsletters."
            )
        }
        "summarize-thread" => {
            let id = args
                .get("thread_id")
                .and_then(Value::as_str)
                .ok_or("thread_id is required")?;
            format!(
                "Call thread_get for thread {id}, then summarize the decisions, unanswered questions and next action in under 150 words."
            )
        }
        "draft-reply" => {
            let id = args
                .get("message_id")
                .and_then(Value::as_str)
                .ok_or("message_id is required")?;
            format!(
                "Call mail_get for message {id}. Draft a concise reply in Markdown. Do not send it; show the draft to me and use draft_save only if I ask."
            )
        }
        "weekly-review" => "Call status, mailboxes_list, drafts_list, mailing_lists_list and signatures_list. Summarize unread mail, drafts, scheduled sends and account state in under 200 words.".to_string(),
        other => return Err(format!("unknown prompt: {other}")),
    };
    Ok(json!({
        "description": name,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }]
    }))
}

fn tools() -> Value {
    let mut tools = raw_tools();
    for tool in &mut tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        tool["title"] = json!(humanize(&name));
        tool["annotations"] = annotations(&name);
        if let Some(properties) = tool
            .get_mut("inputSchema")
            .and_then(|schema| schema.get_mut("properties"))
            .and_then(Value::as_object_mut)
        {
            for (key, property) in properties.iter_mut() {
                if property.get("title").is_none() {
                    property["title"] = json!(humanize(key));
                }
                if property.get("description").is_none() {
                    if let Some(description) = property_description(key) {
                        property["description"] = json!(description);
                    }
                }
            }
        }
    }
    Value::Array(tools)
}

fn raw_tools() -> Vec<Value> {
    let value = json!([
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
    ]);
    value.as_array().cloned().unwrap_or_default()
}

fn is_read_only(name: &str) -> bool {
    matches!(
        name,
        "status"
            | "mailboxes_list"
            | "identities"
            | "mail_list"
            | "mail_search"
            | "thread_get"
            | "mail_get"
            | "mail_get_remote"
            | "mail_unsubscribe_info"
            | "mail_from_sender_list"
            | "rules_list"
            | "contacts_list"
            | "attachment_list"
            | "templates_list"
            | "signatures_list"
            | "tags_list"
            | "mailing_lists_list"
            | "drafts_list"
            | "gpg_status"
            | "pgp_key_info"
            | "pgp_status"
            | "recipient_key_show"
    ) || name.ends_with("_list")
        || name.ends_with("_info")
        || name.ends_with("_status")
        || name.ends_with("_search")
}

fn is_mutating(name: &str) -> bool {
    !is_read_only(name)
}

fn is_destructive(name: &str) -> bool {
    matches!(
        name,
        "mail_delete"
            | "mail_empty_folder"
            | "mail_archive_newsletters"
            | "mailing_list_archive"
            | "mail_bulk_unsubscribe"
            | "mail_from_sender_archive"
            | "mail_from_sender_trash"
            | "mail_from_sender_move"
    ) || name.ends_with("_delete")
        || name.ends_with("_trash")
}

fn is_idempotent(name: &str) -> bool {
    matches!(
        name,
        "mail_set_read"
            | "mail_star"
            | "mail_pin"
            | "mail_spam"
            | "mail_restore"
            | "mail_move"
            | "mail_snooze"
            | "mail_unsnooze"
            | "tag_assign"
            | "tag_remove"
            | "folder_move"
            | "signature_set_default"
    )
}

fn is_open_world(name: &str) -> bool {
    [
        "send",
        "sync",
        "publish",
        "unsubscribe",
        "import",
        "fetch",
        "save",
        "export",
        "remote",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

fn annotations(name: &str) -> Value {
    json!({
        "readOnlyHint": is_read_only(name),
        "destructiveHint": is_destructive(name),
        "idempotentHint": is_idempotent(name),
        "openWorldHint": is_open_world(name)
    })
}

fn property_description(key: &str) -> Option<&'static str> {
    Some(match key {
        "ids" => "Message identifiers.",
        "id" => "Identifier of the message, folder, rule, template or signature.",
        "folder" => "Folder token or well-known folder name such as inbox, sent or trash.",
        "folder_token" => "Folder token.",
        "limit" => "Maximum number of results.",
        "query" => "Search text.",
        "name" => "Display name.",
        "address" => "Email address.",
        "addresses" => "Email addresses.",
        "subject" => "Subject line.",
        "markdown" => "Markdown body.",
        "message" => "Message body.",
        "to" => "Comma-separated recipients.",
        "cc" => "Comma-separated carbon-copy recipients.",
        "bcc" => "Comma-separated blind carbon-copy recipients.",
        "from" => "Sender address.",
        "path" => "Absolute filesystem path.",
        "read" => "True to mark read, false to mark unread.",
        "starred" => "True to star, false to unstar.",
        "pinned" => "True to pin, false to unpin.",
        "spam" => "True to mark as spam, false to mark as not spam.",
        "parent_token" => "Parent folder token.",
        "tag_token" => "Tag token.",
        "uid" => "Contact or record identifier.",
        "vcard" => "vCard body.",
        "seq" => "Attachment sequence number.",
        "rule" => "Rule definition object.",
        "order" => "Ordered rule identifiers.",
        "delay_seconds" => "Undo delay before the message is sent.",
        "scheduled_at" => "ISO timestamp for the scheduled send.",
        "until_iso8601" => "ISO timestamp for the snooze.",
        "content" => "Text content.",
        "category" => "Template category.",
        "fingerprint" => "Expected key fingerprint.",
        "kind" => "Target kind.",
        "list_id" => "Mailing list identifier.",
        "sender" => "Sender address.",
        "queue_id" => "Queued send identifier.",
        "thread_id" => "Thread identifier.",
        "attachments" => "Absolute attachment paths.",
        "message_id" => "Message identifier.",
        "expectedRevision" | "expected_revision" => {
            "Revision from a previous call; rejects stale mutations."
        }
        _ => return None,
    })
}

fn humanize(name: &str) -> String {
    let mut words = name.replace(['_', '.'], " ");
    if let Some(first) = words.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    words
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, value) in map {
                if is_secret_key(key) {
                    out.insert(key.clone(), json!("[redacted]"));
                } else {
                    out.insert(key.clone(), redact(value));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        Value::String(text) => {
            if text.len() > AUDIT_ARG_BYTES {
                Value::String(format!("{}… [truncated]", &text[..AUDIT_ARG_BYTES]))
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "passphrase",
        "api_key",
        "apikey",
        "authorization",
        "credential",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn audit(dir: &Path, message: &Value, response: &Value) {
    let tool = message
        .pointer("/params/name")
        .and_then(Value::as_str)
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
    let line = json!({ "ts": ts, "tool": tool, "args": redact(&args), "ok": ok });
    if let Ok(text) = serde_json::to_string(&line) {
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

fn json_response(value: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&value).unwrap_or_default();
    let mut response = Response::from_data(body);
    if let Ok(header) = Header::from_bytes("content-type", "application/json") {
        response.add_header(header);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(dir: &Path) -> State {
        let revision = Arc::new(AtomicU64::new(0));
        let call_revision = revision.clone();
        State {
            data_dir: dir.to_path_buf(),
            call: Arc::new(move |tool: &str, args: Value| match tool {
                "status" => Ok(json!({ "ok": true, "args": args })),
                "mail_get" => Ok(json!({ "id": args["id"] })),
                "mail_delete" => {
                    call_revision.fetch_add(1, Ordering::SeqCst);
                    Ok(json!({ "deleted": args["ids"] }))
                }
                "__revision" => Ok(json!({ "revision": call_revision.load(Ordering::SeqCst) })),
                _ => Ok(json!({ "tool": tool })),
            }),
            revision: AtomicU64::new(0),
            server_name: "test",
            server_version: "0",
        }
    }

    fn request(id: u64, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    #[test]
    fn registry_tools_are_unique_and_annotated() {
        let tools = tools();
        let tools = tools.as_array().unwrap();
        assert!(tools.len() > 60);
        let mut names = std::collections::HashSet::new();
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            assert!(names.insert(name.to_string()), "duplicate tool {name}");
            assert!(tool["title"]
                .as_str()
                .is_some_and(|title| !title.is_empty()));
            assert!(tool["annotations"]["readOnlyHint"].is_boolean());
            let properties = tool["inputSchema"]["properties"].as_object().unwrap();
            for (key, property) in properties {
                assert_eq!(property["title"], json!(humanize(key)));
                if property_description(key).is_some() {
                    assert!(property["description"].is_string(), "{name}.{key}");
                }
            }
        }
    }

    #[test]
    fn read_only_and_mutating_classification() {
        assert!(is_read_only("mail_list"));
        assert!(is_read_only("mail_get"));
        assert!(!is_read_only("mail_delete"));
        assert!(is_destructive("mail_delete"));
        assert!(!is_destructive("mail_star"));
        assert!(is_idempotent("mail_set_read"));
    }

    #[test]
    fn negotiates_supported_protocol_version() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());
        let response = handle(
            &state,
            &request(1, "initialize", json!({ "protocolVersion": "2024-11-05" })),
        )
        .unwrap();
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn notifications_get_no_response() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());
        assert!(handle(
            &state,
            &json!({
                "jsonrpc": "2.0", "method": "notifications/initialized"
            })
        )
        .is_none());
    }

    #[test]
    fn expected_revision_rejects_stale_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());
        let stale = handle(
            &state,
            &request(
                1,
                "tools/call",
                json!({ "name": "mail_delete", "arguments": { "ids": ["a"] }, "expectedRevision": 2 }),
            ),
        )
        .unwrap();
        assert!(stale["error"]["message"]
            .as_str()
            .unwrap()
            .contains("REVISION_CONFLICT"));
    }

    #[test]
    fn mutation_bumps_revision() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());
        let first = handle(
            &state,
            &request(
                1,
                "tools/call",
                json!({ "name": "mail_delete", "arguments": { "ids": ["a"] } }),
            ),
        )
        .unwrap();
        assert_eq!(first["result"]["_meta"]["revision"], 1);
        let second = handle(
            &state,
            &request(
                2,
                "tools/call",
                json!({ "name": "mail_delete", "arguments": { "ids": ["b"] }, "expectedRevision": 1 }),
            ),
        )
        .unwrap();
        assert_eq!(second["result"]["_meta"]["revision"], 2);
    }

    #[test]
    fn resources_read_status_and_revision() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());
        let status = read_resource(&state, &json!({ "uri": "aster://status" })).unwrap();
        assert_eq!(status["contents"][0]["mimeType"], "application/json");
        let revision = read_resource(&state, &json!({ "uri": "aster://revision" })).unwrap();
        assert_eq!(revision["contents"][0]["text"], "{\n  \"revision\": 0\n}");
    }

    #[test]
    fn settings_resource_redacts_secrets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"mcp":{"allow_send":true},"api_token":"nope"}"#,
        )
        .unwrap();
        let state = state(dir.path());
        let settings = read_resource(&state, &json!({ "uri": "aster://settings" })).unwrap();
        let text = settings["contents"][0]["text"].as_str().unwrap();
        assert!(!text.contains("nope"));
        assert!(text.contains("[redacted]"));
    }

    #[test]
    fn prompts_are_well_formed() {
        for prompt in prompt_list().as_array().unwrap() {
            assert!(prompt["name"].is_string());
            assert!(prompt["arguments"].is_array());
            let args = prompt["arguments"].as_array().unwrap();
            let mut values = Map::new();
            for argument in args {
                let name = argument["name"].as_str().unwrap();
                values.insert(name.to_string(), json!("x"));
            }
            let value = get_prompt(&json!({ "name": prompt["name"], "arguments": values }))
                .expect("prompt renders");
            assert_eq!(value["messages"][0]["role"], "user");
        }
    }

    #[test]
    fn audit_redacts_and_truncates() {
        let value = redact(&json!({
            "token": "secret",
            "body": "x".repeat(AUDIT_ARG_BYTES + 10)
        }));
        assert_eq!(value["token"], "[redacted]");
        assert!(value["body"].as_str().unwrap().ends_with("[truncated]"));
    }
}
