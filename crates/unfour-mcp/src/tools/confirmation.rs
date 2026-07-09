use serde::Serialize;
use serde_json::{json, Map, Value};

use super::policy::ToolPolicyEvaluation;
use super::ToolCallError;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfirmationRequired {
    pub risk_level: &'static str,
    pub reason: String,
    pub confirmation_text: String,
    pub confirmation_hint: String,
    pub details: Value,
}

pub(super) fn ensure_confirmed(
    arguments: &Map<String, Value>,
    code: &str,
    reason: &str,
    payload: Value,
) -> Result<(), ToolCallError> {
    let confirmation_text = confirmation_text(code, &payload);

    if is_confirmed(arguments, code, &payload) {
        return Ok(());
    }

    Err(ToolCallError::ConfirmationRequired(ConfirmationRequired {
        risk_level: "high",
        reason: reason.to_string(),
        confirmation_hint: format!(
            "Re-run with confirm=true and confirmation_text=\"{}\" to execute. The confirmation text is tied to this exact request payload.",
            confirmation_text
        ),
        confirmation_text,
        details: json!({
            "riskCode": code,
            "payloadFingerprint": payload_fingerprint(&payload),
            "dryRun": true,
        }),
    }))
}

/// Policy-aware wrapper around [`ensure_confirmed`].
///
/// The handler decides *which* actions are risky (high-risk SSH command,
/// sensitive path, destructive SQL, …); this function decides *whether* the
/// active MCP policy tier demands a confirmation prompt for that risk. Under
/// `full_access` the policy trusts the calling agent and this returns `Ok(())`
/// immediately, so `guarded` and `full_access` no longer collapse into the
/// same unconditional-confirmation behavior.
pub(super) fn ensure_confirmed_if_guarded(
    evaluation: &ToolPolicyEvaluation,
    arguments: &Map<String, Value>,
    code: &str,
    reason: &str,
    payload: Value,
) -> Result<(), ToolCallError> {
    if !evaluation.requires_confirmation() {
        return Ok(());
    }
    ensure_confirmed(arguments, code, reason, payload)
}

pub(super) fn is_confirmed(arguments: &Map<String, Value>, code: &str, payload: &Value) -> bool {
    let confirm = arguments
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let supplied = arguments
        .get("confirmationText")
        .or_else(|| arguments.get("confirmation_text"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    confirm && supplied == confirmation_text(code, payload)
}

pub(super) fn confirmation_text(code: &str, payload: &Value) -> String {
    format!("{}:{}", code, payload_fingerprint(payload))
}

fn payload_fingerprint(payload: &Value) -> String {
    fnv1a_hex8(&canonical_json(payload))
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(values) => {
            let items = values.iter().map(canonical_json).collect::<Vec<_>>();
            format!("[{}]", items.join(","))
        }
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let items = entries
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", items.join(","))
        }
    }
}

fn fnv1a_hex8(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", hash & 0xffff_ffff)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn confirmation_text_changes_when_payload_changes() {
        let a = confirmation_text(
            "DELETE_WITHOUT_WHERE",
            &json!({ "sql": "delete from users" }),
        );
        let b = confirmation_text(
            "DELETE_WITHOUT_WHERE",
            &json!({ "sql": "delete from users where id = 1" }),
        );

        assert_ne!(a, b);
        assert!(a.starts_with("DELETE_WITHOUT_WHERE:"));
    }

    #[test]
    fn ensure_confirmed_requires_exact_text() {
        let payload = json!({ "command": "rm -rf build" });
        let required = confirmation_text("SSH_RM_RF", &payload);

        assert!(ensure_confirmed(
            json!({}).as_object().unwrap(),
            "SSH_RM_RF",
            "danger",
            payload.clone()
        )
        .is_err());

        assert!(ensure_confirmed(
            json!({ "confirm": true, "confirmation_text": "wrong" })
                .as_object()
                .unwrap(),
            "SSH_RM_RF",
            "danger",
            payload.clone()
        )
        .is_err());

        assert!(ensure_confirmed(
            json!({ "confirm": true, "confirmation_text": required })
                .as_object()
                .unwrap(),
            "SSH_RM_RF",
            "danger",
            payload
        )
        .is_ok());
    }

    #[test]
    fn guarded_requires_confirmation_but_full_access_skips() {
        use super::super::policy::{
            McpCapability, McpRisk, ResolvedMcpPolicy, ToolPolicyEvaluation, WorkspacePolicyContext,
        };

        let payload = json!({ "sql": "delete from users" });
        let code = "DELETE_WITHOUT_WHERE";

        let guarded = ToolPolicyEvaluation {
            workspace: WorkspacePolicyContext {
                workspace_id: "ws".to_string(),
                workspace_name: "ws".to_string(),
                environment_type: "test".to_string(),
                mcp_policy: "guarded".to_string(),
            },
            resolved_policy: ResolvedMcpPolicy::Guarded,
            capability: McpCapability::DbDataWrite,
            risk: McpRisk::Write,
        };

        // Guarded tier: not yet confirmed, so a confirmation prompt is required.
        assert!(ensure_confirmed_if_guarded(
            &guarded,
            json!({}).as_object().unwrap(),
            code,
            "destructive",
            payload.clone()
        )
        .is_err());

        // FullAccess tier: same risky payload, but no confirmation prompt.
        let full = ToolPolicyEvaluation {
            resolved_policy: ResolvedMcpPolicy::FullAccess,
            ..guarded.clone()
        };
        assert!(ensure_confirmed_if_guarded(
            &full,
            json!({}).as_object().unwrap(),
            code,
            "destructive",
            payload
        )
        .is_ok());
    }
}
