// SPDX-License-Identifier: AGPL-3.0-or-later
//! Aster on Rune. Phase 1: shell, sidebar, message list, reader and the action bridge.
//! See docs/RUNE-MIGRATION.md.

#![recursion_limit = "512"]

mod action;
mod bridge;
mod demo;
mod icons;
mod mcp;
mod notify_linux;
mod state;
mod theme;
mod ui;

use rune::{App, AppSpec};

use crate::state::{AppState, SettingsTab};

macro_rules! settings_scene {
    ($tab:ident) => {{
        fn scene(state: &mut AppState) {
            crate::demo::settings_scene(state, SettingsTab::$tab);
        }
        scene
    }};
}

fn main() {
    rune::set_app_id("com.astermail.qt");
    let app = App::new(AppSpec::<AppState> {
        min_size: Some((980.0, 620.0)),
        on_key: Some(ui::on_key),
        poll: Some(ui::poll),
        setup: Some(state::setup),
        scenes: &[
            ("demo", demo::scene),
            ("fixture", demo::fixture_scene),
            ("compose", demo::compose_scene),
            ("command", demo::command_scene),
            ("menu", demo::menu_scene),
            ("filters", demo::filters_scene),
            ("snooze", demo::snooze_scene),
            ("delete", demo::delete_scene),
            ("preview", demo::preview_scene),
            ("outbox", demo::outbox_scene),
            ("settings", settings_scene!(Appearance)),
            ("settings-mail", settings_scene!(Mail)),
            ("settings-rules", settings_scene!(Rules)),
            ("settings-aliases", settings_scene!(Aliases)),
            ("settings-contacts", settings_scene!(Contacts)),
            ("settings-security", settings_scene!(Security)),
            ("settings-mcp", settings_scene!(Mcp)),
            ("settings-diagnostics", settings_scene!(Diagnostics)),
        ],
        ..AppSpec::new("Aster", (1440.0, 900.0), AppState::default, ui::view)
    });
    let handle = app.handle();
    if !rune::is_headless() {
        let actions = bridge::start(handle.clone());
        let data_dir = aster_client_core::ClientConfig::default().data_dir;
        crate::mcp::start(actions.clone(), data_dir);
        handle.update(move |state, _| state.actions = Some(actions));
    }
    app.run();
}
