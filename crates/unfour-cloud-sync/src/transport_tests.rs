use super::*;
use serde_json::json;

fn api_error(code: &str, details: Option<serde_json::Value>) -> crate::ApiErrorDetail {
    crate::ApiErrorDetail {
        code: code.into(),
        message: "server-owned text must not escape".into(),
        request_id: "request-123".into(),
        details,
    }
}

fn remote_problem(error: &TransportError) -> &RemoteSyncProblem {
    match error {
        TransportError::Remote(problem) => problem,
        TransportError::RemoteConflict { problem, .. } => problem,
        other => panic!("expected contextual remote error, got {other:?}"),
    }
}

#[test]
fn desktop_session_debug_is_redacted() {
    let credential =
        DesktopSessionCredential::new("very-secret-token".into(), "account-1".into(), 7)
            .expect("credential");
    let debug = format!("{credential:?}");
    assert!(!debug.contains("very-secret-token"));
    assert!(debug.contains("REDACTED"));
}

#[test]
fn remote_error_codes_drive_classification_and_keep_safe_context() {
    let operation = Some(json!({
        "operationId": "operation-1",
        "operationIndex": 2,
        "entityType": "apiRequest",
        "entityId": "request-entity"
    }));
    let cases = [
        (
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            None,
            RemoteSyncProblemCategory::Auth,
        ),
        (
            StatusCode::FORBIDDEN,
            "entitlement_required",
            None,
            RemoteSyncProblemCategory::Entitlement,
        ),
        (
            StatusCode::BAD_REQUEST,
            "protocol_version_unsupported",
            None,
            RemoteSyncProblemCategory::Protocol,
        ),
        (
            StatusCode::BAD_REQUEST,
            "invalid_sync_entity",
            operation.clone(),
            RemoteSyncProblemCategory::OperationPermanent,
        ),
        (
            StatusCode::BAD_REQUEST,
            "invalid_parent_entity",
            operation.clone(),
            RemoteSyncProblemCategory::OperationPermanent,
        ),
        (
            StatusCode::BAD_REQUEST,
            "payload_schema_version_unsupported",
            operation.clone(),
            RemoteSyncProblemCategory::OperationPermanent,
        ),
        (
            StatusCode::CONFLICT,
            "operation_id_reuse",
            operation.clone(),
            RemoteSyncProblemCategory::OperationPermanent,
        ),
        (
            StatusCode::CONFLICT,
            "sync_workspace_deleted",
            None,
            RemoteSyncProblemCategory::Workspace,
        ),
        (
            StatusCode::CONFLICT,
            "snapshot_required",
            None,
            RemoteSyncProblemCategory::SnapshotRequired,
        ),
        (
            StatusCode::NOT_FOUND,
            "sync_workspace_not_found",
            None,
            RemoteSyncProblemCategory::Workspace,
        ),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            None,
            RemoteSyncProblemCategory::RequestPermanent,
        ),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            operation.clone(),
            RemoteSyncProblemCategory::OperationPermanent,
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            None,
            RemoteSyncProblemCategory::Retryable,
        ),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            None,
            RemoteSyncProblemCategory::Retryable,
        ),
    ];
    for (status, code, details, category) in cases {
        let error = classify_api_error(status, SyncPhase::Push, Some(api_error(code, details)));
        let problem = remote_problem(&error);
        assert_eq!(problem.category, category, "wrong category for {code}");
        assert_eq!(problem.server_error_code, code);
        assert_eq!(problem.request_id.as_deref(), Some("request-123"));
        assert_eq!(problem.http_status, Some(status.as_u16()));
        assert_eq!(problem.phase, SyncPhase::Push);
    }
}

#[test]
fn operation_context_is_explicit_and_never_inferred_from_array_position() {
    let classified = classify_api_error(
        StatusCode::BAD_REQUEST,
        SyncPhase::Push,
        Some(api_error(
            "invalid_sync_entity",
            Some(json!({
                "operationId": "operation-1",
                "operationIndex": 3,
                "entityType": "apiRequest",
                "entityId": "entity-1"
            })),
        )),
    );
    let problem = remote_problem(&classified);
    assert_eq!(problem.operation_id.as_deref(), Some("operation-1"));
    assert_eq!(problem.operation_index, Some(3));
    assert_eq!(problem.entity_type.as_deref(), Some("apiRequest"));
    assert_eq!(problem.entity_id.as_deref(), Some("entity-1"));

    let missing = classify_api_error(
        StatusCode::BAD_REQUEST,
        SyncPhase::Push,
        Some(api_error(
            "invalid_sync_entity",
            Some(json!({ "entityId": "must-not-be-treated-as-an-operation" })),
        )),
    );
    assert_eq!(remote_problem(&missing).operation_id, None);

    let nested_code = classify_api_error(
        StatusCode::BAD_REQUEST,
        SyncPhase::Push,
        Some(api_error(
            "batch_rejected",
            Some(json!({
                "failedOperation": {
                    "operationId": "operation-2",
                    "errorCode": "invalid_parent_entity"
                }
            })),
        )),
    );
    let problem = remote_problem(&nested_code);
    assert_eq!(
        problem.category,
        RemoteSyncProblemCategory::OperationPermanent
    );
    assert_eq!(problem.server_error_code, "invalid_parent_entity");
    assert_eq!(problem.operation_id.as_deref(), Some("operation-2"));
}

#[test]
fn base_version_conflict_requires_structured_conflict_details() {
    let details = json!({
        "entityType": "workspace",
        "entityId": "workspace-1",
        "parentEntityId": null,
        "serverVersion": 2,
        "operation": "upsert",
        "payloadSchemaVersion": 1,
        "payload": { "id": "workspace-1", "name": "Remote" }
    });
    let classified = classify_api_error(
        StatusCode::CONFLICT,
        SyncPhase::Push,
        Some(api_error("base_version_conflict", Some(details))),
    );
    assert!(matches!(classified, TransportError::RemoteConflict { .. }));

    let malformed = classify_api_error(
        StatusCode::CONFLICT,
        SyncPhase::Push,
        Some(api_error("base_version_conflict", None)),
    );
    assert_eq!(
        remote_problem(&malformed).category,
        RemoteSyncProblemCategory::InvalidResponse
    );
}

#[test]
fn malformed_error_envelope_is_an_invalid_response_with_phase() {
    let classified = classify_api_error(StatusCode::BAD_REQUEST, SyncPhase::Snapshot, None);
    let problem = remote_problem(&classified);
    assert_eq!(problem.category, RemoteSyncProblemCategory::InvalidResponse);
    assert_eq!(problem.server_error_code, "invalid_api_response");
    assert_eq!(problem.request_id, None);
    assert_eq!(problem.phase, SyncPhase::Snapshot);

    let server_error = classify_api_error(StatusCode::BAD_GATEWAY, SyncPhase::Changes, None);
    let problem = remote_problem(&server_error);
    assert_eq!(problem.category, RemoteSyncProblemCategory::Retryable);
    assert_eq!(problem.server_error_code, "server_error");
    assert_eq!(problem.http_status, Some(502));

    let blank_code = classify_api_error(
        StatusCode::BAD_REQUEST,
        SyncPhase::CreateWorkspace,
        Some(api_error("   ", None)),
    );
    let problem = remote_problem(&blank_code);
    assert_eq!(problem.category, RemoteSyncProblemCategory::InvalidResponse);
    assert_eq!(problem.server_error_code, "invalid_api_response");
    assert_eq!(problem.request_id.as_deref(), Some("request-123"));
}

#[test]
fn bare_401_remains_recoverable_auth_failure() {
    let classified = classify_api_error(StatusCode::UNAUTHORIZED, SyncPhase::Changes, None);
    assert_eq!(
        remote_problem(&classified).category,
        RemoteSyncProblemCategory::Auth
    );
}

#[test]
fn transport_failures_preserve_phase_and_push_result_uncertainty() {
    for code in [
        "request_timeout",
        "connection_failed",
        "response_body_interrupted",
    ] {
        let push = transport_failure_problem(SyncPhase::Push, code, None);
        assert_eq!(push.server_error_code, code);
        assert_eq!(push.phase, SyncPhase::Push);
        assert_eq!(push.category, RemoteSyncProblemCategory::ResultUnknown);

        let pull = transport_failure_problem(SyncPhase::Changes, code, Some(200));
        assert_eq!(pull.server_error_code, code);
        assert_eq!(pull.phase, SyncPhase::Changes);
        assert_eq!(pull.http_status, Some(200));
        assert_eq!(pull.category, RemoteSyncProblemCategory::Retryable);
    }
}
