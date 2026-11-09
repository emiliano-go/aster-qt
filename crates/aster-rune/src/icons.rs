// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Qt client's stroke icon set, as Rune `Icon`s.

use rune::Icon;

macro_rules! icon {
    ($name:ident, $body:expr) => {
        pub static $name: Icon = Icon {
            name: stringify!($name),
            view: 24.0,
            body: concat!(
                "<g fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.6\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
                $body,
                "</g>"
            ),
        };
    };
}

icon!(
    INBOX,
    r#"<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>"#
);
icon!(
    SEND,
    r#"<path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/>"#
);
icon!(
    DRAFTS,
    r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>"#
);
icon!(
    ARCHIVE,
    r#"<path d="M3 4h18v4H3z"/><path d="M5 8v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>"#
);
icon!(
    TRASH,
    r#"<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"#
);
icon!(
    JUNK,
    r#"<path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#
);
icon!(
    FOLDER,
    r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#
);
icon!(
    TAG,
    r#"<path d="M12.59 2.59A2 2 0 0 0 11.17 2H4a2 2 0 0 0-2 2v7.17a2 2 0 0 0 .59 1.42l8.7 8.7a2.43 2.43 0 0 0 3.42 0l6.58-6.58a2.43 2.43 0 0 0 0-3.42z"/><path d="M7 7h.01"/>"#
);
icon!(PLUS, r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#);
icon!(
    SEARCH,
    r#"<path d="M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z"/><path d="m21 21-4.35-4.35"/>"#
);
icon!(
    SETTINGS,
    r#"<path d="M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7z"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09A1.65 1.65 0 0 0 15 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9c.14.36.38.67.68.9H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>"#
);
icon!(
    REPLY,
    r#"<path d="M9 17 4 12l5-5"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/>"#
);
icon!(
    FORWARD,
    r#"<path d="m15 17 5-5-5-5"/><path d="M4 18v-2a4 4 0 0 1 4-4h12"/>"#
);
icon!(
    ATTACH,
    r#"<path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/>"#
);
icon!(CHECK, r#"<path d="M20 6 9 17l-5-5"/>"#);
icon!(
    SHIELD,
    r#"<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>"#
);
icon!(FILTER, r#"<path d="M22 3H2l8 9.46V19l4 2v-8.54z"/>"#);
icon!(
    CLOCK,
    r#"<path d="M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z"/><path d="M12 6v6l4 2"/>"#
);
icon!(CHEVRON_LEFT, r#"<path d="m15 18-6-6 6-6"/>"#);
icon!(CHEVRON_RIGHT, r#"<path d="m9 18 6-6-6-6"/>"#);
icon!(CHEVRON_DOWN, r#"<path d="m6 9 6 6 6-6"/>"#);
icon!(
    STAR,
    r#"<path d="m12 2 3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>"#
);
icon!(
    PIN,
    r#"<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>"#
);
icon!(
    MORE,
    r#"<path d="M5 12h.01"/><path d="M12 12h.01"/><path d="M19 12h.01"/>"#
);
icon!(
    MOON,
    r#"<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>"#
);
icon!(
    SUN,
    r#"<path d="M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10z"/><path d="M12 1v2"/><path d="M12 21v2"/><path d="M4.22 4.22l1.42 1.42"/><path d="M18.36 18.36l1.42 1.42"/><path d="M1 12h2"/><path d="M21 12h2"/><path d="M4.22 19.78l1.42-1.42"/><path d="M18.36 5.64l1.42-1.42"/>"#
);
icon!(
    REFRESH,
    r#"<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/>"#
);
icon!(
    RULE,
    r#"<path d="M4 6h16"/><path d="M4 12h10"/><path d="M4 18h6"/><path d="m17 14 3 3-3 3"/>"#
);
icon!(
    PRINTER,
    r#"<path d="M6 9V2h12v7"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><path d="M6 14h12v8H6z"/>"#
);
icon!(CLOSE, r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#);
