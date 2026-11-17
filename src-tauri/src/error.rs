use serde::{Serialize, Serializer};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("vault locked")]
    VaultLocked,
    #[error("invalid input")]
    InvalidInput,
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("storage operation failed")]
    Storage,
    #[error("cryptographic operation failed")]
    Crypto,
    #[error("conflicting operation")]
    Conflict,
    #[error("item not found")]
    NotFound,
    #[error("invalid parameters")]
    InvalidParameters,
}

#[derive(Serialize)]
struct SafeErrorPayload {
    code: &'static str,
    message: &'static str,
}

impl AppError {
    fn safe_payload(&self) -> SafeErrorPayload {
        match self {
            Self::VaultLocked => SafeErrorPayload {
                code: "vault_locked",
                message: "The vault is locked.",
            },
            Self::InvalidInput => SafeErrorPayload {
                code: "invalid_input",
                message: "The provided input is invalid.",
            },
            Self::AuthenticationFailed => SafeErrorPayload {
                code: "authentication_failed",
                message: "Authentication failed.",
            },
            Self::Storage => SafeErrorPayload {
                code: "storage",
                message: "The local vault could not be accessed.",
            },
            Self::Crypto => SafeErrorPayload {
                code: "crypto",
                message: "The secure operation could not be completed.",
            },
            Self::Conflict => SafeErrorPayload {
                code: "conflict",
                message: "The requested change conflicts with existing data.",
            },
            Self::NotFound => SafeErrorPayload {
                code: "not_found",
                message: "The requested item was not found.",
            },
            Self::InvalidParameters => SafeErrorPayload {
                code: "invalid_parameters",
                message: "The requested parameters are invalid.",
            },
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.safe_payload().serialize(serializer)
    }
}
