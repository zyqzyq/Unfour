use serde::Serialize;

use crate::AccountError;

/// Safe diagnostic context only; never contains callback URLs or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountFailureStage {
    PrepareSignIn,
    OpenBrowser,
    CallbackValidation,
    CodeExchange,
    CredentialRead,
    CredentialSave,
    AccountRefresh,
    SignOut,
}

impl AccountError {
    pub fn at_stage(self, stage: AccountFailureStage) -> Self {
        Self::AtStage {
            stage,
            source: Box::new(self),
        }
    }

    pub fn stage(&self) -> Option<AccountFailureStage> {
        match self {
            Self::AtStage { stage, .. } => Some(*stage),
            Self::InvalidDeepLink(_)
            | Self::NoPendingAuthorization
            | Self::StateMismatch
            | Self::AuthorizationDenied => Some(AccountFailureStage::CallbackValidation),
            _ => None,
        }
    }
}
