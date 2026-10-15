use super::*;

pub(super) fn input(phase: ScriptPhase) -> ScriptRunInput {
    ScriptRunInput {
        phase,
        request: ScriptRequestContext {
            method: "GET".to_string(),
            url: "https://example.test".to_string(),
            headers: vec![],
            body_raw: None,
        },
        response: None,
        environment: vec![ScriptVariable {
            key: "token".to_string(),
            value: "secret-value".to_string(),
            enabled: true,
        }],
        variables: vec![],
        has_environment: true,
        redactions: vec!["secret-value".to_string()],
    }
}

#[tokio::test]
async fn environment_variables_are_transactional_output() {
    let outcome = run_script(
        r#"pm.environment.set("next", pm.environment.get("token")); pm.environment.unset("token");"#.to_string(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .expect("run script");
    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Success);
    assert!(outcome.environment_changed);
    assert_eq!(outcome.environment.len(), 1);
    assert_eq!(outcome.environment[0].key, "next");
    assert_eq!(outcome.environment[0].value, "secret-value");
}

#[tokio::test]
async fn temporary_variables_override_environment() {
    let mut context = input(ScriptPhase::PreRequest);
    context.variables.push(ScriptVariable {
        key: "token".to_string(),
        value: "temporary".to_string(),
        enabled: true,
    });
    let outcome = run_script(
        r#"pm.request.url = "https://example.test/" + pm.variables.get("token");"#.to_string(),
        context,
    )
    .await
    .expect("run script");
    assert_eq!(outcome.request.url, "https://example.test/temporary");
}

#[tokio::test]
async fn request_headers_url_and_body_can_be_changed() {
    let outcome = run_script(
        r#"
          pm.request.method = "POST";
          pm.request.url = "https://example.test/changed";
          pm.request.headers.upsert({ key: "Authorization", value: "Bearer abc" });
          pm.request.body.raw = "payload";
        "#
        .to_string(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .expect("run script");
    assert_eq!(outcome.request.method, "POST");
    assert_eq!(outcome.request.url, "https://example.test/changed");
    assert_eq!(outcome.request.body_raw.as_deref(), Some("payload"));
    assert_eq!(outcome.request.headers[0].value, "Bearer abc");
}

#[tokio::test]
async fn response_json_tests_and_console_are_collected() {
    let mut context = input(ScriptPhase::PostResponse);
    context.response = Some(ScriptResponseContext::new(
        200,
        "OK".to_string(),
        12,
        vec![KeyValue {
            key: "Content-Type".to_string(),
            value: "application/json".to_string(),
            enabled: true,
        }],
        r#"{"data":{"name":"Ada"}}"#,
    ));
    let outcome = run_script(
        r#"
          pm.test("status", () => pm.expect(pm.response.code).to.equal(200));
          pm.test("property", () => pm.expect(pm.response.json().data).to.have.property("name"));
          pm.test("failure", () => pm.expect(1).to.eql(2));
          pm.test("continues", () => pm.expect(pm.response.headers.get("content-type")).to.equal("application/json"));
          console.log("token", pm.environment.get("token"));
          console.log({ client_secret: "must-not-appear" });
          console.log('{"password":"body-secret","safe":true}');
        "#
        .to_string(),
        context,
    )
    .await
    .expect("run script");
    assert_eq!(outcome.execution.tests.len(), 4);
    assert!(!outcome.execution.tests[2].passed);
    assert!(outcome.execution.tests[3].passed);
    assert_eq!(outcome.execution.console[0].message, "<redacted>");
    assert!(!outcome.execution.console[1]
        .message
        .contains("must-not-appear"));
    assert!(!outcome.execution.console[2].message.contains("body-secret"));
}

#[tokio::test]
async fn invalid_json_is_a_script_error() {
    let mut context = input(ScriptPhase::PostResponse);
    context.response = Some(ScriptResponseContext::new(
        200,
        "OK".to_string(),
        1,
        vec![],
        "invalid-json-canary",
    ));
    let outcome = run_script("pm.response.json();".to_string(), context)
        .await
        .expect("run script");
    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Failed);
    let message = outcome.execution.error.unwrap().message;
    assert!(message.contains("not valid JSON"));
    assert!(!message.contains("invalid-json-canary"));
}

#[tokio::test]
async fn oversized_response_body_is_not_injected() {
    let mut context = input(ScriptPhase::PostResponse);
    context.response = Some(ScriptResponseContext::new(
        200,
        "OK".to_string(),
        1,
        vec![],
        &"x".repeat(SCRIPT_MAX_RESPONSE_BODY_BYTES + 1),
    ));
    let outcome = run_script("pm.response.text();".to_string(), context)
        .await
        .expect("run script");

    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Failed);
    assert!(outcome
        .execution
        .error
        .expect("response body limit error")
        .message
        .contains("script limit"));
}

#[tokio::test]
async fn host_capabilities_are_unavailable() {
    let outcome = run_script(
        r#"pm.test("sandbox", () => pm.expect([typeof require, typeof process, typeof fetch, typeof __unfourExport, typeof __UNFOUR_TRACK_VALUE]).to.eql(["undefined", "undefined", "undefined", "undefined", "undefined"]));"#.to_string(),
        input(ScriptPhase::PostResponse),
    )
    .await
    .expect("run script");
    assert!(outcome.execution.tests[0].passed);
}

#[tokio::test]
async fn infinite_loop_is_interrupted() {
    let outcome = run_script(
        "while (true) {}".to_string(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .expect("run script");
    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Timeout);
    assert!(outcome.execution.duration_ms < 3_000);
}

#[tokio::test]
async fn poisoned_result_serialization_is_also_interrupted() {
    let outcome = run_script(
        "Object.prototype.toJSON = function () { while (true) {} };".to_string(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .expect("run script");

    assert_eq!(outcome.execution.status, ScriptExecutionStatus::Timeout);
    assert!(outcome.execution.duration_ms < 3_000);
}

#[tokio::test]
async fn console_limits_output() {
    let outcome = run_script(
        r#"for (let i = 0; i < 150; i++) console.log("x".repeat(10000));"#.to_string(),
        input(ScriptPhase::PreRequest),
    )
    .await
    .expect("run script");
    assert!(outcome.execution.console.len() <= 100);
    assert!(
        outcome
            .execution
            .console
            .iter()
            .map(|entry| entry.message.len())
            .sum::<usize>()
            <= 65_536
    );
}
