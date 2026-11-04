#!/usr/bin/env python3
"""End-to-end P0 checks against a running Aster Qt app using MCP."""
import json
import os
import sys
import datetime
import urllib.request

DATA = os.environ.get("ASTER_QT_DATA_DIR", os.path.expanduser("~/.local/share/com.astermail.qt"))
CFG = os.path.join(DATA, "mcp.json")
if not os.path.exists(CFG):
    print("FAIL: start Aster Qt first")
    raise SystemExit(1)
config = json.load(open(CFG))
URL = config["url"]
TOKEN = config["token"]
failures = []


def call(name, args=None):
    args = args or {}
    request = urllib.request.Request(
        URL,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                         "params": {"name": name, "arguments": args}}).encode(),
        headers={"Authorization": f"Bearer {TOKEN}", "Content-Type": "application/json"},
    )
    response = json.load(urllib.request.urlopen(request))
    result = response.get("result", {})
    return result.get("isError", False), (result.get("content") or [{}])[0].get("text", "")


def check(name, args=None):
    error, text = call(name, args)
    label = "FAIL" if error else "PASS"
    print(f"{label} {name}: {text[:120].replace(chr(10), ' ')}")
    if error:
        failures.append(name)
    return error, text


error, status_text = call("status")
try:
    DEFAULT_SENDER = json.loads(status_text).get("default_sender", "")
except Exception:
    DEFAULT_SENDER = ""
if not DEFAULT_SENDER:
    print("FAIL: status did not return a default sender")
    raise SystemExit(1)

for tool in ["status", "mailboxes_list", "mail_list", "mail_search", "drafts_list",
             "rules_list", "contacts_list", "pgp_key_info", "pgp_status", "gpg_status",
             "tags_list", "mailing_lists_list", "templates_list", "signatures_list"]:
    args = {"folder": "inbox", "limit": 1} if tool == "mail_list" else (
        {"query": "a", "limit": 1} if tool == "mail_search" else {})
    check(tool, args)

# tag lifecycle
error, text = check("tag_create", {"name": "QA P0 Tag"})
tag_token = None
if not error:
    tag_token = json.loads(text)["tag_token"]
    check("tags_list")
    check("tag_delete", {"tag_token": tag_token})

# folder lifecycle
error, text = check("mailbox_create", {"name": "QA P0 Folder A"})
folder_a = None
if not error:
    folder_a = json.loads(text)["token"]
    error2, text2 = check("mailbox_create", {"name": "QA P0 Folder B"})
    folder_b = None if error2 else json.loads(text2)["token"]
    if folder_b and folder_a:
        check("folder_move", {"folder_token": folder_b, "parent_token": folder_a})
        check("folder_rename", {"folder_token": folder_b, "name": "QA P0 Folder B2"})
        check("folder_delete", {"folder_token": folder_b})
    if folder_a:
        check("folder_delete", {"folder_token": folder_a})

# template lifecycle
error, text = check("template_create", {"name": "QA P0 Template", "category": "QA", "content": "Hello"})
if not error:
    check("templates_list")
    check("template_delete", {"id": json.loads(text)["id"]})

# signature lifecycle
error, text = check("signature_create", {"name": "QA P0 Signature", "content": "Regards"})
if not error:
    check("signatures_list")
    check("signature_delete", {"id": json.loads(text)["id"]})

for view in ["starred", "all", "scheduled"]:
    check("mail_list", {"folder": view, "limit": 3})

# message state lifecycle
error, text = check("mail_list", {"folder": "inbox", "limit": 1})
if not error:
    message = json.loads(text)[0]
    mid = message["id"]
    check("mail_pin", {"ids": [mid], "pinned": True})
    check("mail_pin", {"ids": [mid], "pinned": False})
    check("mail_set_read", {"ids": [mid], "read": not message["is_read"]})
    check("mail_set_read", {"ids": [mid], "read": message["is_read"]})
    check("mail_star", {"ids": [mid], "starred": not message["is_starred"]})
    check("mail_star", {"ids": [mid], "starred": message["is_starred"]})
    until = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(hours=1)).isoformat()
    check("mail_snooze", {"ids": [mid], "until_iso8601": until})
    check("mail_unsnooze", {"id": mid})
    check("mail_spam", {"ids": [mid], "spam": True})
    check("mail_restore", {"ids": [mid]})
    check("mail_from_sender_list", {"sender": message["sender"][:12], "limit": 3})

# queue and cancel
error, text = check("mail_send_queued", {"to": DEFAULT_SENDER, "subject": "QA P0 queue",
                                         "markdown": "queue cancel test", "delay_seconds": 60})
if not error:
    check("mail_cancel_queued", {"queue_id": json.loads(text)["queue_id"]})

check("mail_bulk_unsubscribe", {"ids": ["does-not-exist"]})

print()
if failures:
    print(f"FAILED: {len(failures)} check(s): {', '.join(failures)}")
    raise SystemExit(1)
print("All P0 checks passed")
