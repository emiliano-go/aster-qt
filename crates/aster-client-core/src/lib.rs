// SPDX-License-Identifier: AGPL-3.0-or-later
pub mod compose;
pub mod email_dark;
pub mod config;
pub mod error;
pub mod rules;
pub mod runtime;
pub mod service;
pub mod pgp_agent;
pub mod settings;

pub use config::ClientConfig;
pub use error::{ClientError, ClientResult};
pub use runtime::{AccountStatus, ClientRuntime, RuntimeOptions};
pub use service::{CachedAttachmentView, MailboxView, MessageView};
