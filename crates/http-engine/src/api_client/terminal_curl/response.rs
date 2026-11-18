use super::*;

pub(super) struct Received {
    pub status: u16,
    pub headers: Vec<KeyValue>,
    pub body: String,
    pub curl_start_transfer_ms: Option<u64>,
    pub curl_total_ms: Option<u64>,
}

pub(super) fn parse(
    prepared: &PreparedTerminalCurl,
    stdout: &[u8],
    exit_code: i32,
) -> AppResult<Received> {
    if exit_code != 0 {
        return Err(failure(curl_failure_code(exit_code)));
    }
    if stdout.len() > MAX_OUTPUT_BYTES {
        return Err(failure("SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"));
    }
    let prefix = format!("\nUNFOUR_CURL_RESULT_{}:", prepared.nonce);
    let at = stdout
        .windows(prefix.len())
        .rposition(|part| part == prefix.as_bytes())
        .ok_or_else(invalid_response)?;
    let metadata =
        std::str::from_utf8(&stdout[at + prefix.len()..]).map_err(|_| invalid_response())?;
    let metadata = metadata.strip_suffix('\n').ok_or_else(invalid_response)?;
    let mut fields = metadata.split(':');
    let (Some(status), Some(size), Some(start_transfer), Some(total), None) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(invalid_response());
    };
    if status.len() != 3 || !status.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid_response());
    }
    let status: u16 = status.parse().map_err(|_| invalid_response())?;
    let size: usize = size.parse().map_err(|_| invalid_response())?;
    let (parsed_status, headers, body) = parse_headers(&stdout[..at])?;
    if status != parsed_status || size != body.len() {
        return Err(invalid_response());
    }
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(failure("SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"));
    }
    for header in headers.iter().filter(|_| !body.is_empty()) {
        let value = header.value.to_ascii_lowercase();
        if (header.key == "content-encoding" && value.trim() != "identity")
            || (header.key == "content-type"
                && (value.starts_with("text/event-stream")
                    || (!value.starts_with("text/")
                        && !value.contains("json")
                        && !value.contains("xml")
                        && !value.starts_with("application/x-www-form-urlencoded"))))
        {
            return Err(failure("SSH_TERMINAL_HTTP_UNSUPPORTED_RESPONSE"));
        }
    }
    let body =
        std::str::from_utf8(body).map_err(|_| failure("SSH_TERMINAL_HTTP_UNSUPPORTED_RESPONSE"))?;
    if body
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(failure("SSH_TERMINAL_HTTP_UNSUPPORTED_RESPONSE"));
    }
    Ok(Received {
        status,
        headers,
        curl_start_transfer_ms: curl_millis(start_transfer),
        curl_total_ms: curl_millis(total),
        body: body.trim_start_matches('\u{feff}').to_owned(),
    })
}

// curl writes seconds with a C-locale decimal point. Optional telemetry
// must never discard an otherwise complete, byte-checked business response.
fn curl_millis(value: &str) -> Option<u64> {
    let (seconds, fraction) = value.split_once('.')?;
    if seconds.is_empty()
        || fraction.is_empty()
        || fraction.len() > 6
        || !seconds
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let millis = fraction
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(3)
        .fold(0, |n, digit| n * 10 + u64::from(digit - b'0'));
    seconds
        .parse::<u64>()
        .ok()?
        .checked_mul(1000)?
        .checked_add(millis)
}

fn parse_headers(mut bytes: &[u8]) -> AppResult<(u16, Vec<KeyValue>, &[u8])> {
    let mut consumed = 0;
    // Handles 100 Continue / 103 Early Hints without treating them as body.
    for _ in 0..16 {
        let end = bytes
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .ok_or_else(invalid_response)?;
        consumed += end + 4;
        if consumed > MAX_HEADER_BYTES {
            return Err(failure("SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"));
        }
        let text = std::str::from_utf8(&bytes[..end]).map_err(|_| invalid_response())?;
        let mut lines = text.split("\r\n");
        let first = lines.next().ok_or_else(invalid_response)?;
        let mut words = first.splitn(3, ' ');
        if !matches!(words.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
            return Err(invalid_response());
        }
        let code = words.next().ok_or_else(invalid_response)?;
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid_response());
        }
        let status: u16 = code.parse().map_err(|_| invalid_response())?;
        if !(100..600).contains(&status) || status == 101 {
            return Err(invalid_response());
        }
        let mut headers = Vec::new();
        for line in lines {
            let (key, value) = line.split_once(':').ok_or_else(invalid_response)?;
            let key = HeaderName::from_bytes(key.as_bytes()).map_err(|_| invalid_response())?;
            let value = value.trim_matches([' ', '\t']);
            HeaderValue::from_str(value).map_err(|_| invalid_response())?;
            headers.push(KeyValue {
                key: key.to_string(),
                value: value.into(),
                enabled: true,
            });
        }
        bytes = &bytes[end + 4..];
        if status >= 200 {
            return Ok((status, headers, bytes));
        }
    }
    Err(invalid_response())
}

fn curl_failure_code(code: i32) -> &'static str {
    match code {
        127 => "SSH_TERMINAL_HTTP_CURL_UNAVAILABLE",
        124 => "SSH_TERMINAL_HTTP_CURL_BODY_UNSUPPORTED",
        2 | 4 | 125 | 126 => "SSH_TERMINAL_HTTP_CURL_UNSUPPORTED",
        5 | 6 => "SSH_TERMINAL_HTTP_DNS_FAILED",
        7 => "SSH_TERMINAL_HTTP_CONNECT_FAILED",
        28 => "SSH_TERMINAL_HTTP_TIMEOUT",
        35 | 51 | 58 | 60 | 77 | 83 | 90 | 91 => "SSH_TERMINAL_HTTP_TLS_FAILED",
        63 => "SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE",
        _ => "SSH_TERMINAL_HTTP_TRANSFER_FAILED",
    }
}
