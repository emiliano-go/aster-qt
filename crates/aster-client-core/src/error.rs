// SPDX-License-Identifier: AGPL-3.0-or-later
use thiserror::Error;

pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("not signed in: link this device first")]
    NotSignedIn,
    #[error("this data directory is already in use: {0}")]
    AlreadyRunning(String),
    #[error("bridge access requires a Star plan or higher ({0})")]
    PlanRequired(String),
    #[error("plan check failed: {0}")]
    PlanCheckFailed(String),
    #[error("device was revoked")]
    DeviceRevoked,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("secret storage error: {0}")]
    Secrets(String),
    #[error("session error: {0}")]
    Session(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

impl ClientError {
    pub fn from_bridge(err: aster_bridge_core::error::BridgeError) -> Self {
        match err {
            aster_bridge_core::error::BridgeError::Auth(message)
                if message.contains("first-time setup required") =>
            {
                ClientError::NotSignedIn
            }
            other => ClientError::Session(other.to_string()),
        }
    }
}
