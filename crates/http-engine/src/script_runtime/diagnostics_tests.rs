use super::{tests::input, *};

fn assert_hidden(execution: &ScriptExecutionResult, values: &[&str]) {
    let diagnostics = serde_json::to_string(execution).unwrap();
    for value in values {
        assert!(
            !diagnostics.contains(value),
            "diagnostic leaked a fixture value"
        );
    }
}

#[tokio::test]
async fn opaque_request_mutations_hide_unclassifiable_diagnostics() {
    let outcome = run_script(
        r#"
        const value = "opaque-" + "canary";
        pm.request.body.raw = "ordinary=" + value;
        console.log(value);
        pm.test(value, () => { throw new Error(value); });
        throw new Error(value);
    "#
        .into(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .unwrap();
    assert_eq!(
        outcome.request.body_raw.as_deref(),
        Some("ordinary=opaque-canary")
    );
    assert_eq!(outcome.execution.console[0].message, "<redacted>");
    assert_hidden(&outcome.execution, &["opaque-canary"]);
}

#[tokio::test]
async fn embedded_sensitive_json_in_runtime_errors_uses_conservative_shared_rules() {
    let outcome = run_script(
        r#"throw new Error('{"token":"embedded-canary"}');"#.into(),
        input(ScriptPhase::PostResponse),
    )
    .await
    .unwrap();
    assert_hidden(&outcome.execution, &["embedded-canary"]);
}

#[tokio::test]
async fn both_phases_scrub_generated_values_from_every_diagnostic_without_changing_output() {
    for phase in [ScriptPhase::PreRequest, ScriptPhase::PostResponse] {
        let outcome = run_script(
            r#"
            const a = "generated-" + "token-canary";
            const b = "ordinary-" + "slot-canary";
            const c = "dynamic-" + "signature-canary";
            console.log(a, b, c); // Also cover diagnostics emitted before writes.
            pm.variables.set("token", a);
            pm.environment.set("ordinary", b);
            pm.request.headers.upsert({key: "X-Opaque", value: c});
            console.warn({ordinary: b});
            pm.test(a + b + c, () => pm.expect([a, b, c]).to.eql([]));
            throw new Error(a + b + c);
            "#
            .into(),
            input(phase),
        )
        .await
        .unwrap();
        assert_eq!(outcome.execution.status, ScriptExecutionStatus::Failed);
        assert_eq!(outcome.execution.console.len(), 2);
        assert!(!outcome.execution.tests[0].passed);
        assert_hidden(
            &outcome.execution,
            &[
                "generated-token-canary",
                "ordinary-slot-canary",
                "dynamic-signature-canary",
            ],
        );
        assert_eq!(outcome.variables[0].value, "generated-token-canary");
        assert_eq!(outcome.environment[1].value, "ordinary-slot-canary");
        assert_eq!(outcome.request.headers[0].value, "dynamic-signature-canary");
    }
}

#[tokio::test]
async fn overwritten_unset_and_removed_values_are_still_scrubbed() {
    let outcome = run_script(
        r#"
        pm.variables.set("slot", "transient-local-canary");
        console.log(pm.variables.get("slot"));
        pm.variables.set("slot", "replacement");
        pm.variables.unset("slot");
        pm.environment.set("slot", "transient-env-canary");
        pm.test("transient-env-canary", () => { throw new Error(pm.environment.get("slot")); });
        pm.environment.unset("slot");
        pm.request.headers.upsert({key: "Authorization", value: "Bearer transient-header-canary"});
        console.error("transient-header-canary");
        pm.request.headers.remove("Authorization");
        throw new Error("transient-local-canary transient-env-canary transient-header-canary");
    "#
        .into(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .unwrap();
    assert_hidden(
        &outcome.execution,
        &[
            "transient-local-canary",
            "transient-env-canary",
            "transient-header-canary",
        ],
    );
    assert!(outcome.variables.is_empty());
    assert_eq!(outcome.environment.len(), 1);
    assert!(outcome.request.headers.is_empty());
}

#[tokio::test]
async fn request_differences_cover_direct_headers_url_and_structured_body() {
    let outcome = run_script(
        r#"
        pm.request.headers.push({key: "X-Ordinary", value: "direct-header-canary", enabled: false});
        pm.request.url += "/path-canary?ordinary=query-canary";
        pm.request.body.raw = '{"ordinary":"body-canary"}';
        console.log("direct-header-canary path-canary query-canary body-canary");
    "#
        .into(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .unwrap();
    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Success);
    assert_hidden(
        &outcome.execution,
        &[
            "direct-header-canary",
            "path-canary",
            "query-canary",
            "body-canary",
        ],
    );
    assert_eq!(
        outcome.request.body_raw.as_deref(),
        Some(r#"{"ordinary":"body-canary"}"#)
    );
}

#[tokio::test]
async fn validation_failure_scrubs_collected_console_and_tests_before_returning_fallback() {
    let outcome = run_script(r#"
        pm.variables.set("ordinary", "invalid-output-canary");
        console.log(pm.variables.get("ordinary"));
        pm.test("invalid-output-canary", () => pm.expect(pm.variables.get("ordinary")).to.equal("other"));
        pm.request.method = "";
    "#.into(), input(ScriptPhase::PreRequest)).await.unwrap();
    assert_eq!(
        outcome.execution.error.as_ref().unwrap().code,
        "SCRIPT_OUTPUT_INVALID"
    );
    assert_hidden(&outcome.execution, &["invalid-output-canary"]);
    assert_eq!(outcome.execution.console.len(), 1);
    assert_eq!(outcome.execution.tests.len(), 1);
    assert_eq!(outcome.request.method, "GET");
    assert!(outcome.variables.is_empty());
}

#[tokio::test]
async fn result_export_and_decode_failures_do_not_return_script_controlled_errors() {
    for poison in [
        r#"Object.prototype.toJSON = function() { throw new Error("export-failure-canary"); };"#,
        r#"Object.prototype.toJSON = function() { return {request: {method:"GET",url:"https://example.test",headers:[],bodyRaw:null},environment:[],variables:[],environmentChanged:false,console:[{level:"decode-failure-canary",message:"secret",sequence:0}],tests:[]}; };"#,
    ] {
        let outcome = run_script(
            format!(
                r#"
            pm.variables.set("slot", "generated-before-failure-canary");
            console.log(pm.variables.get("slot"));
            {poison}
        "#
            ),
            input(ScriptPhase::PostResponse),
        )
        .await
        .unwrap();
        assert_eq!(outcome.execution.status, ScriptExecutionStatus::Failed);
        assert_hidden(
            &outcome.execution,
            &[
                "generated-before-failure-canary",
                "export-failure-canary",
                "decode-failure-canary",
            ],
        );
        assert!(outcome.execution.console.is_empty());
        assert!(outcome.execution.tests.is_empty());
    }
}

#[tokio::test]
async fn timeout_and_output_limit_discard_diagnostics_safely() {
    for (tail, expected) in [
        ("while (true) {}", "SCRIPT_TIMEOUT"),
        (
            "pm.request.body.raw = 'x'.repeat(600000)",
            "SCRIPT_OUTPUT_LIMIT",
        ),
    ] {
        let outcome = run_script(
            format!(
                r#"
            pm.environment.set("ordinary", "failure-canary");
            console.log("failure-canary");
            pm.test("failure-canary", () => {{ throw new Error("failure-canary"); }});
            {tail}
        "#
            ),
            input(ScriptPhase::PreRequest),
        )
        .await
        .unwrap();
        assert_eq!(outcome.execution.error.as_ref().unwrap().code, expected);
        assert_hidden(&outcome.execution, &["failure-canary"]);
        if expected == "SCRIPT_OUTPUT_LIMIT" {
            assert!(outcome.execution.console.is_empty());
            assert!(outcome.execution.tests.is_empty());
            assert!(!outcome.environment_changed);
        }
    }
}

#[tokio::test]
async fn truncated_diagnostics_never_return_partial_generated_credentials() {
    let outcome = run_script(
        r#"
        const value = "truncated-canary".repeat(1000);
        console.log(value);
        pm.variables.set("ordinary", value);
    "#
        .into(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .unwrap();
    assert_eq!(outcome.execution.console[0].message, "<redacted>");
    assert_hidden(&outcome.execution, &["truncated-canary"]);
    assert_eq!(outcome.variables[0].value.len(), 16000);
}

#[tokio::test]
async fn provenance_limit_hides_diagnostics_without_changing_script_state() {
    let outcome = run_script(
        r#"
        for (let i = 0; i < 2100; ++i) pm.variables.set("ordinary", "value-" + i);
        console.log("limit-canary");
        pm.test("limit-canary", () => { throw new Error("limit-canary"); });
    "#
        .into(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .unwrap();
    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Success);
    assert_eq!(outcome.variables[0].value, "value-2099");
    assert_eq!(outcome.execution.console[0].message, "<redacted>");
    assert_eq!(outcome.execution.tests[0].name, "<redacted>");
    assert_hidden(&outcome.execution, &["limit-canary"]);
}

#[tokio::test]
async fn existing_overlapping_and_encoded_values_use_shared_longest_first_scrubbing() {
    let outcome = run_script(
        r#"
        const value = pm.environment.get("token") + "-new-canary";
        pm.variables.set("ordinary", value);
        pm.variables.set("escaped", 'quote"slash\\canary');
        pm.variables.set("encoded", "space canary");
        console.log(value, pm.environment.get("token"));
        console.log('quote\\"slash\\\\canary', "space%20canary", "space+canary");
        pm.test(value, () => pm.expect(value).to.equal("other"));
        throw new Error(value);
    "#
        .into(),
        input(ScriptPhase::PostResponse),
    )
    .await
    .unwrap();
    assert_hidden(
        &outcome.execution,
        &["secret-value", "new-canary", "canary"],
    );
    assert_eq!(outcome.variables[0].value, "secret-value-new-canary");
}
