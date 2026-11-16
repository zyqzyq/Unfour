//! Rejected-launch evidence. Never format input text, OS errors or paths into a report.
//! Recognition here describes syntax only; it does not authorize any launch mode.
use std::{ffi::OsStr, io::Write, path::Path};

const MAX_ARGS: usize = 64;
const MAX_ARG_BYTES: usize = 32768;
const DIRECTORY: &str = "Unfour-launch-diagnostics";
const HEADER: &str = "Unfour launch diagnostic v1\nBASTION_LAUNCH_UNSUPPORTED\nNo connection attempted. Values are redacted.\nOptions are canonicalized; their presence does not mean support.\nIndices refer to argv after Windows splitting, without the executable.\nOriginal quoting cannot be recovered.\n";

// All emitted names are these constants, never slices of an incoming argument.
// Value-taking options suppress recognition of the next argument as a switch.
const OPTIONS: &[(&str, bool)] = &[
    ("-url", true),
    ("-newtab", true),
    ("-newwin", false),
    ("/SSH2", false),
    ("/SSH1", false),
    ("/TELNET", false),
    ("/T", false),
    ("/L", true),
    ("/PASSWORD", true),
    ("/P", true),
    ("/N", true),
    ("/TITLEBAR", true),
    ("/S", true),
    ("/F", true),
    ("/SCRIPT", true),
    ("/ARG", true),
    ("/I", true),
    ("/ACCEPTHOSTKEYS", false),
    ("/sessionname", true),
    ("/ini", true),
    ("/hostkey", true),
    ("/privatekey", true),
    ("/command", true),
    ("-ssh", false),
    ("-l", true),
    ("-pw", true),
    ("-P", true),
    ("-i", true),
    ("-load", true),
];

fn option(text: &str) -> Option<(&'static str, bool, &'static str)> {
    let (head, separator) = match text.find(['=', ':']) {
        Some(i) => (
            &text[..i],
            if text.as_bytes()[i] == b'=' { "=" } else { ":" },
        ),
        None => (text, ""),
    };
    OPTIONS.iter().find_map(|&(name, takes_value)| {
        // Slash options are case-insensitive. Dash options retain their CLI case.
        let matches = if name.starts_with('/') {
            name.eq_ignore_ascii_case(head)
        } else {
            name == head
        };
        (matches && (separator.is_empty() || takes_value)).then_some((name, takes_value, separator))
    })
}

fn value_shape(text: &str) -> String {
    let mut tags = vec!["redacted"];
    if text.is_empty() {
        tags.push("empty");
    }
    if text.contains(char::is_whitespace) {
        tags.push("whitespace");
    }
    if text.contains(char::is_control) {
        tags.push("control");
    }
    if !text.is_ascii() {
        tags.push("non-ascii");
    }
    if text.contains('"') {
        tags.push("literal-double-quote");
    }
    if text.contains('|') {
        tags.push("pipe");
    }
    if text.contains('%') {
        tags.push("percent");
    }
    if let Some((scheme, rest)) = text.split_once("://") {
        tags.push(match scheme {
            "ssh" => "ssh-uri",
            "sftp" => "sftp-uri",
            "accessclient" => "accessclient-uri",
            "accessmac" => "accessmac-uri",
            _ => "other-uri-shape",
        });
        // Lexical observations only. Do not decode or infer fields from tickets.
        for (delimiter, tag) in [
            ('@', "uri-at"),
            (':', "uri-colon"),
            ('/', "uri-slash"),
            ('?', "uri-query"),
            ('#', "uri-fragment"),
        ] {
            if rest.contains(delimiter) {
                tags.push(tag);
            }
        }
    }
    format!("<{}>", tags.join(", "))
}

fn summarize<S: AsRef<OsStr>>(args: &[S]) -> String {
    let mut report = format!("{HEADER}argument_count={}\n", args.len());
    let mut expecting_value = false;
    for (index, arg) in args.iter().take(MAX_ARGS).enumerate() {
        let shape = match arg.as_ref().to_str() {
            None => {
                expecting_value = false;
                "<redacted, invalid-unicode>".into()
            }
            Some(text) if text.len() > MAX_ARG_BYTES => {
                expecting_value = false;
                "<redacted, oversized>".into()
            }
            Some(text) if expecting_value => {
                expecting_value = false;
                value_shape(text)
            }
            Some(text) => match option(text) {
                Some((name, takes_value, "")) => {
                    expecting_value = takes_value;
                    name.into()
                }
                Some((name, _, separator)) => format!("{name}{separator}<redacted>"),
                None => value_shape(text),
            },
        };
        report.push_str(&format!("argv[{index}] = {shape}\n"));
    }
    if args.len() > MAX_ARGS {
        report.push_str("Remaining arguments omitted (limit 64).\n");
    }
    report
}

// A separate create-new file per launch avoids races between concurrent requests.
// Only the safe report reaches the filesystem; keep it outside all workspace data.
fn save_report(root: &Path, report: &str) -> std::io::Result<String> {
    let directory = root.join(DIRECTORY);
    std::fs::create_dir_all(&directory)?;
    let mut file = tempfile::Builder::new()
        .prefix("launch-")
        .suffix(".txt")
        .tempfile_in(directory)?;
    file.write_all(report.as_bytes())?;
    file.flush()?;
    let (_, path) = file.keep().map_err(|e| e.error)?;
    // The filename is generated by tempfile, not supplied by the caller.
    Ok(path.file_name().unwrap().to_string_lossy().into_owned())
}

#[cfg(windows)]
pub fn report_windows_launch_failure() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND,
    };

    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let report = summarize(&arguments);
    // Raw values never enter a dialog, log, saved report or child process.
    drop(arguments);
    let location = match save_report(&std::env::temp_dir(), &report) {
        Ok(name) => format!("脱敏报告已保存 / Redacted report saved:\n%TEMP%\\{DIRECTORY}\\{name}"),
        Err(_) => {
            "脱敏报告保存失败。可截图此窗口。\nReport could not be saved. Capture this dialog."
                .into()
        }
    };
    // Bound the native dialog height. The file contains up to 64 slots.
    let mut preview = report
        .lines()
        .skip(6)
        .take(13)
        .collect::<Vec<_>>()
        .join("\n");
    if report.lines().skip(6).count() > 13 {
        preview.push_str("\n... See report for remaining arguments.");
    }
    // This runs before Tauri or the shared frontend i18n provider exists.
    // Fixed bilingual bootstrap text keeps the diagnostic usable offline.
    let text = format!(
        "不支持此启动参数，未发起连接。\nUnsupported launch arguments. No connection attempted.\nBASTION_LAUNCH_UNSUPPORTED\n\n{preview}\n\n{location}\n\n在资源管理器地址栏打开 %TEMP%\\{DIRECTORY}\n只提供此脱敏报告，不要提供原始命令行。\nShare only this redacted report, not the original command line."
    );
    let wide_text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Unfour — 启动诊断 / Launch diagnostic"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: Both buffers are NUL-terminated, remain alive through the blocking
    // call and contain only generated text. No owner window exists yet.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide_text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONWARNING | MB_SETFOREGROUND,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn securecrt_shape_keeps_boundaries_and_hides_all_values() {
        let report = summarize(&[
            "/T",
            "/N",
            "SYNTHETIC asset",
            "/SSH2",
            "/L",
            "SYNTHETIC-USER",
            "/PASSWORD",
            "SYNTHETIC-TICKET",
            "/P",
            "22022",
            "gateway.invalid",
        ]);
        assert!(report.contains("argument_count=11\n"));
        assert!(report.contains("argv[2] = <redacted, whitespace>\n"));
        assert!(report.contains("argv[6] = /PASSWORD\nargv[7] = <redacted>\n"));
        for secret in ["SYNTHETIC", "22022", "gateway.invalid"] {
            assert!(!report.contains(secret));
        }
    }

    #[test]
    fn switch_shaped_values_and_attached_values_never_echo() {
        let report = summarize(&[
            "/PASSWORD",
            "/SSH2",
            "/l=secret-user",
            "/password:secret-grant",
            "/script",
            "secret.py",
            "/unknown=secret-extra",
            "unfour://auth/callback?secret",
        ]);
        assert!(report.contains("argv[1] = <redacted>\n"));
        assert!(report.contains("/L=<redacted>"));
        assert!(report.contains("/PASSWORD:<redacted>"));
        for secret in ["secret", "/unknown", "unfour://", "/SSH2"] {
            assert!(!report.contains(secret));
        }
    }

    #[test]
    fn raw_uri_and_control_characters_cannot_inject_report_text() {
        let report = summarize(&[
            "-url",
            "ssh://secret-user:secret%40grant@secret-host:22/path?q#f",
            "-newtab",
            "secret\r\nINJECTED\t\"资产|label",
        ]);
        assert!(report.contains("ssh-uri, uri-at, uri-colon, uri-slash, uri-query, uri-fragment"));
        assert!(report.contains("whitespace, control, non-ascii, literal-double-quote, pipe"));
        assert!(!report.contains("secret"));
        assert!(!report.contains("INJECTED"));
        assert_eq!(report.lines().filter(|s| s.starts_with("argv[")).count(), 4);
    }

    #[test]
    fn oversized_input_and_argument_count_are_bounded() {
        let report = summarize(&vec!["s".repeat(MAX_ARG_BYTES + 1); 100]);
        assert!(report.contains("argument_count=100"));
        assert!(report.contains("<redacted, oversized>"));
        assert_eq!(
            report.lines().filter(|s| s.starts_with("argv[")).count(),
            64
        );
        assert!(report.len() < 5000);
    }

    #[test]
    fn reports_are_unique_redacted_files_and_write_errors_stay_errors() {
        let root = tempfile::tempdir().unwrap();
        let report = summarize(&["/L", "SYNTHETIC-SECRET"]);
        let first = save_report(root.path(), &report).unwrap();
        let second = save_report(root.path(), &report).unwrap();
        assert_ne!(first, second);
        let saved = std::fs::read_to_string(root.path().join(DIRECTORY).join(first)).unwrap();
        assert_eq!(saved, report);
        assert!(!saved.contains("SYNTHETIC-SECRET"));
        let blocker = root.path().join("file-not-directory");
        std::fs::write(&blocker, b"fixture").unwrap();
        assert!(save_report(&blocker, &report).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn invalid_windows_unicode_has_only_a_fixed_marker() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let report = summarize(&[OsString::from_wide(&[0xd800])]);
        assert!(report.contains("argv[0] = <redacted, invalid-unicode>\n"));
    }
}
