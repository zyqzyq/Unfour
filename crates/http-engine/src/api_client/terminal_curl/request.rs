use super::*;
use std::fmt::Write;
use unfour_core::terminal_curl_contract::CURL_SETUP;

pub(super) fn prepare(
    mut input: ApiRequestInput,
    secrets: &[String],
) -> AppResult<PreparedTerminalCurl> {
    validate_workspace_id(&input.workspace_id)?;
    execution_snapshot::validate_replay_values(&input)?;
    let input_bytes = input
        .url
        .len()
        .saturating_add(input.method.len())
        .saturating_add(input.body.as_ref().map_or(0, String::len))
        .saturating_add(
            input
                .headers
                .iter()
                .chain(&input.query)
                .fold(0usize, |total, row| {
                    total
                        .saturating_add(row.key.len())
                        .saturating_add(row.value.len())
                }),
        );
    if input_bytes > MAX_REQUEST_BYTES {
        return Err(failure("SSH_TERMINAL_HTTP_REQUEST_TOO_LARGE"));
    }
    if !input.multipart_parts.is_empty()
        || !matches!(
            input.body_kind.as_str(),
            "none" | "json" | "text" | "raw" | "form-urlencoded"
        )
        || input
            .timeout_ms
            .is_some_and(|ms| ms > unfour_core::models::MAX_API_TIMEOUT_MS)
    {
        return Err(unsupported());
    }
    // Resolve before deriving the executed request. The remote curl and the
    // terminal driver receive the same effective budget, including callers
    // whose desktop preference resolves to zero (unlimited for direct/TCP).
    input.timeout_ms = Some(
        input
            .timeout_ms
            .filter(|ms| *ms > 0)
            .unwrap_or(DEFAULT_TERMINAL_TIMEOUT_MS),
    );
    let method = parse_method(&input.method).map_err(|_| unsupported())?;
    if !matches!(
        method,
        Method::GET
            | Method::HEAD
            | Method::POST
            | Method::PUT
            | Method::PATCH
            | Method::DELETE
            | Method::OPTIONS
    ) {
        return Err(unsupported());
    }
    let mut url = build_url(&input.url, &input.query)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(unsupported());
    }
    url.set_fragment(None);
    let mut runtime_secrets = runtime_request_secret_values(&input)?;
    runtime_secrets.extend(secrets.iter().cloned());
    execution::retain_canonical_secret_values(&input, &url, &mut runtime_secrets);

    // Build without executing so validation and history snapshots retain the
    // ordinary API client's canonical URL/header/body behavior.
    let mut builder = Client::new().request(method.clone(), url.clone());
    let mut content_type = false;
    let mut accept_encoding = false;
    for header in input.headers.iter().filter(|header| header.enabled) {
        let key = header.key.trim();
        let name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|_| failure("API_HEADER_NAME_INVALID"))?;
        let value = HeaderValue::from_str(&header.value)
            .map_err(|_| failure("API_HEADER_VALUE_INVALID"))?;
        // Framing is generated from the actual body. Prevent a conflicting
        // content length/TE or a connection upgrade from changing semantics.
        if matches!(
            name.as_str(),
            "content-length" | "transfer-encoding" | "upgrade" | "trailer"
        ) || (name.as_str() == "connection"
            && header.value.to_ascii_lowercase().contains("upgrade"))
            || (name.as_str() == "accept"
                && header
                    .value
                    .to_ascii_lowercase()
                    .contains("text/event-stream"))
        {
            return Err(unsupported());
        }
        if name.as_str() == "accept-encoding" {
            if !header.value.trim().eq_ignore_ascii_case("identity") {
                return Err(unsupported());
            }
            accept_encoding = true;
        }
        content_type |= name.as_str() == "content-type";
        builder = builder.header(name, value);
    }
    if !accept_encoding {
        builder = builder.header("accept-encoding", "identity");
    }
    let body = input.body.as_deref().filter(|body| !body.is_empty());
    if let Some(body) = body {
        if input.body_kind == "json" && !content_type {
            builder = builder.header(CONTENT_TYPE, "application/json");
        }
        if !matches!(method, Method::GET | Method::HEAD) {
            builder = builder.body(body.to_owned());
        }
    }
    let request = builder.build().map_err(|_| unsupported())?;
    let executed = execution_snapshot::executed_input(&input, &request);
    runtime_secrets.extend(runtime_request_secret_values(&executed)?);
    let snapshot = execution_snapshot::snapshot(&executed, request.url(), &runtime_secrets)?;
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let mut config = String::new();
    let mut body_check = "";
    option(&mut config, "url", request.url().as_str())?;
    if method == Method::HEAD {
        config.push_str("head\n");
    } else {
        option(&mut config, "request", method.as_str())?;
    }
    for (key, value) in request.headers() {
        let value = value.to_str().map_err(|_| unsupported())?;
        // curl's semicolon syntax sends an explicitly empty header. The colon
        // syntax with no value would silently suppress it instead.
        let header = if value.is_empty() {
            format!("{key};")
        } else {
            format!("{key}: {value}")
        };
        option(&mut config, "header", &header)?;
    }
    if let Some(body) = request.body().and_then(reqwest::Body::as_bytes) {
        let body = std::str::from_utf8(body).map_err(|_| unsupported())?;
        // data-raw arrived in 7.43. Use the legacy literal form for JSON and
        // other bodies, but NEVER let data-binary interpret @ as a remote file.
        // Leading-@ text needs data-raw and a strict per-request CLI check.
        let body_option = if body.starts_with('@') {
            body_check = "u_c --data-raw '{}' --version >/dev/null 2>&1 || exit 124; ";
            "data-raw"
        } else {
            "data-binary"
        };
        option(&mut config, body_option, body)?;
        // curl otherwise invents form content-type for all --data-* bodies.
        if !request.headers().contains_key(CONTENT_TYPE) {
            option(&mut config, "header", "Content-Type:")?;
        }
    }
    // Fractional max-time arrived in 7.32. Round up for curl 7.29 while the
    // local terminal driver still enforces the exact millisecond deadline.
    let timeout_seconds = input.timeout_ms.unwrap().div_ceil(1000);
    option(
        &mut config,
        "write-out",
        &format!("\nUNFOUR_CURL_RESULT_{nonce}:%{{http_code}}:%{{size_download}}:%{{time_starttransfer}}:%{{time_total}}\n"),
    )?;
    if config.len() > MAX_REQUEST_BYTES {
        return Err(failure("SSH_TERMINAL_HTTP_REQUEST_TOO_LARGE"));
    }
    // A large heredoc may be implemented using a temporary file by the remote
    // shell. Use a verified shell builtin plus pipe instead. Shell quotes and
    // curl config quotes are separate encodings; request values never become
    // syntax or arguments of an external process. Fail before sending any
    // payload to printf if the shell cannot establish it is a builtin.
    let quoted = config.replace('\'', "'\\''");
    let command = format!(
        "{CURL_SETUP}{body_check}command printf '%s' '{quoted}' | u_c $h --max-time {timeout_seconds} --config - 2>/dev/null\n"
    );
    Ok(PreparedTerminalCurl {
        command,
        nonce,
        executed,
        secrets: runtime_secrets,
        snapshot,
    })
}

fn option(config: &mut String, key: &str, value: &str) -> AppResult<()> {
    write!(config, "{key} = \"").unwrap();
    for ch in value.chars() {
        match ch {
            '\\' => config.push_str("\\\\"),
            '"' => config.push_str("\\\""),
            '\n' => config.push_str("\\n"),
            '\r' => config.push_str("\\r"),
            '\t' => config.push_str("\\t"),
            ch if ch.is_control() => return Err(unsupported()),
            ch => config.push(ch),
        }
    }
    config.push_str("\"\n");
    Ok(())
}
