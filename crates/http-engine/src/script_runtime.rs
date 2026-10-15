use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

use rquickjs::{CatchResultExt, Context, Function, Runtime};
use serde::{Deserialize, Serialize};
use unfour_core::models::{
    KeyValue, ScriptConsoleEntry, ScriptError, ScriptErrorKind, ScriptExecutionResult,
    ScriptExecutionStatus, ScriptTestResult,
};
use unfour_core::{AppError, AppResult};

mod diagnostics;
use diagnostics::DiagnosticRedactions;

pub const SCRIPT_SCHEMA_VERSION: i64 = 1;
pub const SCRIPT_TIMEOUT: Duration = Duration::from_millis(1_000);
pub const SCRIPT_MAX_LENGTH_BYTES: usize = 64 * 1024;
pub const SCRIPT_MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
pub const SCRIPT_MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
pub const SCRIPT_MAX_OUTPUT_BYTES: usize = 512 * 1024;
pub const SCRIPT_MEMORY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
pub const SCRIPT_STACK_LIMIT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptPhase {
    PreRequest,
    PostResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptVariable {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRequestContext {
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub body_raw: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResponseContext {
    pub code: u16,
    pub status: String,
    pub response_time: u128,
    pub headers: Vec<KeyValue>,
    pub body: Option<String>,
    pub body_error: Option<String>,
}

impl ScriptResponseContext {
    pub fn new(
        code: u16,
        status: String,
        response_time: u128,
        headers: Vec<KeyValue>,
        body: &str,
    ) -> Self {
        if body.len() <= SCRIPT_MAX_RESPONSE_BODY_BYTES {
            Self {
                code,
                status,
                response_time,
                headers,
                body: Some(body.to_string()),
                body_error: None,
            }
        } else {
            Self {
                code,
                status,
                response_time,
                headers,
                body: None,
                body_error: Some(format!(
                    "Response body exceeds the {} byte script limit",
                    SCRIPT_MAX_RESPONSE_BODY_BYTES
                )),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRunInput {
    pub phase: ScriptPhase,
    pub request: ScriptRequestContext,
    pub response: Option<ScriptResponseContext>,
    pub environment: Vec<ScriptVariable>,
    pub variables: Vec<ScriptVariable>,
    pub has_environment: bool,
    pub redactions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScriptRunOutcome {
    pub execution: ScriptExecutionResult,
    pub request: ScriptRequestContext,
    pub environment: Vec<ScriptVariable>,
    pub variables: Vec<ScriptVariable>,
    pub environment_changed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawScriptOutput {
    request: ScriptRequestContext,
    environment: Vec<ScriptVariable>,
    variables: Vec<ScriptVariable>,
    environment_changed: bool,
    console: Vec<ScriptConsoleEntry>,
    tests: Vec<ScriptTestResult>,
}

pub fn validate_script_config(
    pre_request_script: Option<&str>,
    post_response_script: Option<&str>,
    schema_version: i64,
) -> AppResult<()> {
    if schema_version != SCRIPT_SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "unsupported API request script schema version: {schema_version}"
        )));
    }
    for (label, script) in [
        ("pre-request", pre_request_script),
        ("post-response", post_response_script),
    ] {
        if script.is_some_and(|value| value.len() > SCRIPT_MAX_LENGTH_BYTES) {
            return Err(AppError::Validation(format!(
                "{label} script exceeds the {SCRIPT_MAX_LENGTH_BYTES} byte limit"
            )));
        }
    }
    Ok(())
}

pub async fn run_script(script: String, input: ScriptRunInput) -> AppResult<ScriptRunOutcome> {
    let fallback_request = input.request.clone();
    let fallback_environment = input.environment.clone();
    let fallback_variables = input.variables.clone();
    if script.trim().is_empty() {
        return Ok(ScriptRunOutcome {
            execution: ScriptExecutionResult::skipped(),
            request: fallback_request,
            environment: fallback_environment,
            variables: fallback_variables,
            environment_changed: false,
        });
    }
    if script.len() > SCRIPT_MAX_LENGTH_BYTES {
        return Ok(failed_outcome(
            fallback_request,
            fallback_environment,
            fallback_variables,
            ScriptErrorKind::Limit,
            "SCRIPT_LENGTH_LIMIT",
            format!("Script exceeds the {SCRIPT_MAX_LENGTH_BYTES} byte limit"),
            0,
        ));
    }

    let input_json = serde_json::to_string(&input)?;
    if input_json.len() > SCRIPT_MAX_INPUT_BYTES {
        return Ok(failed_outcome(
            fallback_request,
            fallback_environment,
            fallback_variables,
            ScriptErrorKind::Limit,
            "SCRIPT_INPUT_LIMIT",
            format!("Script context exceeds the {SCRIPT_MAX_INPUT_BYTES} byte limit"),
            0,
        ));
    }

    tokio::task::spawn_blocking(move || execute_sync(script, input_json, input))
        .await
        .map_err(|_| AppError::Config("script worker failed".to_string()))?
}

fn execute_sync(
    script: String,
    input_json: String,
    input: ScriptRunInput,
) -> AppResult<ScriptRunOutcome> {
    let started = Instant::now();
    let redactions = Rc::new(RefCell::new(DiagnosticRedactions::new(&input)));
    let timed_out = Arc::new(AtomicBool::new(false));
    let interrupt_enabled = Arc::new(AtomicBool::new(true));
    let deadline = started + SCRIPT_TIMEOUT;
    let timeout_flag = timed_out.clone();
    let enabled_flag = interrupt_enabled.clone();

    let runtime = Runtime::new()
        .map_err(|error| AppError::Config(format!("cannot create script runtime: {error}")))?;
    runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT_BYTES);
    runtime.set_max_stack_size(SCRIPT_STACK_LIMIT_BYTES);
    runtime.set_interrupt_handler(Some(Box::new(move || {
        if enabled_flag.load(Ordering::Relaxed) && Instant::now() >= deadline {
            timeout_flag.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    })));

    let context = Context::full(&runtime)
        .map_err(|error| AppError::Config(format!("cannot create script context: {error}")))?;
    let exporter_name = format!(
        "__unfourHostExport_{}",
        unfour_core::id::new_id().replace('-', "_")
    );
    let mut script_error = None;
    let mut raw_output = None;
    context.with(|ctx| -> AppResult<()> {
        let tracked = redactions.clone();
        let track_value = Function::new(ctx.clone(), move |value: String| {
            tracked.borrow_mut().record(&value);
        })
        .map_err(|_| AppError::Config("cannot initialize script diagnostics".to_string()))?;
        ctx.globals()
            .set("__UNFOUR_TRACK_VALUE", track_value)
            .map_err(|_| AppError::Config("cannot inject script diagnostics".to_string()))?;
        ctx.globals()
            .set("__UNFOUR_INPUT_JSON", input_json)
            .map_err(|error| AppError::Config(format!("cannot inject script context: {error}")))?;
        ctx.eval::<(), _>(BOOTSTRAP).catch(&ctx).map_err(|error| {
            AppError::Config(format!("cannot initialize script sandbox: {error}"))
        })?;
        let capture_exporter =
            format!("const {exporter_name} = __unfourExport; delete globalThis.__unfourExport;");
        ctx.eval::<(), _>(capture_exporter.as_bytes())
            .catch(&ctx)
            .map_err(|error| {
                AppError::Config(format!("cannot isolate script result collector: {error}"))
            })?;

        if let Err(error) = ctx.eval::<(), _>(script.as_bytes()).catch(&ctx) {
            script_error = Some(error.to_string());
        }
        let collect_output = format!("{exporter_name}()");
        match ctx.eval::<String, _>(collect_output.as_bytes()).catch(&ctx) {
            Ok(output) => raw_output = Some(output),
            Err(error) => script_error = Some(error.to_string()),
        }
        interrupt_enabled.store(false, Ordering::Relaxed);
        Ok(())
    })?;

    let duration_ms = started.elapsed().as_millis();
    let Some(output) = raw_output else {
        let is_timeout = timed_out.load(Ordering::Relaxed);
        return Ok(failed_outcome(
            input.request,
            input.environment,
            input.variables,
            if is_timeout {
                ScriptErrorKind::Timeout
            } else {
                ScriptErrorKind::Runtime
            },
            if is_timeout {
                "SCRIPT_TIMEOUT"
            } else {
                "SCRIPT_OUTPUT_ERROR"
            },
            if is_timeout {
                format!(
                    "Script exceeded the {}ms execution limit",
                    SCRIPT_TIMEOUT.as_millis()
                )
            } else {
                // Export may fail after a mutation or invoke script-controlled
                // getters/toJSON. Its exception is not a safe diagnostic.
                "Cannot safely collect script result".to_string()
            },
            duration_ms,
        ));
    };
    if output.len() > SCRIPT_MAX_OUTPUT_BYTES {
        return Ok(failed_outcome(
            input.request,
            input.environment,
            input.variables,
            ScriptErrorKind::Limit,
            "SCRIPT_OUTPUT_LIMIT",
            format!("Script result exceeds the {SCRIPT_MAX_OUTPUT_BYTES} byte limit"),
            duration_ms,
        ));
    }
    let mut parsed: RawScriptOutput = match serde_json::from_str(&output) {
        Ok(parsed) => parsed,
        Err(_) => {
            // Serde errors can quote attacker-controlled field/enum values.
            return Ok(failed_outcome(
                input.request,
                input.environment,
                input.variables,
                ScriptErrorKind::Validation,
                "SCRIPT_OUTPUT_INVALID",
                "Cannot safely decode script result".to_string(),
                duration_ms,
            ));
        }
    };
    let mut redactions = redactions.borrow_mut();
    redactions.collect_output(&input, &parsed);
    // This boundary also precedes output validation and every early return
    // carrying collected console/tests. Business values stay untouched.
    redactions.sanitize_diagnostics(&mut parsed.console, &mut parsed.tests);
    if let Err(message) = validate_output(&parsed) {
        return Ok(ScriptRunOutcome {
            execution: ScriptExecutionResult {
                status: ScriptExecutionStatus::Failed,
                duration_ms,
                console: parsed.console,
                tests: parsed.tests,
                error: Some(ScriptError {
                    kind: ScriptErrorKind::Validation,
                    code: "SCRIPT_OUTPUT_INVALID".to_string(),
                    message,
                }),
            },
            request: input.request,
            environment: input.environment,
            variables: input.variables,
            environment_changed: false,
        });
    }

    let error = if timed_out.load(Ordering::Relaxed) {
        Some(ScriptError {
            kind: ScriptErrorKind::Timeout,
            code: "SCRIPT_TIMEOUT".to_string(),
            message: format!(
                "Script exceeded the {}ms execution limit",
                SCRIPT_TIMEOUT.as_millis()
            ),
        })
    } else {
        script_error.map(|message| ScriptError {
            kind: ScriptErrorKind::Runtime,
            code: "SCRIPT_RUNTIME_ERROR".to_string(),
            message: redactions.scrub(&message),
        })
    };
    let status = match error.as_ref().map(|error| error.kind) {
        Some(ScriptErrorKind::Timeout) => ScriptExecutionStatus::Timeout,
        Some(_) => ScriptExecutionStatus::Failed,
        None => ScriptExecutionStatus::Success,
    };

    Ok(ScriptRunOutcome {
        execution: ScriptExecutionResult {
            status,
            duration_ms,
            console: parsed.console,
            tests: parsed.tests,
            error,
        },
        request: parsed.request,
        environment: parsed.environment,
        variables: parsed.variables,
        environment_changed: parsed.environment_changed,
    })
}

fn validate_output(output: &RawScriptOutput) -> Result<(), String> {
    if output.request.method.trim().is_empty() {
        return Err("pre-request script produced an empty HTTP method".to_string());
    }
    if output.request.url.trim().is_empty() {
        return Err("pre-request script produced an empty URL".to_string());
    }
    if output
        .request
        .headers
        .iter()
        .any(|header| header.key.trim().is_empty())
    {
        return Err("pre-request script produced an empty header name".to_string());
    }
    if output
        .environment
        .iter()
        .chain(output.variables.iter())
        .any(|variable| variable.key.trim().is_empty())
    {
        return Err("script produced an empty variable name".to_string());
    }
    Ok(())
}

fn failed_outcome(
    request: ScriptRequestContext,
    environment: Vec<ScriptVariable>,
    variables: Vec<ScriptVariable>,
    kind: ScriptErrorKind,
    code: &str,
    message: String,
    duration_ms: u128,
) -> ScriptRunOutcome {
    let status = if kind == ScriptErrorKind::Timeout {
        ScriptExecutionStatus::Timeout
    } else {
        ScriptExecutionStatus::Failed
    };
    ScriptRunOutcome {
        execution: ScriptExecutionResult {
            status,
            duration_ms,
            console: Vec::new(),
            tests: Vec::new(),
            error: Some(ScriptError {
                kind,
                code: code.to_string(),
                message,
            }),
        },
        request,
        environment,
        variables,
        environment_changed: false,
    }
}

const BOOTSTRAP: &str = r#"
"use strict";
(() => {
  const input = JSON.parse(globalThis.__UNFOUR_INPUT_JSON);
  const stringify = JSON.stringify.bind(JSON);
  const trackValue = globalThis.__UNFOUR_TRACK_VALUE;
  delete globalThis.__UNFOUR_TRACK_VALUE;
  delete globalThis.__UNFOUR_INPUT_JSON;
  const limits = Object.freeze({ logCount: 100, logEntry: 8192, logTotal: 65536, tests: 100 });
  const sensitiveKey = /(authorization|cookie|proxy-authorization|x-api-key|x-auth-token|password|passwd|token|secret|private[_-]?key|api[_-]?key|credential|license[_-]?key)/i;
  let logSize = 0;
  const logs = [];
  const tests = [];
  let environmentChanged = false;

  const environment = new Map(input.environment.map((item) => [item.key, { ...item }]));
  const variables = new Map(input.variables.map((item) => [item.key, { ...item }]));
  const request = {
    method: input.request.method,
    url: input.request.url,
    headers: input.request.headers.map((item) => ({ ...item })),
    body: { raw: input.request.bodyRaw }
  };

  function variableName(name) {
    const key = String(name).trim();
    if (!key) throw new Error("Variable name cannot be empty");
    return key;
  }
  function getEnabled(map, name) {
    const item = map.get(String(name));
    return item && item.enabled ? item.value : undefined;
  }
  function setVariable(map, name, value) {
    const key = variableName(name);
    const text = String(value);
    trackValue(text);
    map.set(key, { key, value: text, enabled: true });
  }
  function redactString(value) {
    // Keep complete values until the host can replace overlapping old/new
    // credentials longest-first with the shared history scrubber.
    return String(value);
  }
  function sanitize(value, seen = new WeakSet(), depth = 0) {
    if (typeof value === "string") return redactString(value);
    if (value === null || typeof value !== "object") return value;
    if (depth >= 5) return "[Max depth]";
    if (seen.has(value)) return "[Circular]";
    seen.add(value);
    if (Array.isArray(value)) return value.map((item) => sanitize(item, seen, depth + 1));
    const output = {};
    for (const key of Object.keys(value)) {
      output[key] = sensitiveKey.test(key) ? "[REDACTED]" : sanitize(value[key], seen, depth + 1);
    }
    return output;
  }
  function display(value) {
    if (typeof value === "string") {
      const redacted = redactString(value);
      const trimmed = redacted.trim();
      if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
        try { return JSON.stringify(sanitize(JSON.parse(redacted))); } catch (_) {}
      }
      return redacted;
    }
    if (typeof value === "undefined") return "undefined";
    try { return JSON.stringify(sanitize(value)); } catch (_) { return redactString(String(value)); }
  }
  function pushLog(level, args) {
    if (logs.length >= limits.logCount || logSize >= limits.logTotal) return;
    let message = args.map(display).join(" ");
    let truncated = false;
    const suffix = " [truncated]";
    if (message.length > limits.logEntry) {
      message = message.slice(0, limits.logEntry - suffix.length) + suffix;
      truncated = true;
    }
    const remaining = limits.logTotal - logSize;
    if (message.length > remaining) {
      truncated = true;
      message = remaining > suffix.length
        ? message.slice(0, remaining - suffix.length) + suffix
        : suffix.slice(0, remaining);
    }
    logSize += message.length;
    // A clipped credential no longer matches its complete value in Rust.
    logs.push({ level, message: truncated ? "<redacted>" : message, sequence: logs.length });
  }
  function findHeader(headers, name) {
    const key = String(name).toLowerCase();
    return headers.find((item) => item.enabled && String(item.key).toLowerCase() === key);
  }
  function deepEqual(left, right) {
    if (Object.is(left, right)) return true;
    if (!left || !right || typeof left !== "object" || typeof right !== "object") return false;
    if (Array.isArray(left) !== Array.isArray(right)) return false;
    const leftKeys = Object.keys(left).sort();
    const rightKeys = Object.keys(right).sort();
    if (leftKeys.length !== rightKeys.length || leftKeys.some((key, index) => key !== rightKeys[index])) return false;
    return leftKeys.every((key) => deepEqual(left[key], right[key]));
  }
  function assertion(condition, message) { if (!condition) throw new Error(message); }
  function expect(actual) {
    const to = {
      equal(expected) { assertion(Object.is(actual, expected), `expected ${display(actual)} to equal ${display(expected)}`); },
      eql(expected) { assertion(deepEqual(actual, expected), `expected ${display(actual)} to deeply equal ${display(expected)}`); }
    };
    to.be = {};
    Object.defineProperty(to.be, "ok", {
      get() { assertion(Boolean(actual), `expected ${display(actual)} to be truthy`); return true; }
    });
    to.have = {
      property(name) {
        assertion(actual !== null && (typeof actual === "object" || typeof actual === "function") && Object.prototype.hasOwnProperty.call(actual, name), `expected value to have property ${String(name)}`);
        return actual[name];
      }
    };
    return { to };
  }

  const headerApi = Object.freeze({
    get(name) { return findHeader(request.headers, name)?.value; },
    upsert(item) {
      if (!item || typeof item !== "object") throw new Error("Header upsert requires { key, value }");
      const key = String(item.key ?? "").trim();
      if (!key) throw new Error("Header key cannot be empty");
      const current = request.headers.find((entry) => String(entry.key).toLowerCase() === key.toLowerCase());
      const value = String(item.value ?? "");
      trackValue(value);
      if (current) Object.assign(current, { key, value, enabled: true });
      else request.headers.push({ key, value, enabled: true });
    },
    remove(name) {
      const key = String(name).toLowerCase();
      for (let index = request.headers.length - 1; index >= 0; index--) {
        if (String(request.headers[index].key).toLowerCase() === key) request.headers.splice(index, 1);
      }
    }
  });
  Object.defineProperty(request, "headers", { value: request.headers, writable: false, enumerable: true });
  request.headers.get = headerApi.get;
  request.headers.upsert = headerApi.upsert;
  request.headers.remove = headerApi.remove;

  const response = input.response ? Object.freeze({
    code: input.response.code,
    status: input.response.status,
    responseTime: input.response.responseTime,
    headers: Object.freeze({ get(name) { return findHeader(input.response.headers, name)?.value; } }),
    text() { if (input.response.bodyError) throw new Error(input.response.bodyError); return input.response.body ?? ""; },
    json() {
      if (input.response.bodyError) throw new Error(input.response.bodyError);
      try { return JSON.parse(input.response.body ?? ""); }
      // Parser errors can echo an unclassified response-body fragment.
      catch (_) { throw new Error("Response body is not valid JSON"); }
    }
  }) : undefined;

  const pm = Object.freeze({
    environment: Object.freeze({
      get(name) { return getEnabled(environment, name); },
      set(name, value) {
        if (!input.hasEnvironment) throw new Error("No active workspace environment is selected");
        setVariable(environment, name, value); environmentChanged = true;
      },
      unset(name) {
        if (!input.hasEnvironment) throw new Error("No active workspace environment is selected");
        environment.delete(variableName(name)); environmentChanged = true;
      }
    }),
    variables: Object.freeze({
      get(name) { const local = getEnabled(variables, name); return local === undefined ? getEnabled(environment, name) : local; },
      set(name, value) { setVariable(variables, name, value); },
      unset(name) { variables.delete(variableName(name)); }
    }),
    request,
    response,
    test(name, callback) {
      if (tests.length >= limits.tests) throw new Error("Script test count limit exceeded");
      const started = Date.now();
      try {
        const returned = callback();
        if (returned && typeof returned.then === "function") throw new Error("Async tests are not supported");
        tests.push({ name: String(name), passed: true, errorMessage: null, durationMs: Date.now() - started });
      } catch (error) {
        tests.push({ name: String(name), passed: false, errorMessage: display(error?.message ?? String(error)), durationMs: Date.now() - started });
      }
    },
    expect
  });
  Object.defineProperty(globalThis, "pm", { value: pm, writable: false, configurable: false });
  Object.defineProperty(globalThis, "console", { value: Object.freeze({
    log(...args) { pushLog("log", args); },
    warn(...args) { pushLog("warn", args); },
    error(...args) { pushLog("error", args); }
  }), writable: false, configurable: false });

  Object.defineProperty(globalThis, "__unfourExport", { value: () => stringify({
    request: {
      method: String(request.method),
      url: String(request.url),
      headers: request.headers.map((item) => ({ key: String(item.key), value: String(item.value), enabled: Boolean(item.enabled) })),
      bodyRaw: request.body.raw == null ? null : String(request.body.raw)
    },
    environment: Array.from(environment.values()),
    variables: Array.from(variables.values()),
    environmentChanged,
    console: logs,
    tests
  }), writable: false, configurable: true });

  for (const name of ["require", "process", "fetch", "WebSocket", "window", "document", "navigator", "setTimeout", "setInterval", "queueMicrotask", "Promise", "eval", "Function"]) {
    Object.defineProperty(globalThis, name, { value: undefined, writable: false, configurable: false });
  }
  try { Object.defineProperty((() => {}).constructor.prototype, "constructor", { value: undefined, writable: false, configurable: false }); } catch (_) {}
})();
"#;

#[cfg(test)]
#[path = "script_runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "script_runtime/diagnostics_tests.rs"]
mod diagnostics_tests;
