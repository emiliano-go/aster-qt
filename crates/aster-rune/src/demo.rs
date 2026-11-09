// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic sample data for headless snapshots and UI tests.

use aster_client_core::service::{
    CachedAttachmentView, ExternalContentReport, MailboxView, MessageDetail, MessageView,
    SendIdentityView, SignatureView, TemplateView,
};

use crate::state::AppState;

pub fn scene(state: &mut AppState) {
    state.account = "you@astermail.org".into();
    state.status = "Ready".into();
    state.mailboxes = vec![
        mailbox("inbox", "Inbox", "Inbox", 3),
        mailbox("sent", "Sent", "Sent", 0),
        mailbox("archive", "Archive", "Archive", 0),
    ];
    state.counts = serde_json::json!({
        "inbox": { "total": 8, "unread": 3 },
        "starred": { "total": 2, "unread": 0 },
        "all": { "total": 8, "unread": 3 },
        "sent": { "total": 0, "unread": 0 },
        "drafts": { "total": 0, "unread": 0 },
        "archive": { "total": 0, "unread": 0 },
        "spam": { "total": 0, "unread": 0 },
        "trash": { "total": 0, "unread": 0 },
    });
    state.messages = vec![
        message(
            "m1",
            "Mara Velazquez",
            "Q4 planning is on Thursday",
            "I moved the planning session to Thursday 10:00 so the whole team can join. Agenda is in the shared doc.",
            "2026-11-08T18:42:00Z",
            false,
            true,
        ),
        message(
            "m2",
            "Aster Mail",
            "Your weekly digest",
            "Seven conversations, two mentions and one new mailing list this week.",
            "2026-11-08T15:10:00Z",
            false,
            false,
        ),
        message(
            "m3",
            "Lucas Perdomo",
            "Re: invoice for October",
            "Attached is the signed copy. Let me know if the PO number matches on your side.",
            "2026-11-08T11:02:00Z",
            true,
            false,
        ),
        message(
            "m4",
            "Nadia Rahman",
            "Design review notes",
            "The spacing pass looks good. Two remaining nits: the empty state copy and the focus ring on dark.",
            "2026-11-07T20:31:00Z",
            true,
            true,
        ),
        message(
            "m5",
            "GitHub",
            "[aster] Weekly activity",
            "3 pull requests merged, 12 commits, 2 issues closed.",
            "2026-11-07T09:00:00Z",
            true,
            false,
        ),
        message(
            "m6",
            "Tomás Oliveira",
            "Lunch on Saturday?",
            "Ramen place near the market, around 13:00. Bring the book you mentioned.",
            "2026-11-06T22:18:00Z",
            true,
            false,
        ),
        message(
            "m7",
            "Aster Privacy",
            "Security bulletin",
            "A routine dependency update shipped today. No action is needed on your account.",
            "2026-11-05T08:45:00Z",
            true,
            false,
        ),
        message(
            "m8",
            "Lucía Ferrari",
            "Photos from the hike",
            "Finally sorted them. The ones from the ridge came out much better than expected.",
            "2026-11-04T17:12:00Z",
            true,
            false,
        ),
    ];
    state.identities = vec![SendIdentityView {
        address: "you@astermail.org".into(),
        kind: "primary".into(),
        display_name: Some("You".into()),
        is_default: true,
    }];
    state.signatures = vec![SignatureView {
        id: "sig-1".into(),
        name: "Default".into(),
        content: "Emiliano\nAster Mail".into(),
        is_html: false,
        is_default: true,
        alias_id: None,
    }];
    state.templates = vec![TemplateView {
        id: "tpl-1".into(),
        name: "Intro".into(),
        category: "General".into(),
        content: "Hi,\n\nThanks for the note.".into(),
        sort_order: 0,
    }];
    state.rules = serde_json::json!({"rules": [{
        "id": "rule-1",
        "name": "Newsletters to Reading",
        "enabled": true,
        "conditions": [{"field": "from", "op": "contains", "value": "newsletter"}],
        "actions": [{"type": "move_to", "folder_token": "reading"}]
    }]});
    state.aliases = serde_json::json!([{
        "id": "alias-1",
        "address": "shop@astermail.org",
        "enabled": true
    }]);
    state.contacts = serde_json::json!([{
        "uid": "contact-1",
        "vcard": "BEGIN:VCARD\nVERSION:3.0\nFN:Mara Velazquez\nEMAIL:mara@example.org\nEND:VCARD\n"
    }]);
    state.pgp = serde_json::json!({
        "fingerprint": "ABCD 1234 EF56 7890",
        "address": "you@astermail.org",
        "published_wkd": true
    });
    state.gpg = serde_json::json!({"available": true, "agent_running": true});
    state.lists = serde_json::json!([{
        "list_id": "dev@lists.example.org",
        "name": "Dev list",
        "total": 3,
        "unread": 1
    }]);
    state.tags = serde_json::json!([{
        "tag_token": "tag-receipts",
        "name": "Receipts",
        "keyword": "Receipts",
        "position": 0
    }]);
    state.outbox = serde_json::json!([{
        "id": 7,
        "envelope_from": "you@astermail.org",
        "envelope_to": "mara@example.org",
        "queued_at": 0,
        "attempts": 1,
        "last_error": null,
        "status": "queued"
    }]);
    state.outbox_pending = 1;
    state.diagnostics = serde_json::json!({
        "data_dir": "/home/you/.local/share/com.astermail.qt",
        "signed_in": true,
        "outbox_pending": 0
    });
    state.cursor = 0;
    state.selected = Some("m1".into());
    let mut first = detail(state.messages[0].clone());
    first.is_html = true;
    first.body_html = SAMPLE_HTML.into();
    first.external_content = ExternalContentReport {
        blocked_count: 2,
        has_remote_images: true,
        ..Default::default()
    };
    first.attachments = vec![CachedAttachmentView {
        seq: 1,
        name: "agenda.pdf".into(),
        content_type: "application/pdf".into(),
        content_id: None,
        is_inline: false,
        size: 182_000,
        is_cached: true,
    }];
    state.detail = Some(first);
    state.thread = state.messages[..3].to_vec();
    state.refresh_visible();
}

/// Demo data, then the HTML in `ASTER_MAILHTML_FIXTURE` if set (local QA only).
pub fn fixture_scene(state: &mut AppState) {
    scene(state);
    if let Ok(path) = std::env::var("ASTER_MAILHTML_FIXTURE") {
        if let Ok(html) = std::fs::read_to_string(path) {
            if let Some(detail) = &mut state.detail {
                detail.is_html = true;
                detail.body_html = html;
                detail.body_html_dark = None;
            }
        }
    }
}

pub fn filters_scene(state: &mut AppState) {
    scene(state);
    state.filters_open = true;
}

pub fn snooze_scene(state: &mut AppState) {
    scene(state);
    state.snooze_targets = vec!["m1".into()];
    state.snooze_open = true;
}

pub fn delete_scene(state: &mut AppState) {
    scene(state);
    state.delete_targets = vec!["m1".into()];
    state.delete_open = true;
}

pub fn preview_scene(state: &mut AppState) {
    scene(state);
    state.preview_text = "Attachment preview\n\nThis is a text preview.".into();
    state.preview_open = true;
}

pub fn outbox_scene(state: &mut AppState) {
    scene(state);
    state.outbox_open = true;
}

/// Demo data with the command palette open.
pub fn command_scene(state: &mut AppState) {
    scene(state);
    state.command_open = true;
}

/// Demo data with a message context menu open.
pub fn menu_scene(state: &mut AppState) {
    scene(state);
    state.menu_message = Some("m1".into());
    state.menu_point = Some(rune::PointerPress { x: 640.0, y: 220.0 });
}

/// Demo data with Settings open on `tab`.
pub fn settings_scene(state: &mut AppState, tab: crate::state::SettingsTab) {
    scene(state);
    state.open_settings(tab);
}

/// Demo data with the composer open.
pub fn compose_scene(state: &mut AppState) {
    scene(state);
    state.open_new_mail();
    state.compose.to = rune::TextEdit::new("mara@example.org");
    state.compose.subject = rune::TextEdit::new("Q4 planning notes");
    state.compose.body = rune::TextEdit::new(
        "Hi Mara,\n\nHere are the notes from today. I added the hiring plan and the open questions.\n\n-- \nEmiliano\nAster Mail",
    );
}

const SAMPLE_HTML: &str = r#"
<h2>Planning on Thursday</h2>
<p>I moved the planning session to <b>Thursday 10:00</b> so the whole team can join.
The agenda is in the <a href="https://example.test/doc">shared doc</a>.</p>
<p style="color:#8a6d3b">Please add your topics before Wednesday.</p>
<ul>
  <li>Roadmap check-in</li>
  <li>Q4 hiring plan</li>
  <li><i>Open questions</i> from the last retro</li>
</ul>
<blockquote>The room is booked for 90 minutes.</blockquote>
<table>
  <tr><th>Item</th><th>Owner</th></tr>
  <tr><td>Roadmap</td><td>Mara</td></tr>
  <tr><td>Hiring</td><td>Lucas</td></tr>
</table>
<pre>agenda.md
  1. roadmap
  2. hiring</pre>
<hr>
<p><small>Sent from Aster</small></p>
"#;

fn mailbox(id: &str, name: &str, label: &str, unread: u32) -> MailboxView {
    MailboxView {
        id: id.into(),
        name: name.into(),
        parent_id: None,
        role: Some(id.into()),
        sort_order: 0,
        folder_label: label.into(),
        token: Some(id.into()),
        total: 8,
        unread,
    }
}

#[allow(clippy::too_many_arguments)]
fn message(
    id: &str,
    sender: &str,
    subject: &str,
    preview: &str,
    date: &str,
    read: bool,
    starred: bool,
) -> MessageView {
    MessageView {
        id: id.into(),
        folder: "inbox".into(),
        subject: subject.into(),
        sender: sender.into(),
        recipients: "you@astermail.org".into(),
        date: Some(date.into()),
        size: 8192,
        flags: 0,
        is_read: read,
        is_starred: starred,
        has_attachments: false,
        list_id: None,
        list_name: None,
        list_unsubscribe: None,
        list_unsubscribe_post: None,
        is_pinned: false,
        category: None,
        message_id: Some(format!("<{id}@astermail.org>")),
        in_reply_to: None,
        references: None,
        thread_id: Some(id.into()),
        tags: Vec::new(),
        snoozed_until: None,
        cc: None,
        bcc: None,
        reply_to: None,
        routing_token: None,
        spf_result: Some("pass".into()),
        dkim_result: Some("pass".into()),
        dmarc_result: Some("pass".into()),
        sender_verified: true,
        sender_verified_domain: Some("astermail.org".into()),
        sender_aligned: true,
        phishing_level: None,
        system_origin: false,
        scheduled_at: None,
        send_status: None,
        expires_at: None,
        expiry_type: None,
        preview: preview.into(),
    }
}

fn detail(message: MessageView) -> MessageDetail {
    let body = format!("Hi,\n\n{}\n\n— {}\n", message.preview, message.sender);
    MessageDetail {
        message,
        body,
        is_html: false,
        body_html: String::new(),
        body_html_dark: None,
        remote_content_loaded: false,
        external_content: ExternalContentReport::default(),
        sender_unverified: false,
        not_end_to_end_encrypted: false,
        attachments_unverified: false,
        attachments: Vec::new(),
    }
}
