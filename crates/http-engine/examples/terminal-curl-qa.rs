//! Isolated synthetic QA bridge: emit the production curl command, then parse
//! stdout returned by a separate real-shell/PTY harness with the same nonce.
//! Never executes a subprocess, sends HTTP, or opens a real workspace DB.
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use unfour_core::models::ApiRequestInput;
use unfour_http_engine::{prepare_terminal_curl, ApiClientService};
use unfour_local_storage::LocalDb;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=4).contains(&args.len()) {
        return Err(
            "usage: terminal-curl-qa <http://127.0.0.1:port/path> <new-qa-directory> [json|small-json|text-at|text-crlf|empty] [emit-only]".into(),
        );
    }
    let url = reqwest::Url::parse(&args[0])?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("only explicit IPv4 loopback HTTP fixtures are allowed".into());
    }
    let qa = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&qa)?;
    for filename in ["command.sh", "response.bin", "exit-code.txt", "result.json"] {
        if qa.join(filename).exists() {
            return Err("QA artifacts must not preexist".into());
        }
    }
    let scenario = args.get(2).map(String::as_str).unwrap_or("json");
    let (body_kind, body) = match scenario {
        "json" => ("json", serde_json::json!({
        "model":"synthetic-model", "input":["北京是中国的首都。", "single ' quote $(false) `false` \n next line"],
        "padding":"x".repeat(70000)
        }).to_string()),
        "small-json" => ("json", serde_json::json!({"input":"x".repeat(118)}).to_string()),
        "text-at" => ("text", "@/UNFOUR_CURL_QA_MUST_NOT_READ\r\n中文 ' quote $(false) `false`".into()),
        "text-crlf" => ("text", "line1\r\n中文 ' quote $(false) `false`\nline3\tend".into()),
        "empty" | "get" | "long-header" => ("text", String::new()),
        _ => return Err("unknown synthetic scenario".into()),
    };
    let emit_only = match args.get(3).map(String::as_str) {
        None => false,
        Some("emit-only") => true,
        _ => return Err("unknown QA mode".into()),
    };
    std::fs::write(qa.join("request-body.bin"), body.as_bytes())?;
    let input: ApiRequestInput = serde_json::from_value(serde_json::json!({
        "workspaceId":"terminal-curl-qa", "method":if matches!(scenario, "get" | "long-header") { "GET" } else { "POST" }, "url":url.as_str(),
        "headers":[{"key":"Authorization","value":format!("Bearer UNFOUR_CURL_QA_SECRET{}", if scenario == "long-header" { "x".repeat(1200) } else { String::new() }),"enabled":true}],
        "query":[{"key":"q","value":"中文 ' $() &","enabled":true}],
        "bodyKind":body_kind, "body":body, "timeoutMs":20000
    }))?;
    let prepared = prepare_terminal_curl(input, &[])?;
    // Publish only after the complete command has been written; the external
    // PTY harness polls for command.sh and must never observe a partial file.
    let pending_command = qa.join("command.sh.pending");
    std::fs::write(qa.join("config.txt"), &prepared.request.config)?;
    std::fs::write(
        qa.join("nonce.txt"),
        prepared.request.id.simple().to_string(),
    )?;
    std::fs::write(&pending_command, prepared.request.shell_fixture())?;
    std::fs::rename(pending_command, qa.join("command.sh"))?;
    if emit_only {
        return Ok(());
    }
    println!("READY: synthetic production command written; waiting for raw stdout and exit code");
    let started = Instant::now();
    while !qa.join("exit-code.txt").exists() {
        if started.elapsed() > Duration::from_secs(180) {
            return Err("QA harness did not complete within 180 seconds".into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let bytes = std::fs::read(qa.join("response.bin"))?;
    let exit_code = std::fs::read_to_string(qa.join("exit-code.txt"))?
        .trim()
        .parse()?;
    let db = LocalDb::connect_ephemeral().await?;
    db.migrate().await?;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO workspaces(id,name,is_default,last_opened_at,created_at,updated_at,revision,sync_status) VALUES('terminal-curl-qa','synthetic',0,?1,?1,?1,1,'local')")
        .bind(now).execute(db.pool()).await?;
    let service = ApiClientService::new(db.clone());
    let response = service
        .complete_terminal_curl(
            prepared,
            bytes,
            exit_code,
            started.elapsed(),
            true,
            Default::default(),
        )
        .await?;
    let history: (String, String, String) = sqlx::query_as("SELECT request_headers_json,response_headers_json,response_body_preview FROM api_history WHERE id=?")
        .bind(&response.history_id).fetch_one(db.pool()).await?;
    let history_contains_secret = [history.0, history.1, history.2]
        .iter()
        .any(|value| value.contains("UNFOUR_CURL_QA_SECRET"));
    if history_contains_secret {
        return Err("synthetic secret reached history".into());
    }
    let result = serde_json::json!({
        "result":"PASS", "status":response.status, "bodyBytes":response.body.len(),
        "bodyIsJson":serde_json::from_str::<serde_json::Value>(&response.body).is_ok(),
        "curlStartTransferMs":response.terminal_timing.as_ref().and_then(|t| t.curl_start_transfer_ms),
        "curlTotalMs":response.terminal_timing.as_ref().and_then(|t| t.curl_total_ms), "headerCount":response.headers.len(), "warnings":response.warnings,
        "historyContainsSyntheticSecret":history_contains_secret
    });
    std::fs::write(qa.join("result.json"), serde_json::to_vec_pretty(&result)?)?;
    println!("{}", result);
    Ok(())
}
