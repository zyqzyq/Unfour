use serde::Serialize;
use std::future::Future;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use unfour_account::{AccountError, AccountService, AccountState, BillingUrl};
use unfour_cloud_sync::{SyncAccountContext, SyncError, CLOUD_SYNC_ENTITLEMENT};

pub struct AccountAppState {
    service: AccountService,
}

impl AccountAppState {
    pub fn new(
        api_base_url: &str,
        web_base_url: &str,
        allow_loopback_http: bool,
    ) -> Result<Self, AccountError> {
        Ok(Self {
            service: AccountService::new(api_base_url, web_base_url, allow_loopback_http)?,
        })
    }

    pub fn service(&self) -> AccountService {
        self.service.clone()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCommandError {
    code: &'static str,
    message: String,
}

impl From<AccountError> for AccountCommandError {
    fn from(error: AccountError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SyncAccountContextState {
    Ready,
    Inactive,
    Error { code: &'static str },
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountStateSnapshot {
    account: AccountState,
    sync_context: SyncAccountContextState,
}

async fn reconcile_sync_account_context<Activate, ActivateFuture, Deactivate, DeactivateFuture>(
    access: &crate::sync::SyncAccessGate,
    account_state: &AccountState,
    generation: u64,
    activate: Activate,
    deactivate: Deactivate,
) -> SyncAccountContextState
where
    Activate: FnOnce(SyncAccountContext) -> ActivateFuture,
    ActivateFuture: Future<Output = Result<(), SyncError>>,
    Deactivate: FnOnce() -> DeactivateFuture,
    DeactivateFuture: Future<Output = Result<(), SyncError>>,
{
    // A successfully refreshed remote state immediately closes the network
    // boundary. It is reopened only after the matching local context commits.
    access.deny();
    let result = match account_state {
        AccountState::SignedIn { profile }
            if account_state.has_active_entitlement(CLOUD_SYNC_ENTITLEMENT) =>
        {
            activate(SyncAccountContext {
                account_id: profile.id.clone(),
                generation,
            })
            .await
            .map(|_| SyncAccountContextState::Ready)
        }
        AccountState::SigningIn => Ok(SyncAccountContextState::Inactive),
        AccountState::SignedOut | AccountState::SignedIn { .. } => deactivate()
            .await
            .map(|_| SyncAccountContextState::Inactive),
    };
    match result {
        Ok(SyncAccountContextState::Ready) => {
            access.allow(generation);
            SyncAccountContextState::Ready
        }
        Ok(state) => state,
        Err(error) => SyncAccountContextState::Error { code: error.code() },
    }
}

async fn update_sync_account_context(
    account: &AccountService,
    sync_state: &crate::sync::SyncAppState,
    account_state: &AccountState,
) -> SyncAccountContextState {
    // The just-fetched /v1/me response is authoritative. Do not let an older
    // positive cache survive a revoked entitlement if local cleanup fails.
    account.invalidate_entitlement_cache();
    let sync_context = reconcile_sync_account_context(
        &sync_state.access,
        account_state,
        account.generation(),
        |context| {
            sync_state
                .service
                .activate_verified_account_context(context)
        },
        || sync_state.service.deactivate_account_context(),
    )
    .await;
    if sync_context == SyncAccountContextState::Ready {
        // The activation helper commits local context before returning, while
        // reconcile_sync_account_context opens the credential gate only after
        // that commit succeeds. Schedule the worker at this boundary so a
        // re-login drains preserved outbox rows without racing entitlement
        // validation.
        sync_state.service.schedule_account_sync();
    }
    sync_context
}

async fn sign_out_with_sync_cleanup<AccountFuture, SyncFuture>(
    account_sign_out: AccountFuture,
    deactivate_sync: SyncFuture,
) -> Result<AccountState, AccountCommandError>
where
    AccountFuture: Future<Output = Result<AccountState, AccountError>>,
    SyncFuture: Future<Output = Result<(), unfour_cloud_sync::SyncError>>,
{
    // The credential boundary and account generation are always handled first.
    let account_result = account_sign_out.await;
    let sync_result = deactivate_sync.await;
    match (account_result, sync_result) {
        (Ok(account_state), Ok(())) => Ok(account_state),
        (Ok(_), Err(_)) => Err(AccountCommandError {
            code: "sign_out_sync_deactivation_failed",
            message: "Signed out locally, but Cloud Sync storage could not be paused.".into(),
        }),
        (Err(account_error), Ok(())) => Err(account_error.into()),
        (Err(_), Err(_)) => Err(AccountCommandError {
            code: "sign_out_cleanup_failed",
            message: "Local sign-out cleanup did not complete.".into(),
        }),
    }
}

#[tauri::command]
pub async fn account_get_state(
    state: State<'_, AccountAppState>,
    sync_state: State<'_, crate::sync::SyncAppState>,
) -> Result<AccountStateSnapshot, AccountCommandError> {
    let account_state = state.service.state().await?;
    let sync_context =
        update_sync_account_context(&state.service, &sync_state, &account_state).await;
    Ok(AccountStateSnapshot {
        account: account_state,
        sync_context,
    })
}

#[tauri::command]
pub async fn account_begin_sign_in(
    app: AppHandle,
    state: State<'_, AccountAppState>,
) -> Result<AccountState, AccountCommandError> {
    let authorization_url = state.service.begin_sign_in().await?;
    if let Err(error) = app
        .opener()
        .open_url(authorization_url.as_str(), None::<&str>)
    {
        state.service.cancel_sign_in();
        return Err(AccountCommandError {
            code: "browser_open_failed",
            message: format!("failed to open the account sign-in page: {error}"),
        });
    }
    Ok(AccountState::SigningIn)
}

#[tauri::command]
pub async fn account_handle_deep_link(
    url: String,
    state: State<'_, AccountAppState>,
    sync_state: State<'_, crate::sync::SyncAppState>,
) -> Result<AccountStateSnapshot, AccountCommandError> {
    let account_state = state
        .service
        .handle_deep_link(&url)
        .await
        .map_err(AccountCommandError::from)?;
    let sync_context =
        update_sync_account_context(&state.service, &sync_state, &account_state).await;
    Ok(AccountStateSnapshot {
        account: account_state,
        sync_context,
    })
}

#[tauri::command]
pub async fn account_sign_out(
    state: State<'_, AccountAppState>,
    sync_state: State<'_, crate::sync::SyncAppState>,
) -> Result<AccountState, AccountCommandError> {
    sync_state.access.deny();
    sign_out_with_sync_cleanup(
        state.service.sign_out(),
        sync_state.service.deactivate_account_context(),
    )
    .await
}

#[derive(Clone, Copy)]
enum BillingPage {
    Checkout,
    Portal,
}

impl BillingPage {
    fn open_error(self) -> AccountCommandError {
        match self {
            Self::Checkout => AccountCommandError {
                code: "checkout_page_open_failed",
                message: "The billing checkout page could not be opened.".into(),
            },
            Self::Portal => AccountCommandError {
                code: "billing_portal_open_failed",
                message: "The billing portal page could not be opened.".into(),
            },
        }
    }
}

async fn create_and_open_billing_page<F, E>(
    create_url: F,
    page: BillingPage,
    open: impl FnOnce(&str) -> Result<(), E>,
) -> Result<(), AccountCommandError>
where
    F: Future<Output = Result<BillingUrl, AccountError>>,
{
    let url = create_url.await?;
    // Do not include the opener error in the IPC response. Platform errors can
    // echo their input and must never become a route for credential leakage.
    open(url.as_str()).map_err(|_| page.open_error())
}

#[tauri::command]
pub async fn account_open_upgrade(
    app: AppHandle,
    state: State<'_, AccountAppState>,
) -> Result<(), AccountCommandError> {
    create_and_open_billing_page(
        state.service.create_billing_checkout(),
        BillingPage::Checkout,
        |trusted_url| app.opener().open_url(trusted_url, None::<&str>),
    )
    .await
}

#[tauri::command]
pub async fn account_open_web_account(
    app: AppHandle,
    state: State<'_, AccountAppState>,
) -> Result<(), AccountCommandError> {
    create_and_open_billing_page(
        state.service.create_billing_portal(),
        BillingPage::Portal,
        |trusted_url| app.opener().open_url(trusted_url, None::<&str>),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use unfour_account::{AccountSummary, EntitlementStatus, EntitlementSummary};

    fn signed_in_account(entitled: bool) -> AccountState {
        AccountState::SignedIn {
            profile: AccountSummary {
                id: "account-a".into(),
                email: "account-a@example.test".into(),
                username: Some("account-a".into()),
                display_name: Some("Account A".into()),
                avatar_url: None,
                entitlements: entitled
                    .then(|| EntitlementSummary {
                        code: CLOUD_SYNC_ENTITLEMENT.into(),
                        status: EntitlementStatus::Active,
                        valid_until: None,
                    })
                    .into_iter()
                    .collect(),
                devices: Vec::new(),
            },
        }
    }

    #[test]
    fn account_state_survives_sync_activation_failure_and_stays_closed() {
        tauri::async_runtime::block_on(async {
            let access = crate::sync::SyncAccessGate::default();
            let account = signed_in_account(true);
            let sync_context = reconcile_sync_account_context(
                &access,
                &account,
                7,
                |_| async { Err(SyncError::Storage) },
                || async { Ok(()) },
            )
            .await;
            let snapshot = AccountStateSnapshot {
                account,
                sync_context,
            };

            assert!(matches!(snapshot.account, AccountState::SignedIn { .. }));
            assert_eq!(
                snapshot.sync_context,
                SyncAccountContextState::Error {
                    code: "cloud_sync_storage_failed"
                }
            );
            let payload = serde_json::to_value(&snapshot).expect("serialize account snapshot");
            assert_eq!(payload["account"]["kind"], "signedIn");
            assert_eq!(payload["syncContext"]["kind"], "error");
            assert_eq!(payload["syncContext"]["code"], "cloud_sync_storage_failed");
            assert!(!access.is_allowed_for(7));
        });
    }

    #[test]
    fn revoked_entitlement_survives_cleanup_failure_and_closes_sync() {
        tauri::async_runtime::block_on(async {
            let access = crate::sync::SyncAccessGate::default();
            access.allow(8);
            let account = signed_in_account(false);
            let sync_context = reconcile_sync_account_context(
                &access,
                &account,
                8,
                |_| async { Ok(()) },
                || async { Err(SyncError::Storage) },
            )
            .await;
            let snapshot = AccountStateSnapshot {
                account,
                sync_context,
            };

            assert!(matches!(
                snapshot.account,
                AccountState::SignedIn { ref profile }
                    if profile.entitlements.is_empty()
            ));
            assert_eq!(
                snapshot.sync_context,
                SyncAccountContextState::Error {
                    code: "cloud_sync_storage_failed"
                }
            );
            let payload = serde_json::to_value(&snapshot).expect("serialize account snapshot");
            assert_eq!(payload["account"]["kind"], "signedIn");
            assert!(payload["account"]["profile"]["entitlements"]
                .as_array()
                .is_some_and(|values| values.is_empty()));
            assert_eq!(payload["syncContext"]["kind"], "error");
            assert!(!access.is_allowed_for(8));
        });
    }

    #[test]
    fn suspended_entitlement_deactivates_context_and_closes_sync() {
        tauri::async_runtime::block_on(async {
            let access = crate::sync::SyncAccessGate::default();
            access.allow(0);
            let mut account = signed_in_account(true);
            let AccountState::SignedIn { profile } = &mut account else {
                panic!("expected signed-in account");
            };
            profile.entitlements[0].status = EntitlementStatus::Suspended;
            let deactivated = AtomicBool::new(false);

            let sync_context = reconcile_sync_account_context(
                &access,
                &account,
                9,
                |_| async { Ok(()) },
                || async {
                    deactivated.store(true, Ordering::SeqCst);
                    Ok(())
                },
            )
            .await;

            assert!(matches!(account, AccountState::SignedIn { .. }));
            assert_eq!(sync_context, SyncAccountContextState::Inactive);
            assert!(deactivated.load(Ordering::SeqCst));
            assert!(!access.is_allowed_for(9));
        });
    }

    #[test]
    fn normal_activation_and_deactivation_preserve_context_behavior() {
        tauri::async_runtime::block_on(async {
            let access = crate::sync::SyncAccessGate::default();
            let activated = AtomicBool::new(false);
            let activated_ref = &activated;
            let active = signed_in_account(true);
            let state = reconcile_sync_account_context(
                &access,
                &active,
                9,
                |context| async move {
                    assert_eq!(context.account_id, "account-a");
                    assert_eq!(context.generation, 9);
                    activated_ref.store(true, Ordering::SeqCst);
                    Ok(())
                },
                || async { Ok(()) },
            )
            .await;
            assert_eq!(state, SyncAccountContextState::Ready);
            assert!(activated.load(Ordering::SeqCst));
            assert!(access.is_allowed_for(9));

            let deactivated = AtomicBool::new(false);
            let state = reconcile_sync_account_context(
                &access,
                &AccountState::SignedOut,
                10,
                |_| async { Ok(()) },
                || async {
                    deactivated.store(true, Ordering::SeqCst);
                    Ok(())
                },
            )
            .await;
            assert_eq!(state, SyncAccountContextState::Inactive);
            assert!(deactivated.load(Ordering::SeqCst));
            assert!(!access.is_allowed_for(10));
        });
    }

    #[test]
    fn command_errors_expose_stable_codes_without_session_data() {
        let error = AccountCommandError::from(AccountError::StateMismatch);
        let value = serde_json::to_value(error).expect("serialize account error");
        assert_eq!(value["code"], "state_mismatch");
        assert!(value.get("accessToken").is_none());
        assert!(value.get("refreshToken").is_none());
        assert!(value.get("sessionToken").is_none());
    }

    #[test]
    fn opener_receives_only_the_validated_api_billing_url() {
        tauri::async_runtime::block_on(async {
            let session_token = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde";
            let url = BillingUrl::from_api_response(
                "https://checkout.example.test/session/checkout-1",
                session_token,
            )
            .expect("valid API URL");
            let mut opened = None;
            create_and_open_billing_page(async { Ok(url) }, BillingPage::Checkout, |trusted_url| {
                opened = Some(trusted_url.to_string());
                Ok::<_, ()>(())
            })
            .await
            .expect("open trusted page");
            assert_eq!(
                opened.as_deref(),
                Some("https://checkout.example.test/session/checkout-1")
            );
        });
    }

    #[test]
    fn opener_failures_use_stable_sanitized_errors() {
        tauri::async_runtime::block_on(async {
            let url = BillingUrl::from_api_response(
                "https://billing.example.test/portal/portal-1",
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde",
            )
            .expect("valid API URL");
            let error =
                create_and_open_billing_page(async { Ok(url) }, BillingPage::Portal, |_| {
                    Err("sessionToken=must-not-escape")
                })
                .await
                .expect_err("opener failure");
            assert_eq!(error.code, "billing_portal_open_failed");
            let encoded = serde_json::to_string(&error).expect("serialize error");
            assert!(!encoded.contains("must-not-escape"));
            assert!(!encoded.contains("sessionToken"));
        });
    }

    #[test]
    fn invalid_api_billing_urls_never_reach_the_opener() {
        tauri::async_runtime::block_on(async {
            let session_token = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde";
            for malicious in [
                "http://billing.example.test/checkout",
                "javascript:alert(1)",
                "https://user:password@billing.example.test/checkout",
                "not a URL",
                "https://billing.example.test/?desktopSession=ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde",
            ] {
                let mut opened = false;
                let result = BillingUrl::from_api_response(malicious, session_token);
                let error = create_and_open_billing_page(
                    async { result },
                    BillingPage::Checkout,
                    |_| {
                        opened = true;
                        Ok::<_, ()>(())
                    },
                )
                .await
                .expect_err("reject invalid API URL");
                assert_eq!(error.code, "invalid_billing_url");
                assert!(!opened, "opened invalid URL: {malicious}");
            }
        });
    }

    #[test]
    fn sqlite_deactivation_failure_cannot_retain_the_local_session() {
        tauri::async_runtime::block_on(async {
            let session_present = AtomicBool::new(true);
            let result = sign_out_with_sync_cleanup(
                async {
                    session_present.store(false, Ordering::SeqCst);
                    Ok(AccountState::SignedOut)
                },
                async {
                    assert!(!session_present.load(Ordering::SeqCst));
                    Err(unfour_cloud_sync::SyncError::Storage)
                },
            )
            .await;

            let error = result.expect_err("surface SQLite cleanup failure");
            assert_eq!(error.code, "sign_out_sync_deactivation_failed");
            assert!(!session_present.load(Ordering::SeqCst));
            let encoded = serde_json::to_string(&error).expect("serialize error");
            for forbidden in ["sessionToken", "accessToken", "refreshToken"] {
                assert!(!encoded.contains(forbidden));
            }
        });
    }
}
