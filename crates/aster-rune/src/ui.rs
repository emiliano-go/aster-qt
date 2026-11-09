// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Rune view: shell, sidebar, message list and reader.

use aster_client_core::settings as app_settings;
use aster_mailhtml::{render as render_mail, MailStyle};
use rune::{
    col, el, icon, row, text, Color, Cursor, Cx, El, Icon, Key, KeyEvent, ScrollSpec, TextStyle, Tx,
};
use rune_kit::{context_menu, Dialog, MenuItem};
use rune_kit::{Button, ButtonVariant, Kit, TextArea, TextInput};
use rune_kit::{ControlSize, IconButton, IconButtonVariant};

use crate::action::Action;
use crate::icons;
use crate::state::{AppState, SettingsTab};

const VIEWS: &[(&str, &str)] = &[
    ("inbox", "Inbox"),
    ("starred", "Starred"),
    ("all", "All Mail"),
    ("scheduled", "Scheduled"),
    ("snoozed", "Snoozed"),
    ("sent", "Sent"),
    ("drafts", "Drafts"),
    ("archive", "Archive"),
    ("spam", "Spam"),
    ("trash", "Trash"),
];

pub fn view(s: &AppState, _cx: &mut Cx<AppState>) -> El {
    let k = s.theme.kit();

    let base = col()
        .key("window")
        .w_full()
        .h_full()
        .fill(k.c.page)
        .child(topbar(s, &k))
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .child(sidebar_pane(s, &k))
                .child(split_handle(
                    "sidebar-split",
                    &k,
                    Cursor::ResizeColumn,
                    |s: &mut AppState, drag: &mut rune::Drag| match drag.phase {
                        rune::Phase::Down => drag.data = s.sidebar_width,
                        rune::Phase::Move => {
                            s.sidebar_width = (drag.data + drag.dx).clamp(208.0, 420.0);
                        }
                        _ => {}
                    },
                ))
                .child(list_pane(s, &k))
                .child(if s.reader_open {
                    split_handle(
                        "reader-split",
                        &k,
                        Cursor::ResizeColumn,
                        |s: &mut AppState, drag: &mut rune::Drag| match drag.phase {
                            rune::Phase::Down => drag.data = s.reader_width,
                            rune::Phase::Move => {
                                s.reader_width = (drag.data - drag.dx).clamp(300.0, 760.0);
                            }
                            _ => {}
                        },
                    )
                } else {
                    el().w(0.0)
                })
                .child(if s.reader_open {
                    reader_pane(s, &k)
                } else {
                    el().w(0.0)
                }),
        )
        .child(statusbar(s, &k));
    let mut layer = Vec::new();
    overlays(s, &k, &mut layer);
    if s.settings_open {
        settings_dialog(s, &k, &mut layer);
    }
    if s.compose.requested {
        compose_dialog(s, &k, &mut layer);
    }
    rune_kit::root(base, layer)
}

fn split_handle(
    key: &str,
    k: &Kit,
    cursor: Cursor,
    on_drag: impl Fn(&mut AppState, &mut rune::Drag) + 'static,
) -> El {
    el().key(key)
        .w(14.0)
        .h_full()
        .center()
        .cursor(cursor)
        .hover(k.c.hover)
        .on_drag(move |s: &mut AppState, drag: &mut rune::Drag, _| on_drag(s, drag))
        .child(el().w(1.0).h_full().fill(k.c.border_muted))
}

fn topbar(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let sidebar = s.theme.token_color("sidebar", c.page);
    let search = TextInput::new(&s.query)
        .label("Search mail")
        .width(Some(360.0))
        .height(36.0)
        .placeholder("Search mail")
        .leading_icon(&icons::SEARCH)
        .clear_button()
        .on_edit(|s: &mut AppState| &mut s.query)
        .build(k)
        .key("search");
    row()
        .key("topbar")
        .w_full()
        .h(56.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .fill(sidebar)
        .child(
            el().w(28.0)
                .h(28.0)
                .radius(k.radius.sm)
                .center()
                .fill(c.bg_contrast)
                .child(icon(&icons::SEND, 16.0).color(c.text_contrast)),
        )
        .child(
            col()
                .gap(0.0)
                .child(
                    text("Aster")
                        .font(TextStyle::new(18.0, 600.0))
                        .color(c.text_base),
                )
                .child(
                    text(if s.account.is_empty() {
                        "Aster Mail"
                    } else {
                        s.account.as_str()
                    })
                    .font(TextStyle::new(11.0, 400.0))
                    .color(c.text_muted),
                ),
        )
        .child(search)
        .child(
            IconButton::new(&icons::FILTER)
                .label("Search filters")
                .variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Large)
                .on_press(|s: &mut AppState| s.filters_open = true)
                .build(k)
                .size(36.0, 36.0),
        )
        .child(el().grow(1.0))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(el().w(8.0).h(8.0).radius(4.0).fill(if s.status == "Ready" {
                    c.success.fg
                } else {
                    c.warning.fg
                }))
                .child(
                    text(if s.status == "Ready" {
                        "Synced"
                    } else {
                        s.status.as_str()
                    })
                    .font(TextStyle::new(12.0, 400.0))
                    .color(c.text_muted),
                ),
        )
        .child(
            IconButton::new(&icons::SETTINGS)
                .label("Settings")
                .variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Large)
                .on_press(|s: &mut AppState| s.open_settings(SettingsTab::Appearance))
                .build(k)
                .size(36.0, 36.0),
        )
        .child(
            Button::new("Compose")
                .icon(&icons::PLUS)
                .variant(ButtonVariant::Contrast)
                .control_size(ControlSize::ExtraLarge)
                .on_press(|s: &mut AppState| s.open_new_mail())
                .build(k)
                .key("compose-open"),
        )
}

fn statusbar(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let sidebar = s.theme.token_color("sidebar", c.page);
    let mut bar = row()
        .key("statusbar")
        .w_full()
        .h(30.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .fill(sidebar);
    if s.status != "Ready" {
        bar = bar.child(
            text(s.status.as_str())
                .font(TextStyle::new(11.0, 400.0))
                .color(c.text_muted)
                .truncate(),
        );
    }
    bar = bar.child(el().grow(1.0));
    if s.outbox_pending > 0 {
        let count = s.outbox_pending;
        bar = bar.child(
            text(format!("Outbox ({count})"))
                .font(TextStyle::new(11.0, 400.0))
                .color(c.text_accent)
                .cursor(Cursor::Pointer)
                .on_press(|s: &mut AppState| {
                    s.outbox_open = true;
                    s.send(Action::LoadOutbox);
                }),
        );
    }
    bar.child(
        text("MCP idle")
            .font(TextStyle::new(11.0, 400.0))
            .color(c.text_muted),
    )
}

fn sidebar_pane(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let sidebar = s.theme.token_color("sidebar", c.page);
    let selected_color = s.theme.token_color("selected", c.layer_03);
    let card = s.theme.token_color("card", c.surface_raised);
    let unread_total: u32 = s
        .mailboxes
        .iter()
        .filter(|mailbox| mailbox.folder_label != "sent" && mailbox.folder_label != "drafts")
        .map(|mailbox| mailbox.unread)
        .sum();
    let initial = s
        .account
        .chars()
        .next()
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "A".into());

    let account = row()
        .key("account-switcher")
        .w_full()
        .h(56.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(k.radius.lg)
        .fill(card)
        .hover(c.hover)
        .on_press(|s: &mut AppState| s.open_settings(SettingsTab::Aliases))
        .child(
            el().w(32.0)
                .h(32.0)
                .radius(16.0)
                .center()
                .fill(c.bg_contrast)
                .child(
                    text(initial)
                        .font(TextStyle::new(14.0, 600.0))
                        .color(c.text_contrast),
                ),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(1.0)
                .child(
                    text(if s.account.is_empty() {
                        "Aster Mail"
                    } else {
                        s.account.as_str()
                    })
                    .font(TextStyle::new(12.0, 600.0))
                    .color(c.text_base)
                    .truncate(),
                )
                .child(
                    text(format!("{unread_total} unread"))
                        .font(TextStyle::new(11.0, 400.0))
                        .color(c.text_muted),
                ),
        )
        .child(icon(&icons::CHEVRON_DOWN, 16.0).color(c.text_muted));

    let mut column = col()
        .key("sidebar")
        .w_full()
        .gap(4.0)
        .padding(12.0)
        .child(account)
        .child(section_label("Mailboxes", k));

    let system: [(&str, &str, &Icon); 10] = [
        ("inbox", "Inbox", &icons::INBOX),
        ("starred", "Starred", &icons::STAR),
        ("all", "All Mail", &icons::INBOX),
        ("scheduled", "Scheduled", &icons::CLOCK),
        ("snoozed", "Snoozed", &icons::MOON),
        ("sent", "Sent", &icons::SEND),
        ("drafts", "Drafts", &icons::DRAFTS),
        ("archive", "Archive", &icons::ARCHIVE),
        ("spam", "Spam", &icons::JUNK),
        ("trash", "Trash", &icons::TRASH),
    ];
    for (id, label, glyph) in system {
        let selected = s.folder == id;
        let unread = count(s, id, "unread");
        let folder = id.to_string();
        column = column.child(nav_row(
            k,
            label,
            glyph,
            unread as u32,
            selected,
            selected_color,
            move |s: &mut AppState| s.open_folder(folder.clone()),
        ));
    }

    for (label, glyph, action) in [
        ("Contacts", &icons::INBOX, 0u8),
        ("Calendar", &icons::CLOCK, 1),
        ("Subscriptions", &icons::TAG, 2),
    ] {
        let click: Box<dyn Fn(&mut AppState)> = match action {
            0 => Box::new(|s: &mut AppState| s.open_settings(SettingsTab::Contacts)),
            1 => Box::new(|_: &mut AppState| {
                let _ = std::process::Command::new("xdg-open")
                    .arg("https://calendar.astermail.org")
                    .spawn();
            }),
            _ => Box::new(|s: &mut AppState| {
                s.filters_open = true;
            }),
        };
        column = column.child(nav_row_boxed(
            k,
            label,
            glyph,
            0,
            false,
            selected_color,
            click,
        ));
    }

    column = column
        .child(el().w_full().h(1.0).mb(4.0).mt(4.0).fill(c.border_muted))
        .child(
            row()
                .w_full()
                .h(32.0)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .hover(c.hover)
                .on_press(|s: &mut AppState| s.open_settings(SettingsTab::Aliases))
                .child(
                    text("Custom")
                        .font(TextStyle::new(11.0, 500.0))
                        .color(c.text_muted)
                        .grow(1.0),
                )
                .child(icon(&icons::PLUS, 16.0).color(c.text_accent)),
        );

    for mailbox in &s.mailboxes {
        if VIEWS.iter().any(|(id, _)| *id == mailbox.folder_label) {
            continue;
        }
        let folder = mailbox.folder_label.clone();
        let selected = s.folder == mailbox.id;
        column = column.child(nav_row(
            k,
            mailbox.name.as_str(),
            &icons::FOLDER,
            mailbox.unread,
            selected,
            selected_color,
            move |s: &mut AppState| s.open_folder(folder.clone()),
        ));
    }

    column = column
        .child(section_label("Mailing lists", k))
        .child(nav_row(
            k,
            "All mail",
            &icons::TAG,
            0,
            s.filters.list_id.text.is_empty(),
            selected_color,
            |s: &mut AppState| {
                s.filters.list_id = rune::TextEdit::new("");
                s.refresh_visible();
            },
        ));
    for list in s.lists.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let id = list
            .get("list_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let name = list
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Mailing list")
            .to_string();
        let unread = list
            .get("unread")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32;
        let filter = id.clone();
        let selected = s.filters.list_id.text == id;
        column = column.child(nav_row(
            k,
            name.as_str(),
            &icons::TAG,
            unread,
            selected,
            selected_color,
            move |s: &mut AppState| {
                s.filters.list_id = rune::TextEdit::new(filter.clone());
                s.refresh_visible();
            },
        ));
    }

    if let Some(aliases) = s.aliases.as_array() {
        if !aliases.is_empty() {
            column = column.child(section_label("Aliases", k));
            for alias in aliases {
                let address = alias
                    .get("address")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let enabled = alias
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true);
                let folder = format!("alias:{address}");
                let selected = s.folder == folder;
                let mut item = nav_row(
                    k,
                    address.as_str(),
                    &icons::SEND,
                    0,
                    selected,
                    selected_color,
                    move |s: &mut AppState| s.open_folder(folder.clone()),
                );
                if !enabled {
                    item = item.opacity(0.55);
                }
                column = column.child(item);
            }
        }
    }

    column = column.child(section_label("Tags", k)).child(nav_row(
        k,
        "All tags",
        &icons::TAG,
        0,
        s.filters.tag.text.is_empty(),
        selected_color,
        |s: &mut AppState| {
            s.filters.tag = rune::TextEdit::new("");
            s.refresh_visible();
        },
    ));
    for tag in s.tags.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let token = tag
            .get("tag_token")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let name = tag
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Tag")
            .to_string();
        let filter = token.clone();
        let selected = s.filters.tag.text == token;
        column = column.child(nav_row(
            k,
            name.as_str(),
            &icons::TAG,
            0,
            selected,
            selected_color,
            move |s: &mut AppState| {
                s.filters.tag = rune::TextEdit::new(filter.clone());
                s.refresh_visible();
            },
        ));
    }

    let searches = s
        .settings
        .pointer("/mail/saved_searches")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    column = column.child(section_label("Saved searches", k));
    if searches.is_empty() {
        column = column.child(
            text("Save a search from the filter dialog.")
                .font(TextStyle::new(9.0, 400.0))
                .color(c.text_muted)
                .wrap(),
        );
    }
    for search in searches {
        let value = search.clone();
        let name = search
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Saved search")
            .to_string();
        column = column.child(
            row()
                .w_full()
                .h(28.0)
                .px(12.0)
                .center_y()
                .radius(k.radius.sm)
                .hover(c.hover)
                .on_press(move |s: &mut AppState| s.apply_saved_search(&value))
                .child(
                    text(name)
                        .font(TextStyle::new(12.0, 400.0))
                        .color(c.text_base)
                        .truncate(),
                ),
        );
    }

    column = column
        .child(el().grow(1.0))
        .child(el().w_full().h(1.0).fill(c.border_muted).mb(4.0).mt(4.0));
    let rule_count = rules_of(&s.rules).len();
    column = column.child(
        row()
            .w_full()
            .h(30.0)
            .px(8.0)
            .gap(8.0)
            .center_y()
            .hover(c.hover)
            .on_press(|s: &mut AppState| s.open_settings(SettingsTab::Rules))
            .child(icon(&icons::RULE, 16.0).color(c.text_muted))
            .child(
                text("Mail rules")
                    .font(TextStyle::new(12.0, 400.0))
                    .color(c.text_base)
                    .grow(1.0),
            )
            .child(
                text(rule_count.to_string())
                    .font(TextStyle::new(11.0, 400.0))
                    .color(c.text_muted),
            ),
    );

    col()
        .w(s.sidebar_width)
        .h_full()
        .min_h(0.0)
        .fill(sidebar)
        .child(
            col()
                .key("sidebar-scroll")
                .w_full()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(column),
        )
}

fn section_label(label: &str, k: &Kit) -> El {
    text(label)
        .font(TextStyle::new(11.0, 500.0))
        .color(k.c.text_muted)
        .mt(12.0)
        .ml(8.0)
}

#[allow(clippy::too_many_arguments)]
fn nav_row(
    k: &Kit,
    label: &str,
    glyph: &'static Icon,
    unread: u32,
    selected: bool,
    selected_color: Color,
    action: impl Fn(&mut AppState) + 'static,
) -> El {
    nav_row_boxed(
        k,
        label,
        glyph,
        unread,
        selected,
        selected_color,
        Box::new(action),
    )
}

#[allow(clippy::too_many_arguments)]
fn nav_row_boxed(
    k: &Kit,
    label: &str,
    glyph: &'static Icon,
    unread: u32,
    selected: bool,
    selected_color: Color,
    action: Box<dyn Fn(&mut AppState)>,
) -> El {
    let c = &k.c;
    let ink = if selected {
        c.text_accent
    } else {
        c.text_muted
    };
    let slug = label
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-");
    row()
        .key(format!("nav-{slug}"))
        .role("button")
        .label(label)
        .w_full()
        .h(32.0)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(k.radius.sm)
        .fill(if selected {
            selected_color
        } else {
            Color::TRANSPARENT
        })
        .hover(c.hover)
        .on_press(move |s: &mut AppState| action(s))
        .child(icon(glyph, 16.0).color(ink))
        .child(
            text(label)
                .font(TextStyle::new(14.0, if selected { 600.0 } else { 400.0 }))
                .color(c.text_base)
                .truncate()
                .grow(1.0),
        )
        .child(
            text(if unread > 0 {
                unread.to_string()
            } else {
                String::new()
            })
            .font(TextStyle::new(12.0, 500.0))
            .color(ink)
            .w(22.0),
        )
}

fn list_pane(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let mut header = row().w_full().h(56.0).px(16.0).gap(8.0).center_y().child(
        col()
            .gap(1.0)
            .grow(1.0)
            .min_w(0.0)
            .child(
                text(folder_label(s))
                    .font(TextStyle::new(16.0, 600.0))
                    .color(c.text_base)
                    .truncate(),
            )
            .child(
                text(format!("{} messages", s.visible.len()))
                    .font(TextStyle::new(11.0, 400.0))
                    .color(c.text_muted),
            ),
    );
    if !s.filters.list_id.text.is_empty() {
        let list = s.filters.list_id.text.clone();
        header = header
            .child(
                text("Archive all")
                    .font(TextStyle::new(12.0, 400.0))
                    .color(c.text_accent)
                    .cursor(Cursor::Pointer)
                    .on_press(move |s: &mut AppState| {
                        s.send(Action::ArchiveMailingList(list.clone()))
                    }),
            )
            .child(
                text("All mail")
                    .font(TextStyle::new(12.0, 400.0))
                    .color(c.text_accent)
                    .cursor(Cursor::Pointer)
                    .on_press(|s: &mut AppState| {
                        s.filters.list_id = rune::TextEdit::new("");
                        s.refresh_visible();
                    }),
            );
    }
    header = header
        .child(
            IconButton::new(&icons::INBOX)
                .label("Unread only")
                .variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Regular)
                .on_press(|s: &mut AppState| {
                    s.filters.unread_only = !s.filters.unread_only;
                    s.refresh_visible();
                })
                .build(k)
                .size(28.0, 28.0),
        )
        .child(
            IconButton::new(&icons::CHECK)
                .label("Select messages")
                .variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Regular)
                .on_press(|s: &mut AppState| {
                    s.selection_mode = !s.selection_mode;
                    if !s.selection_mode {
                        s.selection.clear();
                    }
                })
                .build(k)
                .size(28.0, 28.0),
        )
        .child(
            IconButton::new(&icons::REFRESH)
                .label("Refresh")
                .variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Regular)
                .on_press(|s: &mut AppState| s.send(Action::Refresh))
                .build(k)
                .size(28.0, 28.0),
        );

    let chips = row()
        .w_full()
        .h(30.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .child(chip_button(
            "Unread",
            s.filters.unread_only,
            k,
            |s: &mut AppState| {
                s.filters.unread_only = !s.filters.unread_only;
                s.refresh_visible();
            },
        ))
        .child(chip_button(
            "Starred",
            s.filters.starred,
            k,
            |s: &mut AppState| {
                s.filters.starred = !s.filters.starred;
                s.refresh_visible();
            },
        ))
        .child(chip_button(
            "Attachments",
            s.filters.attachments,
            k,
            |s: &mut AppState| {
                s.filters.attachments = !s.filters.attachments;
                s.refresh_visible();
            },
        ))
        .child(el().grow(1.0));
    let chips = if s.filters.unread_only || s.filters.starred || s.filters.attachments {
        chips.child(
            text("Clear chips")
                .font(TextStyle::new(12.0, 400.0))
                .color(c.text_accent)
                .cursor(Cursor::Pointer)
                .on_press(|s: &mut AppState| {
                    s.filters.unread_only = false;
                    s.filters.starred = false;
                    s.filters.attachments = false;
                    s.refresh_visible();
                }),
        )
    } else {
        chips
    };

    let mut body = col()
        .key("messages")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .fill(c.page);
    if s.visible.is_empty() {
        body = body.child(
            col()
                .w_full()
                .h_full()
                .center()
                .gap(8.0)
                .child(icon(&icons::INBOX, 32.0).color(c.text_muted))
                .child(
                    text(if !s.query.text.is_empty() || s.filters.unread_only {
                        "No matching mail"
                    } else {
                        "This mailbox is empty"
                    })
                    .font(TextStyle::new(14.0, 400.0))
                    .color(c.text_base),
                ),
        );
    } else {
        let mut rows = Vec::with_capacity(s.visible.len() * 2);
        for (index, item) in s.visible.iter().enumerate() {
            if index > 0 {
                rows.push(
                    el().w_full()
                        .h(1.0)
                        .fill(s.theme.token_color("message-divider", c.border_muted)),
                );
            }
            rows.push(message_row(s, k, &item.message, index, item.thread_count));
        }
        body = body.child(
            col()
                .w_full()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(rows),
        );
    }

    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .fill(c.page)
        .child(header)
        .child(el().w_full().h(1.0).fill(c.border_muted))
        .child(chips)
        .child(el().w_full().h(1.0).fill(c.border_muted))
        .child(body)
}

fn chip_button(label: &str, active: bool, k: &Kit, action: impl Fn(&mut AppState) + 'static) -> El {
    Button::new(label)
        .variant(if active {
            ButtonVariant::Contrast
        } else {
            ButtonVariant::Ghost
        })
        .control_size(ControlSize::Small)
        .on_press(action)
        .build(k)
}

fn reader_pane(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let surface = c.layer_01;
    let Some(detail) = &s.detail else {
        return col()
            .key("reader")
            .w(s.reader_width)
            .h_full()
            .fill(surface)
            .center()
            .gap(8.0)
            .child(icon(&icons::INBOX, 32.0).color(c.text_muted))
            .child(
                text("Select a message")
                    .font(TextStyle::new(14.0, 400.0))
                    .color(c.text_base),
            );
    };
    let message = &detail.message;
    let id = message.id.clone();
    let mut header = row()
        .key("reader-actions")
        .w_full()
        .h(56.0)
        .px(16.0)
        .gap(2.0)
        .center_y();
    if s.folder == "scheduled" {
        let send_id = id.clone();
        let cancel_id = id.clone();
        header = header
            .child(link_action(
                "Send now",
                c.text_accent,
                k,
                move |s: &mut AppState| s.send(Action::SendScheduledNow(send_id.clone())),
            ))
            .child(link_action(
                "Cancel",
                c.danger.fg,
                k,
                move |s: &mut AppState| s.send(Action::CancelScheduled(cancel_id.clone())),
            ));
    }
    if message.snoozed_until.is_some() {
        let snooze_id = id.clone();
        header = header.child(link_action(
            "Unsnooze",
            c.text_accent,
            k,
            move |s: &mut AppState| s.send(Action::UnsnoozeMessage(snooze_id.clone())),
        ));
    }
    let icon_button = |glyph: &'static Icon, label: &str, action: Box<dyn Fn(&mut AppState)>| {
        IconButton::new(glyph)
            .label(label)
            .variant(IconButtonVariant::Ghost)
            .control_size(ControlSize::Regular)
            .on_press(move |s: &mut AppState| action(s))
            .build(k)
            .size(28.0, 28.0)
    };
    let snooze_ids = vec![id.clone()];
    header = header
        .child(icon_button(
            &icons::CLOCK,
            "Snooze",
            Box::new(move |s: &mut AppState| {
                s.snooze_targets = snooze_ids.clone();
                s.snooze_at = rune::TextEdit::new("");
                s.snooze_open = true;
            }),
        ))
        .child(icon_button(
            &icons::REPLY,
            "Reply",
            Box::new(|s: &mut AppState| s.open_reply(false)),
        ))
        .child(icon_button(
            &icons::REPLY,
            "Reply all",
            Box::new(|s: &mut AppState| s.open_reply(true)),
        ))
        .child(icon_button(
            &icons::FORWARD,
            "Forward",
            Box::new(|s: &mut AppState| s.open_forward()),
        ))
        .child(el().grow(1.0));
    let pin_id = id.clone();
    let pinned = message.is_pinned;
    header = header.child(icon_button(
        &icons::PIN,
        if pinned { "Unpin" } else { "Pin" },
        Box::new(move |s: &mut AppState| {
            s.send(Action::SetPinned {
                ids: vec![pin_id.clone()],
                pinned: !pinned,
            })
        }),
    ));
    let star_id = id.clone();
    let starred = message.is_starred;
    header = header.child(icon_button(
        &icons::STAR,
        if starred { "Unstar" } else { "Star" },
        Box::new(move |s: &mut AppState| {
            s.send(Action::SetStarred {
                ids: vec![star_id.clone()],
                starred: !starred,
            })
        }),
    ));
    let read_id = id.clone();
    let read = message.is_read;
    header = header.child(icon_button(
        &icons::CHECK,
        if read { "Mark unread" } else { "Mark read" },
        Box::new(move |s: &mut AppState| {
            s.send(Action::SetRead {
                ids: vec![read_id.clone()],
                read: !read,
            })
        }),
    ));
    let print_id = id.clone();
    header = header.child(icon_button(
        &icons::PRINTER,
        "Print",
        Box::new(move |s: &mut AppState| s.send(Action::Print(print_id.clone()))),
    ));
    let dark = s.email_dark;
    header = header.child(icon_button(
        if dark { &icons::SUN } else { &icons::MOON },
        "Toggle email theme",
        Box::new(move |s: &mut AppState| s.email_dark = !dark),
    ));
    let archive_id = id.clone();
    header = header.child(icon_button(
        &icons::ARCHIVE,
        "Archive",
        Box::new(move |s: &mut AppState| s.send(Action::Archive(vec![archive_id.clone()]))),
    ));
    let trash_id = id.clone();
    header = header.child(icon_button(
        &icons::TRASH,
        "Delete",
        Box::new(move |s: &mut AppState| s.send(Action::Trash(vec![trash_id.clone()]))),
    ));
    let spam_id = id.clone();
    let spam = s.folder == "spam";
    header = header.child(icon_button(
        &icons::JUNK,
        "Spam",
        Box::new(move |s: &mut AppState| {
            s.send(Action::SetSpam {
                ids: vec![spam_id.clone()],
                spam: !spam,
            })
        }),
    ));
    if s.folder == "trash" || s.folder == "spam" {
        let restore_id = id.clone();
        header = header.child(icon_button(
            &icons::REFRESH,
            "Restore",
            Box::new(move |s: &mut AppState| {
                s.send(Action::RestoreMessages(vec![restore_id.clone()]))
            }),
        ));
    }
    let menu_id = id.clone();
    header = header.child(icon_button(
        &icons::MORE,
        "More",
        Box::new(move |s: &mut AppState| {
            s.menu_message = Some(menu_id.clone());
            s.menu_point = Some(rune::PointerPress { x: 900.0, y: 120.0 });
        }),
    ));
    header = header.child(icon_button(
        &icons::CLOSE,
        "Close reader",
        Box::new(|s: &mut AppState| s.reader_open = false),
    ));

    let (auth, auth_color) = if message.sender_verified {
        ("Verified sender", c.success.fg)
    } else if message.phishing_level.is_some() {
        ("Suspicious sender", c.danger.fg)
    } else {
        ("Unverified sender", c.warning.fg)
    };
    let initial = message
        .sender
        .chars()
        .next()
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "?".into());

    let mut content = col()
        .w_full()
        .gap(12.0)
        .child(
            text(if message.subject.is_empty() {
                "(no subject)"
            } else {
                message.subject.as_str()
            })
            .font(TextStyle::new(22.0, 600.0))
            .color(c.text_base)
            .wrap(),
        )
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .child(
                    el().w(36.0)
                        .h(36.0)
                        .radius(18.0)
                        .center()
                        .fill(c.bg_contrast)
                        .child(
                            text(initial)
                                .font(TextStyle::new(14.0, 600.0))
                                .color(c.text_contrast),
                        ),
                )
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(2.0)
                        .child(
                            text(if message.sender.is_empty() {
                                "Unknown sender"
                            } else {
                                message.sender.as_str()
                            })
                            .font(TextStyle::new(12.0, 600.0))
                            .color(c.text_base)
                            .truncate(),
                        )
                        .child(
                            text(format!(
                                "to {} · {}",
                                if message.recipients.is_empty() {
                                    "me"
                                } else {
                                    message.recipients.as_str()
                                },
                                short_date(message.date.as_deref())
                            ))
                            .font(TextStyle::new(11.0, 400.0))
                            .color(c.text_muted)
                            .truncate(),
                        ),
                )
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(icon(&icons::SHIELD, 16.0).color(auth_color))
                        .child(
                            text(auth)
                                .font(TextStyle::new(11.0, 500.0))
                                .color(auth_color),
                        ),
                ),
        );

    if !message.tags.is_empty() {
        let mut tags = row().w_full().gap(4.0).center_y();
        for tag in &message.tags {
            tags = tags.child(
                row()
                    .h(22.0)
                    .px(8.0)
                    .gap(4.0)
                    .center_y()
                    .radius(k.radius.sm)
                    .fill(c.layer_02)
                    .border(1.0, c.border_base)
                    .child(icon(&icons::TAG, 12.0).color(c.text_muted))
                    .child(
                        text(tag.as_str())
                            .font(TextStyle::new(11.0, 400.0))
                            .color(c.text_base),
                    ),
            );
        }
        content = content.child(tags);
    }

    if s.thread.len() > 1 {
        let index = s
            .thread
            .iter()
            .position(|item| item.id == message.id)
            .map(|index| index + 1)
            .unwrap_or(1);
        content = content.child(
            row()
                .key("thread-strip")
                .w_full()
                .gap(4.0)
                .center_y()
                .child(
                    text(format!("Conversation · {} messages", s.thread.len()))
                        .font(TextStyle::new(11.0, 500.0))
                        .color(c.text_muted)
                        .grow(1.0),
                )
                .child(
                    icon(&icons::CHEVRON_LEFT, 16.0)
                        .color(c.text_muted)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut AppState| s.step_thread(-1)),
                )
                .child(
                    text(format!("{index} of {}", s.thread.len()))
                        .font(TextStyle::new(11.0, 400.0))
                        .color(c.text_base),
                )
                .child(
                    icon(&icons::CHEVRON_RIGHT, 16.0)
                        .color(c.text_muted)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut AppState| s.step_thread(1)),
                ),
        );
    }

    if !detail.remote_content_loaded && detail.external_content.blocked_count > 0 {
        let remote_id = message.id.clone();
        content = content.child(
            row()
                .key("remote-banner")
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    text(format!(
                        "{} remote item(s) blocked",
                        detail.external_content.blocked_count
                    ))
                    .font(TextStyle::new(12.0, 400.0))
                    .color(c.text_muted)
                    .grow(1.0),
                )
                .child(link_action(
                    "Load remote content",
                    c.text_accent,
                    k,
                    move |s: &mut AppState| s.send(Action::LoadRemoteContent(remote_id.clone())),
                )),
        );
    }

    if !detail.attachments.is_empty() {
        let all_id = message.id.clone();
        let mut list = col().w_full().gap(6.0).child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    text("Attachments")
                        .font(TextStyle::new(12.0, 400.0))
                        .color(c.text_muted)
                        .grow(1.0),
                )
                .child(link_action(
                    "Save all",
                    c.text_accent,
                    k,
                    move |s: &mut AppState| s.send(Action::SaveAllAttachments(all_id.clone())),
                )),
        );
        for attachment in &detail.attachments {
            let preview_id = message.id.clone();
            let save_id = message.id.clone();
            let seq = attachment.seq;
            list = list.child(
                row()
                    .key(format!("attachment-{}", attachment.seq))
                    .h(30.0)
                    .px(12.0)
                    .gap(8.0)
                    .center_y()
                    .radius(k.radius.sm)
                    .fill(c.layer_02)
                    .border(1.0, c.border_base)
                    .child(icon(&icons::ATTACH, 16.0).color(c.text_muted))
                    .child(
                        text(attachment.name.as_str())
                            .font(TextStyle::new(12.0, 400.0))
                            .color(c.text_base)
                            .truncate()
                            .grow(1.0),
                    )
                    .child(
                        text(size_label(attachment.size))
                            .font(TextStyle::new(11.0, 400.0))
                            .color(c.text_muted),
                    )
                    .child(link_action(
                        "Preview",
                        c.text_accent,
                        k,
                        move |s: &mut AppState| {
                            s.send(Action::PreviewAttachment {
                                id: preview_id.clone(),
                                seq,
                            })
                        },
                    ))
                    .child(link_action(
                        "Save",
                        c.text_accent,
                        k,
                        move |s: &mut AppState| {
                            s.send(Action::SaveAttachment {
                                id: save_id.clone(),
                                seq,
                            })
                        },
                    )),
            );
        }
        content = content.child(list);
    }

    content = content.child(body_el(s, detail, k));

    col()
        .key("reader")
        .w(s.reader_width)
        .h_full()
        .min_h(0.0)
        .fill(surface)
        .child(header)
        .child(el().w_full().h(1.0).fill(c.border_muted))
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .px(16.0)
                .py(24.0)
                .child(content),
        )
}

fn overlays(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if let Some(toast) = &s.toast {
        let box_ = row()
            .h(34.0)
            .px(16.0)
            .radius(k.radius.sm)
            .fill(k.c.layer_03)
            .border(1.0, k.c.border_strong)
            .center()
            .child(
                text(toast.as_str())
                    .font(TextStyle::new(12.0, 400.0))
                    .color(k.c.text_base),
            );
        layer.push(
            row()
                .key("toast")
                .pin(Some(0.0), None, Some(0.0), Some(44.0))
                .h(34.0)
                .center()
                .inert()
                .child(box_),
        );
    }
    if let Some(message_id) = s.menu_message.clone() {
        if s.menu_point.is_some() {
            message_menu(s, k, layer, message_id);
        }
    }
    outbox_dialog(s, k, layer);
    filters_dialog(s, k, layer);
    command_dialog(s, k, layer);
    snooze_dialog(s, k, layer);
    delete_dialog(s, k, layer);
    preview_dialog(s, k, layer);
}

fn message_menu(s: &AppState, k: &Kit, layer: &mut Vec<El>, message_id: String) {
    let ids = if s.selection.is_empty() {
        vec![message_id.clone()]
    } else {
        s.selection.iter().cloned().collect()
    };
    let starred = s
        .detail
        .as_ref()
        .map(|detail| detail.message.is_starred)
        .unwrap_or(false);
    let read = s
        .detail
        .as_ref()
        .map(|detail| detail.message.is_read)
        .unwrap_or(true);
    let star_ids = ids.clone();
    let read_ids = ids.clone();
    let archive_ids = ids.clone();
    let trash_ids = ids.clone();
    let snooze_ids = ids.clone();
    let delete_ids = ids.clone();
    let print_id = message_id.clone();
    context_menu("message-menu", el(), s.menu_point)
        .item(MenuItem::new("Reply").on_select(|s: &mut AppState| {
            s.menu_point = None;
            s.open_reply(false);
        }))
        .item(MenuItem::new("Reply all").on_select(|s: &mut AppState| {
            s.menu_point = None;
            s.open_reply(true);
        }))
        .item(MenuItem::new("Forward").on_select(|s: &mut AppState| {
            s.menu_point = None;
            s.open_forward();
        }))
        .separator()
        .item(
            MenuItem::new(if starred { "Unstar" } else { "Star" }).on_select(
                move |s: &mut AppState| {
                    s.menu_point = None;
                    s.send(Action::SetStarred {
                        ids: star_ids.clone(),
                        starred: !starred,
                    });
                },
            ),
        )
        .item(
            MenuItem::new(if read { "Mark unread" } else { "Mark read" }).on_select(
                move |s: &mut AppState| {
                    s.menu_point = None;
                    s.send(Action::SetRead {
                        ids: read_ids.clone(),
                        read: !read,
                    });
                },
            ),
        )
        .separator()
        .item(MenuItem::new("Archive").on_select(move |s: &mut AppState| {
            s.menu_point = None;
            s.send(Action::Archive(archive_ids.clone()));
        }))
        .item(MenuItem::new("Trash").on_select(move |s: &mut AppState| {
            s.menu_point = None;
            s.send(Action::Trash(trash_ids.clone()));
        }))
        .item(MenuItem::new("Snooze…").on_select(move |s: &mut AppState| {
            s.menu_point = None;
            s.snooze_targets = snooze_ids.clone();
            s.snooze_at = rune::TextEdit::new("");
            s.snooze_open = true;
        }))
        .item(MenuItem::new("Print").on_select(move |s: &mut AppState| {
            s.menu_point = None;
            s.send(Action::Print(print_id.clone()));
        }))
        .separator()
        .item(MenuItem::new("Delete permanently…").danger(true).on_select(
            move |s: &mut AppState| {
                s.menu_point = None;
                s.delete_targets = delete_ids.clone();
                s.delete_word = rune::TextEdit::new("");
                s.delete_open = true;
            },
        ))
        .on_close(|s: &mut AppState| s.menu_point = None)
        .build(k, layer);
}

fn outbox_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.outbox_open {
        return;
    }
    let mut body = col().w_full().gap(6.0);
    let rows = s.outbox.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if rows.is_empty() {
        body = body.child(
            text("Outbox is empty")
                .font(k.ty.small)
                .color(k.c.text_muted),
        );
    }
    for entry in rows {
        let id = entry
            .get("id")
            .and_then(|value| value.as_i64())
            .unwrap_or_default();
        let to = entry
            .get("envelope_to")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let status = entry
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("queued");
        let error = entry
            .get("last_error")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        body = body.child(
            row()
                .key(format!("outbox-{id}"))
                .w_full()
                .gap(8.0)
                .center_y()
                .padding(8.0)
                .radius(k.radius.md)
                .fill(k.c.surface_weak)
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(text(to).font(k.ty.small).color(k.c.text_base).truncate())
                        .child(
                            text(format!("{status} {error}"))
                                .font(k.ty.small)
                                .color(k.c.text_muted)
                                .truncate(),
                        ),
                )
                .child(
                    Button::new("Retry")
                        .variant(ButtonVariant::Ghost)
                        .on_press(move |s: &mut AppState| s.send(Action::RetryOutbox(id)))
                        .build(k),
                ),
        );
    }
    body = body.child(
        row()
            .gap(6.0)
            .child(
                Button::new("Refresh")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut AppState| s.send(Action::LoadOutbox))
                    .build(k),
            )
            .child(
                Button::new("Close")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut AppState| s.outbox_open = false)
                    .build(k),
            ),
    );
    Dialog::new("outbox", "Outbox")
        .open(true)
        .fit()
        .width(600.0)
        .child(
            col()
                .w_full()
                .max_h(600.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(body),
        )
        .on_close(|s: &mut AppState| s.outbox_open = false)
        .build(k, layer);
}

fn filters_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.filters_open {
        return;
    }
    let mut body = col().w_full().gap(8.0);
    if !s.filters.list_id.text.is_empty() {
        let list = s.filters.list_id.text.clone();
        body = body.child(
            Button::new(format!("Archive all mail from {list}"))
                .variant(ButtonVariant::Danger)
                .on_press(move |s: &mut AppState| s.send(Action::ArchiveMailingList(list.clone())))
                .build(k),
        );
    }
    for (label, placeholder, value) in [
        ("From", "sender or domain", &s.filters.from),
        ("Subject", "subject contains", &s.filters.subject),
        ("Body", "preview contains", &s.filters.body),
        ("Since", "YYYY-MM-DD", &s.filters.since),
        ("Until", "YYYY-MM-DD", &s.filters.until),
        ("Category", "category", &s.filters.category),
        ("Tag", "tag token", &s.filters.tag),
        ("Mailing list", "List-Id", &s.filters.list_id),
    ] {
        let field = match label {
            "From" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.from)
                .build(k),
            "Subject" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.subject)
                .build(k),
            "Body" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.body)
                .build(k),
            "Since" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.since)
                .build(k),
            "Until" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.until)
                .build(k),
            "Category" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.category)
                .build(k),
            "Tag" => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.tag)
                .build(k),
            _ => TextInput::new(value)
                .label(label)
                .width(None)
                .placeholder(placeholder)
                .on_edit(|s: &mut AppState| &mut s.filters.list_id)
                .build(k),
        };
        body = body.child(field);
    }
    body = body.child(
        row()
            .gap(6.0)
            .child(toggle_small(k, "Unread only", s.filters.unread_only, |s| {
                s.filters.unread_only = !s.filters.unread_only
            }))
            .child(toggle_small(k, "Starred", s.filters.starred, |s| {
                s.filters.starred = !s.filters.starred
            }))
            .child(toggle_small(k, "Attachments", s.filters.attachments, |s| {
                s.filters.attachments = !s.filters.attachments
            })),
    );
    body = body.child(
        TextInput::new(&s.filters.name)
            .label("Saved search name")
            .width(None)
            .placeholder("name for Save search")
            .on_edit(|s: &mut AppState| &mut s.filters.name)
            .build(k),
    );
    body = body.child(
        row()
            .gap(6.0)
            .child(
                Button::new("Clear")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut AppState| {
                        s.clear_filters();
                        s.filters_open = false;
                    })
                    .build(k),
            )
            .child(
                Button::new("Save search")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut AppState| s.save_search())
                    .build(k),
            )
            .child(
                Button::new("Close")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut AppState| s.filters_open = false)
                    .build(k),
            ),
    );
    if let Some(searches) = s
        .settings
        .pointer("/mail/saved_searches")
        .and_then(|value| value.as_array())
    {
        if !searches.is_empty() {
            body = body.child(
                text("Saved searches")
                    .font(k.ty.small_medium)
                    .color(k.c.text_muted),
            );
            for search in searches {
                let value = search.clone();
                let name = search
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Saved search")
                    .to_string();
                body = body.child(
                    Button::new(name)
                        .variant(ButtonVariant::Ghost)
                        .on_press(move |s: &mut AppState| s.apply_saved_search(&value))
                        .build(k),
                );
            }
        }
    }
    Dialog::new("filters", "Filters")
        .open(true)
        .fit()
        .width(560.0)
        .child(
            col()
                .w_full()
                .max_h(600.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(body),
        )
        .on_close(|s: &mut AppState| s.filters_open = false)
        .build(k, layer);
}

fn toggle_small(k: &Kit, label: &str, on: bool, action: impl Fn(&mut AppState) + 'static) -> El {
    Button::new(format!("{label}: {}", if on { "on" } else { "off" }))
        .variant(if on {
            ButtonVariant::Contrast
        } else {
            ButtonVariant::Ghost
        })
        .on_press(action)
        .build(k)
}

fn command_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.command_open {
        return;
    }
    let filter = s.command_filter.text.to_ascii_lowercase();
    let mut commands: Vec<(&str, String)> = VIEWS
        .iter()
        .map(|(id, label)| ("go", format!("Go to {label}:{id}")))
        .collect();
    commands.push(("compose", "Compose".into()));
    commands.push(("settings", "Settings".into()));
    commands.push(("refresh", "Refresh mail".into()));
    commands.push(("sync", "Sync now".into()));
    let matches = |label: &str| filter.is_empty() || label.to_ascii_lowercase().contains(&filter);
    let mut list = col().w_full().gap(2.0);
    for (kind, label) in commands {
        let shown = label.split(':').next().unwrap_or(&label).to_string();
        if !matches(&shown) {
            continue;
        }
        let target = label.split(':').nth(1).map(str::to_string);
        let kind = kind.to_string();
        list = list.child(
            Button::new(shown)
                .variant(ButtonVariant::Ghost)
                .on_press(move |s: &mut AppState| {
                    s.command_open = false;
                    match kind.as_str() {
                        "compose" => s.open_new_mail(),
                        "settings" => s.open_settings(SettingsTab::Appearance),
                        "refresh" => s.send(Action::Refresh),
                        "sync" => s.send(Action::SyncNow),
                        "go" => {
                            if let Some(target) = &target {
                                s.open_folder(target.clone());
                            }
                        }
                        _ => {}
                    }
                })
                .build(k),
        );
    }
    Dialog::new("command", "Command palette")
        .open(true)
        .fit()
        .width(520.0)
        .child(
            TextInput::new(&s.command_filter)
                .label("Command")
                .width(None)
                .placeholder("Type a command")
                .on_edit(|s: &mut AppState| &mut s.command_filter)
                .build(k),
        )
        .child(list)
        .on_close(|s: &mut AppState| s.command_open = false)
        .build(k, layer);
}

fn snooze_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.snooze_open {
        return;
    }
    let mut body = col().w_full().gap(8.0).child(
        TextInput::new(&s.snooze_at)
            .label("Snooze until")
            .width(None)
            .placeholder("ISO timestamp")
            .on_edit(|s: &mut AppState| &mut s.snooze_at)
            .build(k),
    );
    let mut presets = row().gap(6.0);
    for (label, iso) in snooze_presets() {
        let until = iso.clone();
        presets = presets.child(
            Button::new(label)
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut AppState| {
                    let ids = s.snooze_targets.clone();
                    s.snooze_open = false;
                    s.send(Action::Snooze {
                        ids,
                        until: until.clone(),
                    });
                })
                .build(k),
        );
    }
    body = body.child(presets);
    let custom_targets = s.snooze_targets.clone();
    body = body.child(
        row()
            .gap(6.0)
            .child(
                Button::new("Cancel")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut AppState| s.snooze_open = false)
                    .build(k),
            )
            .child(
                Button::new("Snooze")
                    .variant(ButtonVariant::Contrast)
                    .on_press(move |s: &mut AppState| {
                        let until = s.snooze_at.text.trim().to_string();
                        if until.is_empty() {
                            s.toast = Some("Enter a snooze time".into());
                            return;
                        }
                        s.snooze_open = false;
                        s.send(Action::Snooze {
                            ids: custom_targets.clone(),
                            until,
                        });
                    })
                    .build(k),
            ),
    );
    Dialog::new("snooze", "Snooze")
        .open(true)
        .fit()
        .width(460.0)
        .child(body)
        .on_close(|s: &mut AppState| s.snooze_open = false)
        .build(k, layer);
}

fn delete_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.delete_open {
        return;
    }
    Dialog::new("delete", "Delete permanently")
        .open(true)
        .fit()
        .width(460.0)
        .description("This cannot be undone. Type delete to confirm.")
        .child(
            TextInput::new(&s.delete_word)
                .label("Confirmation")
                .width(None)
                .placeholder("delete")
                .on_edit(|s: &mut AppState| &mut s.delete_word)
                .build(k),
        )
        .child(
            row()
                .gap(6.0)
                .child(
                    Button::new("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut AppState| s.cancel_delete())
                        .build(k),
                )
                .child(
                    Button::new("Delete")
                        .variant(ButtonVariant::Danger)
                        .on_press(|s: &mut AppState| s.confirm_delete())
                        .build(k),
                ),
        )
        .on_close(|s: &mut AppState| s.cancel_delete())
        .build(k, layer);
}

fn preview_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.preview_open {
        return;
    }
    Dialog::new("attachment-preview", "Attachment preview")
        .open(true)
        .width(640.0)
        .child(
            col()
                .key("preview-body")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    text(s.preview_text.as_str())
                        .font(k.ty.mono)
                        .color(k.c.text_base)
                        .selectable(true),
                ),
        )
        .on_close(|s: &mut AppState| s.preview_open = false)
        .build(k, layer);
}

fn snooze_presets() -> Vec<(&'static str, String)> {
    let now = chrono::Utc::now();
    vec![
        ("1 hour", (now + chrono::Duration::hours(1)).to_rfc3339()),
        ("Tomorrow", (now + chrono::Duration::days(1)).to_rfc3339()),
        ("Next week", (now + chrono::Duration::days(7)).to_rfc3339()),
    ]
}

fn message_row(
    s: &AppState,
    k: &Kit,
    message: &aster_client_core::service::MessageView,
    index: usize,
    thread_count: usize,
) -> El {
    let c = &k.c;
    let id = message.id.clone();
    let menu_id = id.clone();
    let selected = s.selected.as_deref() == Some(message.id.as_str());
    let marked = s.selection.contains(&message.id);
    let unread = !message.is_read;
    let subject_style = if unread {
        TextStyle::new(k.ty.ui.size, 600.0)
    } else {
        k.ty.ui
    };
    let fill = if selected {
        c.surface_raised
    } else if index == s.cursor {
        c.surface_hover
    } else {
        c.bg_base
    };
    let mut meta = row().w_full().gap(8.0).center_y().child(
        text(message.sender.as_str())
            .font(k.ty.small)
            .color(c.text_muted)
            .truncate()
            .grow(1.0),
    );
    if thread_count > 1 {
        meta = meta.child(
            text(format!("({thread_count})"))
                .font(k.ty.small)
                .color(c.text_muted),
        );
    }
    if message.is_starred {
        meta = meta.child(text("★").font(k.ty.small).color(c.text_accent));
    }
    meta = meta.child(
        text(short_date(message.date.as_deref()))
            .font(k.ty.small)
            .color(c.text_muted),
    );

    col()
        .key(format!("message-{}", message.id))
        .role("listitem")
        .label(format!("{}: {}", message.sender, message.subject))
        .w_full()
        .py(12.0)
        .px(16.0)
        .gap(4.0)
        .fill(fill)
        .hover(c.hover)
        .on_press(move |s: &mut AppState| {
            s.cursor = index;
            s.select(id.clone());
        })
        .on_secondary_press(move |s: &mut AppState, point, _| {
            s.cursor = index;
            s.selected = Some(menu_id.clone());
            s.menu_message = Some(menu_id.clone());
            s.menu_point = Some(point);
            s.send(Action::Select(menu_id.clone()));
        })
        .child(meta)
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(el().w(6.0).h(6.0).radius(3.0).fill(if unread {
                    c.text_accent
                } else {
                    Color::TRANSPARENT
                }))
                .child(
                    text(message.subject.as_str())
                        .font(subject_style)
                        .color(c.text_base)
                        .truncate()
                        .grow(1.0),
                )
                .child(if marked {
                    el().w(3.0).h(16.0).radius(2.0).fill(c.bg_accent)
                } else {
                    el().w(3.0).h(16.0)
                }),
        )
        .child(
            text(message.preview.as_str())
                .font(k.ty.ui)
                .color(c.text_muted)
                .truncate(),
        )
}

fn link_action(label: &str, color: Color, k: &Kit, action: impl Fn(&mut AppState) + 'static) -> El {
    text(label)
        .font(k.ty.small)
        .color(color)
        .cursor(Cursor::Pointer)
        .on_press(action)
}

fn size_label(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let bytes = bytes.max(0) as f64;
    if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes / KB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn body_el(s: &AppState, detail: &aster_client_core::service::MessageDetail, k: &Kit) -> El {
    if !detail.is_html {
        return text(body_text(detail))
            .font(k.ty.ui)
            .color(k.c.text_base)
            .wrap()
            .selectable(true);
    }
    let html = if k.dark {
        detail
            .body_html_dark
            .as_deref()
            .unwrap_or(&detail.body_html)
    } else {
        &detail.body_html
    };
    let key = rune::memo::digest(&(detail.message.id.as_str(), k.dark, html));
    el().key("mail-body")
        .w_full()
        .child(rune::memo::cached(key, || {
            let style = mail_style(k);
            render_mail(html, &style, s.images.as_ref(), 2.0, open_url)
        }))
}

fn mail_style(k: &Kit) -> MailStyle {
    MailStyle {
        text: k.c.text_base,
        muted: k.c.text_muted,
        link: k.c.text_accent,
        code_bg: k.c.surface_base,
        quote_border: k.c.border_base,
        rule: k.c.border_muted,
        surface: k.c.surface_weak,
        body_size: 14.0,
    }
}

fn open_url(_: &mut AppState, url: &str) {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("file:")
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

pub fn on_key(s: &mut AppState, event: &KeyEvent, tx: &mut Tx) -> bool {
    if s.compose.requested {
        match &event.key {
            Key::Escape => s.close_compose(),
            Key::Command('q') => tx.quit(),
            _ => return false,
        }
        return true;
    }
    if s.command_open || s.snooze_open || s.delete_open || s.preview_open || s.filters_open {
        if matches!(event.key, Key::Escape) {
            s.command_open = false;
            s.snooze_open = false;
            if s.delete_open {
                s.cancel_delete();
            }
            s.preview_open = false;
            s.filters_open = false;
            return true;
        }
        if !matches!(event.key, Key::Command('q')) {
            return false;
        }
    }
    match &event.key {
        Key::Down => s.move_cursor(1),
        Key::Up => s.move_cursor(-1),
        Key::Text(value) if value == "j" => s.move_cursor(1),
        Key::Text(value) if value == "k" => s.move_cursor(-1),
        Key::Text(value) if value == "x" => s.toggle_selection(),
        Key::Text(value) if value == "e" => s.batch_archive(),
        Key::Text(value) if value == "u" => s.batch_read(false),
        Key::Text(value) if value == "s" => s.batch_star(true),
        Key::Text(value) if value == "c" => s.open_new_mail(),
        Key::Text(value) if value == "r" => s.open_reply(false),
        Key::Text(value) if value == "a" => s.open_reply(true),
        Key::Text(value) if value == "f" => s.open_forward(),
        Key::Text(value) if value == "#" => {
            s.delete_targets = s.target_ids();
            s.delete_word = rune::TextEdit::new("");
            s.delete_open = true;
        }
        Key::Delete => s.batch_trash(),
        Key::Command('k') => {
            s.command_open = true;
            s.command_filter = rune::TextEdit::new("");
        }
        Key::Text(value) if value == "/" => {
            s.command_open = true;
            s.command_filter = rune::TextEdit::new("");
        }
        Key::Enter => {
            if let Some(id) = s.cursor_id() {
                s.select(id);
            }
        }
        Key::Escape => {
            s.menu_point = None;
            s.selection.clear();
        }
        Key::Command(',') => s.open_settings(SettingsTab::Appearance),
        Key::Command('r') => s.send(Action::Refresh),
        Key::Command('q') => tx.quit(),
        _ => return false,
    }
    true
}

pub fn poll(state: &mut AppState, seconds: f64, _tx: &mut Tx) -> bool {
    let mut changed = state.refresh_visible();
    if state.toast.is_some() {
        match state.toast_at {
            None => state.toast_at = Some(std::time::Instant::now()),
            Some(at) if at.elapsed() > std::time::Duration::from_secs(4) => {
                state.toast = None;
                state.toast_at = None;
                changed = true;
            }
            _ => {}
        }
    } else {
        state.toast_at = None;
    }
    if seconds >= state.theme.next_check {
        state.theme.next_check = seconds + 5.0;
        changed |= state.theme.refresh_system_dark();
    }
    changed
}

fn count(s: &AppState, folder: &str, key: &str) -> u64 {
    s.counts
        .get(folder)
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
}

fn folder_label(s: &AppState) -> String {
    if let Some((_, label)) = VIEWS.iter().find(|(id, _)| *id == s.folder) {
        return (*label).to_string();
    }
    s.mailboxes
        .iter()
        .find(|mailbox| mailbox.id == s.folder)
        .map(|mailbox| mailbox.name.clone())
        .unwrap_or_else(|| s.folder.clone())
}

fn short_date(date: Option<&str>) -> String {
    let Some(date) = date else {
        return String::new();
    };
    date.get(..16).unwrap_or(date).replace('T', " ")
}

fn body_text(detail: &aster_client_core::service::MessageDetail) -> String {
    if detail.is_html {
        strip_tags(&detail.body)
    } else {
        detail.body.clone()
    }
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

fn settings_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.settings_open {
        return;
    }
    let c = &k.c;
    let sidebar = s.theme.token_color("sidebar", c.page);
    let selected_color = s.theme.token_color("selected", c.layer_03);
    let mut nav = col()
        .key("settings-nav")
        .w(208.0)
        .h_full()
        .padding(12.0)
        .gap(4.0)
        .fill(sidebar)
        .child(
            row()
                .w_full()
                .mb(8.0)
                .gap(8.0)
                .center_y()
                .child(icon(&icons::SETTINGS, 16.0).color(c.text_accent))
                .child(
                    text("Settings")
                        .font(TextStyle::new(16.0, 600.0))
                        .color(c.text_base)
                        .grow(1.0),
                ),
        );
    for tab in SettingsTab::ALL {
        let selected = s.settings_tab == tab;
        nav = nav.child(nav_row_boxed(
            k,
            tab.label(),
            tab_icon(tab),
            0,
            selected,
            selected_color,
            Box::new(move |s: &mut AppState| {
                s.settings_tab = tab;
                if tab == SettingsTab::Diagnostics {
                    s.send(Action::LoadDiagnostics);
                }
            }),
        ));
    }
    nav = nav.child(el().grow(1.0)).child(
        text("Aster Qt")
            .font(TextStyle::new(11.0, 400.0))
            .color(c.text_muted),
    );

    let content = match s.settings_tab {
        SettingsTab::Appearance => appearance_tab(s, k),
        SettingsTab::Mail => mail_tab(s, k),
        SettingsTab::Rules => rules_tab(s, k),
        SettingsTab::Aliases => aliases_tab(s, k),
        SettingsTab::Contacts => contacts_tab(s, k),
        SettingsTab::Security => security_tab(s, k),
        SettingsTab::Mcp => mcp_tab(s, k),
        SettingsTab::Diagnostics => diagnostics_tab(s, k),
    };
    let content = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .fill(c.layer_01)
        .child(
            row()
                .w_full()
                .h(54.0)
                .px(16.0)
                .center_y()
                .child(
                    text(s.settings_tab.label())
                        .font(TextStyle::new(16.0, 600.0))
                        .color(c.text_base)
                        .grow(1.0),
                )
                .child(
                    IconButton::new(&icons::CLOSE)
                        .label("Close settings")
                        .variant(IconButtonVariant::Ghost)
                        .control_size(ControlSize::Regular)
                        .on_press(|s: &mut AppState| s.close_settings())
                        .build(k)
                        .size(28.0, 28.0),
                ),
        )
        .child(el().w_full().h(1.0).fill(c.border_muted))
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .padding(24.0)
                .child(col().w_full().max_w(720.0).gap(12.0).child(content)),
        );

    let body = row().w_full().h(760.0).child(nav).child(content);
    modal_panel(s, k, layer, "settings", 1040.0, 760.0, body, |s| {
        s.close_settings()
    });
}

#[allow(clippy::too_many_arguments)]
fn modal_panel(
    _s: &AppState,
    k: &Kit,
    layer: &mut Vec<El>,
    key: &str,
    width: f32,
    height: f32,
    body: El,
    close: impl Fn(&mut AppState) + 'static,
) {
    let scrim = rune_kit::scrim(format!("{key}-scrim"), close).fill(k.c.scrim);
    let panel = col()
        .key(key)
        .w(width)
        .h(height)
        .radius(8.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_strong)
        .on_press(|_: &mut AppState| {})
        .child(body);
    let wrap = el()
        .key(format!("{key}-wrap"))
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .center()
        .child(panel);
    layer.push(scrim);
    layer.push(wrap);
}

fn tab_icon(tab: SettingsTab) -> &'static Icon {
    match tab {
        SettingsTab::Appearance => &icons::SUN,
        SettingsTab::Mail => &icons::INBOX,
        SettingsTab::Rules => &icons::RULE,
        SettingsTab::Aliases => &icons::FOLDER,
        SettingsTab::Contacts => &icons::INBOX,
        SettingsTab::Security => &icons::SHIELD,
        SettingsTab::Mcp => &icons::SEND,
        SettingsTab::Diagnostics => &icons::SETTINGS,
    }
}

fn appearance_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let mode = &s.theme.mode;
    let mut root = col().w_full().gap(10.0).child(
        row()
            .gap(6.0)
            .child(mode_button(k, "System", mode == "system", "system"))
            .child(mode_button(k, "Light", mode == "light", "light"))
            .child(mode_button(k, "Dark", mode == "dark", "dark")),
    );
    for (slot, label) in [("light", "Light theme"), ("dark", "Dark theme")] {
        let selected = if slot == "light" {
            s.theme.light.clone()
        } else {
            s.theme.dark.clone()
        };
        let mut themes = col()
            .w_full()
            .gap(2.0)
            .child(text(label).font(k.ty.small_medium).color(c.text_muted));
        for theme in s
            .theme
            .themes
            .iter()
            .filter(|theme| (slot == "light") == theme.light)
        {
            let id = theme.id.clone();
            let active = id == selected;
            themes = themes.child(
                Button::new(format!("{}{}", theme.name, if active { "  ✓" } else { "" }))
                    .variant(if active {
                        ButtonVariant::Outline
                    } else {
                        ButtonVariant::Ghost
                    })
                    .on_press(move |s: &mut AppState| s.set_theme_slot(slot, id.clone()))
                    .build(k),
            );
        }
        root = root.child(themes);
    }
    root.child(
        row()
            .gap(6.0)
            .child(
                Button::new("New from template")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut AppState| s.create_theme_from_template())
                    .build(k),
            )
            .child(
                Button::new("Open folder")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut AppState| s.open_themes_folder())
                    .build(k),
            )
            .child(
                Button::new("Reload")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut AppState| s.reload_themes())
                    .build(k),
            ),
    )
}

fn mode_button(k: &Kit, label: &str, active: bool, mode: &'static str) -> El {
    Button::new(label)
        .variant(if active {
            ButtonVariant::Contrast
        } else {
            ButtonVariant::Ghost
        })
        .on_press(move |s: &mut AppState| s.set_theme_mode(mode))
        .build(k)
}

fn mail_tab(s: &AppState, k: &Kit) -> El {
    let dark = app_settings::string(&s.settings, &["mail", "html_dark_mode"], "auto");
    let dim = app_settings::boolean(&s.settings, &["mail", "html_dark_dim_images"], true);
    let reader = app_settings::boolean(&s.settings, &["mail", "reader_open"], true);
    let history = app_settings::boolean(&s.settings, &["mail", "full_history"], true);
    let searches = s
        .settings
        .pointer("/mail/saved_searches")
        .and_then(|value| value.as_array())
        .map(Vec::len)
        .unwrap_or(0);
    col()
        .w_full()
        .gap(10.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    text("HTML mail")
                        .font(k.ty.ui)
                        .color(k.c.text_base)
                        .grow(1.0),
                )
                .child(mode_setting_button(k, "Off", &dark, "off"))
                .child(mode_setting_button(k, "Auto", &dark, "auto"))
                .child(mode_setting_button(k, "Always", &dark, "always")),
        )
        .child(toggle_row(k, "Dim images in dark mail", dim, move |s| {
            s.apply_settings(serde_json::json!({"mail": {"html_dark_dim_images": !dim}}))
        }))
        .child(toggle_row(
            k,
            "Reader opens on selection",
            reader,
            move |s| s.apply_settings(serde_json::json!({"mail": {"reader_open": !reader}})),
        ))
        .child(toggle_row(k, "Keep full mail history", history, move |s| {
            s.apply_settings(serde_json::json!({"mail": {"full_history": !history}}))
        }))
        .child(
            text(format!("{searches} saved search(es)"))
                .font(k.ty.small)
                .color(k.c.text_muted),
        )
}

fn mode_setting_button(k: &Kit, label: &str, current: &str, value: &'static str) -> El {
    let active = current == value;
    Button::new(label)
        .variant(if active {
            ButtonVariant::Contrast
        } else {
            ButtonVariant::Ghost
        })
        .on_press(move |s: &mut AppState| {
            s.apply_settings(serde_json::json!({"mail": {"html_dark_mode": value}}))
        })
        .build(k)
}

fn toggle_row(k: &Kit, label: &str, on: bool, action: impl Fn(&mut AppState) + 'static) -> El {
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(label).font(k.ty.ui).color(k.c.text_base).grow(1.0))
        .child(
            Button::new(if on { "On" } else { "Off" })
                .variant(if on {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .on_press(action)
                .build(k),
        )
}

fn rules_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let mut root = col().w_full().gap(8.0);
    let rules = rules_of(&s.rules);
    let rule_ids: Vec<String> = rules
        .iter()
        .filter_map(|rule| {
            rule.get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect();
    if rules.is_empty() {
        root = root.child(
            text("No server-side rules yet")
                .font(k.ty.small)
                .color(c.text_muted),
        );
    }
    for (rule_index, rule) in rules.iter().enumerate() {
        let id = rule
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let name = rule
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Rule")
            .to_string();
        let enabled = rule
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let summary = rule_summary(rule);
        let delete_id = id.clone();
        let run_id = id.clone();
        let mut reorder_ids = rule_ids.clone();
        let can_up = rule_index > 0;
        let can_down = rule_index + 1 < rule_ids.len();
        let up_ids = {
            if can_up {
                reorder_ids.swap(rule_index, rule_index - 1);
            }
            reorder_ids.clone()
        };
        let mut reorder_ids = rule_ids.clone();
        let down_ids = {
            if can_down {
                reorder_ids.swap(rule_index, rule_index + 1);
            }
            reorder_ids.clone()
        };
        root = root.child(
            col()
                .key(format!("settings-rule-{id}"))
                .w_full()
                .gap(2.0)
                .padding(8.0)
                .radius(k.radius.md)
                .fill(c.surface_weak)
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .child(
                            text(format!("{name}{}", if enabled { "" } else { " (off)" }))
                                .font(k.ty.ui_medium)
                                .color(c.text_base)
                                .grow(1.0),
                        )
                        .child(
                            Button::new("↑")
                                .variant(ButtonVariant::Ghost)
                                .on_press(move |s: &mut AppState| {
                                    s.send(Action::ReorderRules(up_ids.clone()))
                                })
                                .build(k),
                        )
                        .child(
                            Button::new("↓")
                                .variant(ButtonVariant::Ghost)
                                .on_press(move |s: &mut AppState| {
                                    s.send(Action::ReorderRules(down_ids.clone()))
                                })
                                .build(k),
                        )
                        .child(
                            Button::new("Run")
                                .variant(ButtonVariant::Ghost)
                                .on_press(move |s: &mut AppState| {
                                    s.send(Action::RunRule(run_id.clone()))
                                })
                                .build(k),
                        )
                        .child(
                            Button::new("Delete")
                                .variant(ButtonVariant::Danger)
                                .on_press(move |s: &mut AppState| {
                                    s.send(Action::DeleteRule(delete_id.clone()))
                                })
                                .build(k),
                        ),
                )
                .child(text(summary).font(k.ty.small).color(c.text_muted)),
        );
    }

    root.child(el().w_full().h(1.0).fill(c.border_muted))
        .child(text("New rule").font(k.ty.small_medium).color(c.text_muted))
        .child(
            TextInput::new(&s.forms.rule_name)
                .label("Rule name")
                .width(None)
                .placeholder("Rule name")
                .on_edit(|s: &mut AppState| &mut s.forms.rule_name)
                .build(k),
        )
        .child(
            TextInput::new(&s.forms.rule_value)
                .label("From contains")
                .width(None)
                .placeholder("sender or domain")
                .on_edit(|s: &mut AppState| &mut s.forms.rule_value)
                .build(k),
        )
        .child(
            TextInput::new(&s.forms.rule_action)
                .label("Move to folder token")
                .width(None)
                .placeholder("folder token (optional)")
                .on_edit(|s: &mut AppState| &mut s.forms.rule_action)
                .build(k),
        )
        .child(
            row().child(
                Button::new("Create rule")
                    .variant(ButtonVariant::Contrast)
                    .on_press(|s: &mut AppState| {
                        let value = s.forms.rule_value.text.trim().to_string();
                        if value.is_empty() {
                            s.toast = Some("Add a condition value".into());
                            return;
                        }
                        let name = s.forms.rule_name.text.trim().to_string();
                        let token = s.forms.rule_action.text.trim().to_string();
                        let actions = if token.is_empty() {
                            serde_json::json!([])
                        } else {
                            serde_json::json!([{"type": "move_to", "folder_token": token}])
                        };
                        s.send(Action::CreateRule(serde_json::json!({
                            "name": if name.is_empty() { "Rule".to_string() } else { name },
                            "color": "#d98e5c",
                            "enabled": true,
                            "match_mode": "all",
                            "conditions": [{"field": "from", "op": "contains", "value": value}],
                            "actions": actions,
                        })));
                    })
                    .build(k),
            ),
        )
}

fn aliases_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let mut root = col().w_full().gap(8.0);
    let aliases = s.aliases.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if aliases.is_empty() {
        root = root.child(text("No aliases").font(k.ty.small).color(c.text_muted));
    }
    for alias in aliases {
        let id = alias
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let address = alias
            .get("address")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let enabled = alias
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let delete_id = id.clone();
        let route_address = address.clone();
        let pin_id = id.clone();
        let toggle_id = id.clone();
        let pinned = alias
            .get("pinned")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        root = root.child(
            row()
                .key(format!("settings-alias-{id}"))
                .w_full()
                .center_y()
                .gap(8.0)
                .padding(8.0)
                .radius(k.radius.md)
                .fill(c.surface_weak)
                .child(
                    text(format!("{address}{}", if enabled { "" } else { " (off)" }))
                        .font(k.ty.small)
                        .color(c.text_base)
                        .grow(1.0),
                )
                .child(
                    Button::new(if pinned { "Unpin" } else { "Pin" })
                        .variant(ButtonVariant::Ghost)
                        .on_press(move |s: &mut AppState| {
                            s.send(Action::ToggleAliasPin(pin_id.clone()))
                        })
                        .build(k),
                )
                .child(
                    Button::new(if enabled { "Disable" } else { "Enable" })
                        .variant(ButtonVariant::Ghost)
                        .on_press(move |s: &mut AppState| {
                            s.send(Action::SetAliasEnabled {
                                id: toggle_id.clone(),
                                enabled: !enabled,
                            })
                        })
                        .build(k),
                )
                .child(
                    Button::new("Route")
                        .variant(ButtonVariant::Ghost)
                        .on_press(move |s: &mut AppState| {
                            let mailbox = s.forms.alias_route.text.trim().to_string();
                            if mailbox.is_empty() {
                                s.toast = Some("Enter a mailbox name".into());
                                return;
                            }
                            s.send(Action::RouteAlias {
                                alias: route_address.clone(),
                                mailbox,
                            });
                        })
                        .build(k),
                )
                .child(
                    Button::new("Delete")
                        .variant(ButtonVariant::Danger)
                        .on_press(move |s: &mut AppState| {
                            s.send(Action::DeleteAlias(delete_id.clone()))
                        })
                        .build(k),
                ),
        );
    }

    root.child(el().w_full().h(1.0).fill(c.border_muted))
        .child(
            row()
                .w_full()
                .gap(6.0)
                .child(
                    TextInput::new(&s.forms.alias_local)
                        .label("Alias local part")
                        .width(None)
                        .placeholder("local part")
                        .on_edit(|s: &mut AppState| &mut s.forms.alias_local)
                        .build(k)
                        .grow(1.0),
                )
                .child(
                    TextInput::new(&s.forms.alias_domain)
                        .label("Alias domain")
                        .width(None)
                        .placeholder("domain")
                        .on_edit(|s: &mut AppState| &mut s.forms.alias_domain)
                        .build(k)
                        .grow(1.0),
                ),
        )
        .child(
            TextInput::new(&s.forms.alias_display)
                .label("Alias display name")
                .width(None)
                .placeholder("display name (optional)")
                .on_edit(|s: &mut AppState| &mut s.forms.alias_display)
                .build(k),
        )
        .child(
            row().child(
                Button::new("Create alias")
                    .variant(ButtonVariant::Contrast)
                    .on_press(|s: &mut AppState| {
                        let local = s.forms.alias_local.text.trim().to_string();
                        let domain = s.forms.alias_domain.text.trim().to_string();
                        if local.is_empty() || domain.is_empty() {
                            s.toast = Some("Alias local part and domain are required".into());
                            return;
                        }
                        let display = s.forms.alias_display.text.trim().to_string();
                        s.send(Action::CreateAlias {
                            local,
                            domain,
                            display: (!display.is_empty()).then_some(display),
                        });
                    })
                    .build(k),
            ),
        )
        .child(
            TextInput::new(&s.forms.alias_route)
                .label("Mailbox to route into")
                .width(None)
                .placeholder("mailbox to route into")
                .on_edit(|s: &mut AppState| &mut s.forms.alias_route)
                .build(k),
        )
        .child(el().w_full().h(1.0).fill(c.border_muted))
        .child(
            row()
                .w_full()
                .gap(6.0)
                .child(
                    TextInput::new(&s.forms.mailbox_name)
                        .label("New mailbox")
                        .width(None)
                        .placeholder("mailbox name")
                        .on_edit(|s: &mut AppState| &mut s.forms.mailbox_name)
                        .build(k)
                        .grow(1.0),
                )
                .child(
                    Button::new("Create mailbox")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| {
                            let name = s.forms.mailbox_name.text.trim().to_string();
                            if name.is_empty() {
                                s.toast = Some("Enter a mailbox name".into());
                                return;
                            }
                            s.send(Action::CreateMailbox(name));
                        })
                        .build(k),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(6.0)
                .child(
                    TextInput::new(&s.forms.folder_name)
                        .label("New folder")
                        .width(None)
                        .placeholder("folder name")
                        .on_edit(|s: &mut AppState| &mut s.forms.folder_name)
                        .build(k)
                        .grow(1.0),
                )
                .child(
                    TextInput::new(&s.forms.folder_parent)
                        .label("Parent folder token")
                        .width(None)
                        .placeholder("parent token (optional)")
                        .on_edit(|s: &mut AppState| &mut s.forms.folder_parent)
                        .build(k)
                        .grow(1.0),
                )
                .child(
                    Button::new("Create folder")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| {
                            let name = s.forms.folder_name.text.trim().to_string();
                            if name.is_empty() {
                                s.toast = Some("Enter a folder name".into());
                                return;
                            }
                            let parent = s.forms.folder_parent.text.trim().to_string();
                            s.send(Action::CreateFolder {
                                name,
                                parent: (!parent.is_empty()).then_some(parent),
                            });
                        })
                        .build(k),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(6.0)
                .child(
                    TextInput::new(&s.forms.folder_token)
                        .label("Delete folder token")
                        .width(None)
                        .placeholder("folder token")
                        .on_edit(|s: &mut AppState| &mut s.forms.folder_token)
                        .build(k)
                        .grow(1.0),
                )
                .child(
                    Button::new("Delete folder")
                        .variant(ButtonVariant::Danger)
                        .on_press(|s: &mut AppState| {
                            let token = s.forms.folder_token.text.trim().to_string();
                            if token.is_empty() {
                                s.toast = Some("Enter a folder token".into());
                                return;
                            }
                            s.send(Action::DeleteFolder(token));
                        })
                        .build(k),
                ),
        )
        .child(
            TextInput::new(&s.forms.folder_rename)
                .label("Folder token to rename")
                .width(None)
                .placeholder("folder token")
                .on_edit(|s: &mut AppState| &mut s.forms.folder_rename)
                .build(k),
        )
        .child(
            TextInput::new(&s.forms.folder_move_parent)
                .label("Rename / move")
                .width(None)
                .placeholder("new name (rename) or parent token (move)")
                .on_edit(|s: &mut AppState| &mut s.forms.folder_move_parent)
                .build(k),
        )
        .child(
            row()
                .gap(6.0)
                .child(
                    Button::new("Rename folder")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| {
                            let token = s.forms.folder_rename.text.trim().to_string();
                            let name = s.forms.folder_move_parent.text.trim().to_string();
                            if token.is_empty() || name.is_empty() {
                                s.toast = Some("Enter a folder token and new name".into());
                                return;
                            }
                            s.send(Action::RenameFolder { token, name });
                        })
                        .build(k),
                )
                .child(
                    Button::new("Move folder")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut AppState| {
                            let token = s.forms.folder_rename.text.trim().to_string();
                            if token.is_empty() {
                                s.toast = Some("Enter a folder token".into());
                                return;
                            }
                            let parent = s.forms.folder_move_parent.text.trim().to_string();
                            s.send(Action::MoveFolder {
                                token,
                                parent: (!parent.is_empty()).then_some(parent),
                            });
                        })
                        .build(k),
                ),
        )
}

fn contacts_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let mut root = col().w_full().gap(8.0);
    root = root
        .child(
            TextInput::new(&s.forms.contact_name)
                .label("Contact name")
                .width(None)
                .placeholder("Full name")
                .on_edit(|s: &mut AppState| &mut s.forms.contact_name)
                .build(k),
        )
        .child(
            TextInput::new(&s.forms.contact_email)
                .label("Contact email")
                .width(None)
                .placeholder("name@example.org")
                .on_edit(|s: &mut AppState| &mut s.forms.contact_email)
                .build(k),
        )
        .child(
            Button::new("Add contact")
                .variant(ButtonVariant::Contrast)
                .on_press(|s: &mut AppState| {
                    let name = s.forms.contact_name.text.trim().to_string();
                    let email = s.forms.contact_email.text.trim().to_string();
                    if name.is_empty() || !email.contains('@') {
                        s.toast = Some("Enter a name and email".into());
                        return;
                    }
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|value| value.as_secs())
                        .unwrap_or(0);
                    let uid = format!("aster-{stamp}");
                    let vcard = format!(
                    "BEGIN:VCARD\nVERSION:3.0\nUID:{uid}\nFN:{name}\nEMAIL:{email}\nEND:VCARD\n"
                );
                    s.send(Action::SaveContact { uid, vcard });
                })
                .build(k),
        );
    let contacts = contacts_of(s);
    if contacts.is_empty() {
        root = root.child(text("No contacts").font(k.ty.small).color(c.text_muted));
    }
    for contact in contacts {
        let uid = contact
            .get("uid")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let vcard = contact
            .get("vcard")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let name = vcard_field(vcard, "FN")
            .unwrap_or_else(|| vcard_field(vcard, "EMAIL").unwrap_or_default());
        let delete_uid = uid.clone();
        root = root.child(
            row()
                .key(format!("settings-contact-{uid}"))
                .w_full()
                .center_y()
                .gap(8.0)
                .padding(8.0)
                .radius(k.radius.md)
                .fill(c.surface_weak)
                .child(text(name).font(k.ty.small).color(c.text_base).grow(1.0))
                .child(
                    Button::new("Delete")
                        .variant(ButtonVariant::Danger)
                        .on_press(move |s: &mut AppState| {
                            s.send(Action::DeleteContact(delete_uid.clone()))
                        })
                        .build(k),
                ),
        );
    }
    root
}

fn security_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let pq = app_settings::boolean(&s.settings, &["security", "require_post_quantum"], false);
    let encrypt = app_settings::boolean(&s.settings, &["security", "encrypt_emails"], true);
    let sign = app_settings::boolean(&s.settings, &["security", "gpg_sign_outgoing"], false);
    col()
        .w_full()
        .gap(10.0)
        .child(toggle_row(k, "Require post-quantum", pq, move |s| {
            s.apply_settings(serde_json::json!({"security": {"require_post_quantum": !pq}}))
        }))
        .child(toggle_row(k, "Encrypt messages", encrypt, move |s| {
            s.apply_settings(serde_json::json!({"security": {"encrypt_emails": !encrypt}}))
        }))
        .child(toggle_row(k, "GPG-sign outgoing", sign, move |s| {
            s.apply_settings(serde_json::json!({"security": {"gpg_sign_outgoing": !sign}}))
        }))
        .child(
            text(format!(
                "gpg: {} · agent: {}",
                s.gpg
                    .get("available")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                s.gpg
                    .get("agent_running")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
            ))
            .font(k.ty.small)
            .color(c.text_muted),
        )
        .child(
            text(pretty(&s.pgp, 300))
                .font(k.ty.mono)
                .color(c.text_muted)
                .selectable(true),
        )
        .child(
            row()
                .gap(6.0)
                .child(
                    Button::new("Import Aster key")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| s.send(Action::ImportGpgKey))
                        .build(k),
                )
                .child(
                    Button::new("Publish WKD")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| s.send(Action::PublishPgp("wkd".into())))
                        .build(k),
                )
                .child(
                    Button::new("Publish keyserver")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut AppState| s.send(Action::PublishPgp("keyserver".into())))
                        .build(k),
                ),
        )
}

fn mcp_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let enabled = app_settings::boolean(&s.settings, &["mcp", "enabled"], false);
    let send = app_settings::boolean(&s.settings, &["mcp", "allow_send"], false);
    let destructive = app_settings::boolean(&s.settings, &["mcp", "allow_destructive"], false);
    col()
        .w_full()
        .gap(10.0)
        .child(toggle_row(k, "MCP enabled", enabled, move |s| {
            s.apply_settings(serde_json::json!({"mcp": {"enabled": !enabled}}))
        }))
        .child(toggle_row(k, "Allow MCP send", send, move |s| {
            s.apply_settings(serde_json::json!({"mcp": {"allow_send": !send}}))
        }))
        .child(toggle_row(
            k,
            "Allow destructive MCP tools",
            destructive,
            move |s| {
                s.apply_settings(serde_json::json!({"mcp": {"allow_destructive": !destructive}}))
            },
        ))
        .child(
            text(format!(
                "Endpoint and token: {}/mcp.json (written by the MCP server)",
                s.data_dir.display()
            ))
            .font(k.ty.small)
            .color(c.text_muted),
        )
}

fn diagnostics_tab(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let mut root = col()
        .w_full()
        .gap(10.0)
        .child(
            text(pretty(&s.diagnostics, 40))
                .font(k.ty.mono)
                .color(c.text_muted)
                .selectable(true),
        )
        .child(
            row()
                .gap(6.0)
                .child(
                    Button::new("Refresh")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| s.send(Action::LoadDiagnostics))
                        .build(k),
                )
                .child(
                    Button::new("Repair cache")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut AppState| s.send(Action::RepairCache))
                        .build(k),
                ),
        );
    if s.confirm_sign_out {
        root = root.child(
            row()
                .gap(6.0)
                .child(
                    text("Sign out and quit Aster?")
                        .font(k.ty.small)
                        .color(c.danger.fg),
                )
                .child(
                    Button::new("Sign out keep cache")
                        .variant(ButtonVariant::Danger)
                        .on_press(|s: &mut AppState| s.send(Action::SignOut(true)))
                        .build(k),
                )
                .child(
                    Button::new("Sign out delete cache")
                        .variant(ButtonVariant::Danger)
                        .on_press(|s: &mut AppState| s.send(Action::SignOut(false)))
                        .build(k),
                )
                .child(
                    Button::new("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut AppState| s.confirm_sign_out = false)
                        .build(k),
                ),
        );
    } else {
        root = root.child(
            Button::new("Sign out")
                .variant(ButtonVariant::Ghost)
                .on_press(|s: &mut AppState| s.confirm_sign_out = true)
                .build(k),
        );
    }
    root.child(
        text(format!("Data: {}", s.data_dir.display()))
            .font(k.ty.small)
            .color(c.text_muted),
    )
}

fn rules_of(value: &serde_json::Value) -> &[serde_json::Value] {
    value
        .get("rules")
        .and_then(|value| value.as_array())
        .or_else(|| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn contacts_of(s: &AppState) -> &[serde_json::Value] {
    s.contacts.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn rule_summary(rule: &serde_json::Value) -> String {
    let conditions = rule
        .get("conditions")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{} {} {}",
                        item.get("field")
                            .and_then(|value| value.as_str())
                            .unwrap_or("?"),
                        item.get("op")
                            .and_then(|value| value.as_str())
                            .unwrap_or("contains"),
                        item.get("value")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join(" and ")
        })
        .unwrap_or_default();
    let actions = rule
        .get("actions")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let kind = item
                        .get("type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("?");
                    let target = item
                        .get("folder_token")
                        .or_else(|| item.get("value"))
                        .or_else(|| item.get("to"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    if target.is_empty() {
                        kind.to_string()
                    } else {
                        format!("{kind} {target}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    format!("{conditions} → {actions}")
}

fn vcard_field(vcard: &str, field: &str) -> Option<String> {
    vcard.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        let key = key.split(';').next().unwrap_or(key);
        key.eq_ignore_ascii_case(field)
            .then(|| value.trim().to_string())
    })
}

fn pretty(value: &serde_json::Value, max: usize) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    if text.len() > max {
        format!("{}…", &text[..max])
    } else {
        text
    }
}

fn compose_dialog(s: &AppState, k: &Kit, layer: &mut Vec<El>) {
    if !s.compose.requested {
        return;
    }
    let body = compose_body(s, k);
    Dialog::new("compose", s.compose.heading.as_str())
        .open(true)
        .fit()
        .width(760.0)
        .child(body)
        .on_close(|s: &mut AppState| s.close_compose())
        .build(k, layer);
}

fn compose_body(s: &AppState, k: &Kit) -> El {
    let c = &k.c;
    let from = TextInput::new(&s.compose.from)
        .label("From")
        .width(None)
        .placeholder("you@example.org")
        .on_edit(|s: &mut AppState| &mut s.compose.from)
        .build(k);
    let to = TextInput::new(&s.compose.to)
        .label("To")
        .width(None)
        .placeholder("To, comma separated")
        .on_edit(|s: &mut AppState| &mut s.compose.to)
        .build(k);
    let subject = TextInput::new(&s.compose.subject)
        .label("Subject")
        .width(None)
        .placeholder("Subject")
        .on_edit(|s: &mut AppState| &mut s.compose.subject)
        .build(k);

    let mut fields = col().w_full().gap(8.0);
    if s.identities.len() > 1 {
        let mut identities = row()
            .w_full()
            .gap(6.0)
            .center_y()
            .child(text("From").font(k.ty.small).color(c.text_muted));
        for identity in &s.identities {
            let address = identity.address.clone();
            let active = s.compose.from.text.trim() == address;
            identities = identities.child(
                Button::new(address.clone())
                    .variant(if active {
                        ButtonVariant::Contrast
                    } else {
                        ButtonVariant::Ghost
                    })
                    .on_press(move |s: &mut AppState| {
                        s.compose.from = rune::TextEdit::new(address.clone());
                    })
                    .build(k),
            );
        }
        fields = fields.child(identities);
    }
    fields = fields
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    text("From")
                        .font(TextStyle::new(12.0, 400.0))
                        .color(c.text_muted)
                        .w(42.0),
                )
                .child(from.grow(1.0)),
        )
        .child(to)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(
                    TextInput::new(&s.compose.cc)
                        .label("Cc")
                        .width(None)
                        .placeholder("Cc")
                        .on_edit(|s: &mut AppState| &mut s.compose.cc)
                        .build(k)
                        .grow(1.0),
                )
                .child(
                    TextInput::new(&s.compose.bcc)
                        .label("Bcc")
                        .width(None)
                        .placeholder("Bcc")
                        .on_edit(|s: &mut AppState| &mut s.compose.bcc)
                        .build(k)
                        .grow(1.0),
                ),
        );
    fields = fields.child(subject);

    let editor = col()
        .w_full()
        .padding(12.0)
        .radius(k.radius.sm)
        .fill(c.surface_base)
        .border(1.0, c.border_base)
        .child(
            TextArea::new(&s.compose.body)
                .label("Message")
                .placeholder("Write your message…")
                .rows(14)
                .max_height(340.0)
                .width(None)
                .on_edit(|s: &mut AppState| &mut s.compose.body)
                .build(k),
        );

    let mut root = col()
        .w_full()
        .gap(10.0)
        .child(
            row().w_full().center_y().child(el().grow(1.0)).child(
                text("Markdown")
                    .font(TextStyle::new(11.0, 400.0).mono())
                    .color(c.text_accent),
            ),
        )
        .child(fields);

    let mut links = row().w_full().gap(12.0).center_y();
    links = links
        .child(link_action(
            "Insert signature",
            c.text_accent,
            k,
            |s: &mut AppState| s.insert_default_signature(),
        ))
        .child(link_action(
            "Insert template",
            c.text_accent,
            k,
            |s: &mut AppState| {
                if let Some(template) = s.templates.first() {
                    let mut text = template.content.clone();
                    if !s.compose.body.text.is_empty() {
                        text.push_str("\n\n");
                        text.push_str(&s.compose.body.text);
                    }
                    s.compose.body = rune::TextEdit::new(text);
                }
            },
        ))
        .child(el().grow(1.0))
        .child(
            text("Body · Markdown")
                .font(TextStyle::new(11.0, 500.0))
                .color(c.text_muted),
        );
    root = root.child(links).child(editor);

    if !s.compose.attachments.is_empty() {
        let mut attachments = row().w_full().gap(6.0);
        for (index, path) in s.compose.attachments.iter().enumerate() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("attachment")
                .to_string();
            let remove = index;
            attachments = attachments.child(
                row()
                    .key(format!("compose-attachment-{index}"))
                    .h(26.0)
                    .px(12.0)
                    .gap(4.0)
                    .center_y()
                    .radius(k.radius.sm)
                    .fill(c.layer_02)
                    .border(1.0, c.border_base)
                    .child(icon(&icons::ATTACH, 16.0).color(c.text_muted))
                    .child(
                        text(name)
                            .font(TextStyle::new(12.0, 400.0))
                            .color(c.text_base)
                            .truncate(),
                    )
                    .child(icon(&icons::CLOSE, 12.0).color(c.text_muted))
                    .on_press(move |s: &mut AppState| {
                        if remove < s.compose.attachments.len() {
                            s.compose.attachments.remove(remove);
                        }
                    }),
            );
        }
        root = root.child(attachments);
    }

    root = root
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    TextInput::new(&s.compose.expires_at)
                        .label("Expires")
                        .width(None)
                        .placeholder("Expires at YYYY-MM-DD HH:MM")
                        .on_edit(|s: &mut AppState| &mut s.compose.expires_at)
                        .build(k),
                )
                .child(
                    text("Obscure subject")
                        .font(TextStyle::new(12.0, 400.0))
                        .color(c.text_base),
                )
                .child(
                    Button::new(if s.compose.obscure_subject {
                        "On"
                    } else {
                        "Off"
                    })
                    .variant(if s.compose.obscure_subject {
                        ButtonVariant::Contrast
                    } else {
                        ButtonVariant::Ghost
                    })
                    .on_press(|s: &mut AppState| {
                        s.compose.obscure_subject = !s.compose.obscure_subject
                    })
                    .build(k),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    TextInput::new(&s.compose.schedule_at)
                        .label("Schedule")
                        .width(None)
                        .placeholder("Send at YYYY-MM-DD HH:MM")
                        .on_edit(|s: &mut AppState| &mut s.compose.schedule_at)
                        .build(k),
                )
                .child(link_action(
                    "Schedule",
                    c.text_accent,
                    k,
                    |s: &mut AppState| {
                        let at = s.compose.schedule_at.text.trim().to_string();
                        if at.is_empty() {
                            s.toast = Some("Enter a schedule time".into());
                            return;
                        }
                        let outgoing = s.outgoing();
                        s.compose.requested = false;
                        s.send(Action::Schedule(outgoing, at));
                    },
                )),
        );

    root.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(
                IconButton::new(&icons::ATTACH)
                    .label("Attach")
                    .variant(IconButtonVariant::Ghost)
                    .control_size(ControlSize::Regular)
                    .on_press_tx(|_: &mut AppState, tx: &mut Tx| {
                        tx.open_file(
                            rune::FileDialogOptions {
                                title: "Attach file".into(),
                                directory: None,
                                file_name: None,
                                filters: Vec::new(),
                            },
                            |s: &mut AppState, result, _| {
                                if let Ok(Some(path)) = result {
                                    s.compose.attachments.push(path);
                                }
                            },
                        );
                    })
                    .build(k)
                    .size(28.0, 28.0),
            )
            .child(el().grow(1.0))
            .child(
                text("Markdown is converted to HTML and plain text.")
                    .font(TextStyle::new(11.0, 400.0))
                    .color(c.text_muted),
            )
            .child(
                Button::new("Send")
                    .variant(ButtonVariant::Contrast)
                    .control_size(ControlSize::ExtraLarge)
                    .on_press(|s: &mut AppState| {
                        let outgoing = s.outgoing();
                        if outgoing.to.is_empty()
                            && outgoing.cc.is_empty()
                            && outgoing.bcc.is_empty()
                        {
                            s.toast = Some("Add at least one recipient".into());
                            return;
                        }
                        s.compose.requested = false;
                        s.send(Action::Send(outgoing));
                    })
                    .build(k)
                    .key("compose-send"),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    #[test]
    fn strip_tags_flattens_markup() {
        assert_eq!(strip_tags("<p>Hello <b>world</b></p>"), "Hello world");
        assert_eq!(strip_tags("<span>a</span>\n<span>b</span>"), "a b");
    }

    #[test]
    fn demo_scene_renders_rows() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::scene(&mut state);
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("message-m1").is_some());
        assert!(rt.find("message-m8").is_some());
    }

    #[test]
    fn reader_split_drag_resizes() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::scene(&mut state);
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("reader-split").expect("reader split");
        rt.pointer_down(x, y);
        rt.pointer_move(x - 60.0, y);
        rt.pointer_up(x - 60.0, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.reader_width > 380.0, "{}", rt.state.reader_width);
    }

    #[test]
    fn filters_dialog_renders() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::scene(&mut state);
        state.filters_open = true;
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("filters-dialog").is_some());
    }

    #[test]
    fn command_palette_renders() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::command_scene(&mut state);
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("command-dialog").is_some());
    }

    #[test]
    fn message_menu_renders() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::menu_scene(&mut state);
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("message-menu-menu").is_some());
    }

    #[test]
    fn settings_tabs_render() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        for tab in SettingsTab::ALL {
            let mut state = AppState::default();
            crate::demo::settings_scene(&mut state, tab);
            let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
            rt.frame(0.0);
            assert!(rt.find("settings").is_some(), "{tab:?}");
            assert!(rt.find("settings-nav").is_some(), "{tab:?}");
        }
    }

    #[test]
    fn compose_scene_renders() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::compose_scene(&mut state);
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("compose-dialog").is_some());
        assert!(rt.find("compose-send").is_some());
    }

    #[test]
    fn html_mail_body_renders() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = AppState::default();
        crate::demo::scene(&mut state);
        let mut rt = Runtime::new(state, view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("mail-body").is_some());
    }

    #[test]
    fn shell_renders_headlessly() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(AppState::default(), view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("messages").is_some());
        assert!(rt.find("sidebar").is_some());
        assert!(rt.find("reader").is_some());
        assert!(rt.find("topbar").is_some());
        assert!(rt.find("statusbar").is_some());
    }
}
