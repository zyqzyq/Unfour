//! Shell output is encoded before it reaches the PTY. The exit marker is
//! encoded in the same pipe and the outer END is emitted only after pipe EOF.
pub(super) use crate::ssh::terminal_wire::{Fragment, InputEncoding, Parser};
use base64::{engine::general_purpose::STANDARD, Engine};

pub(super) const STARTUP: &str = "UNFOUR_RAW_V3";
pub(super) const READY_CAPABILITIES: &str = "READY:3:240:b64";
pub(super) const OCTAL_CAPABILITIES: &str = "READY:3:240:octal";
pub(super) const BOOTSTRAP: &str = concat!(
    " sh -c '",
    "set +avx; unset l part p u_cmd; tty=$(stty -g) || exit 125; sid=; ",
    "restore() { stty \"$tty\" && { [ -z \"$sid\" ] || command printf \"\\036%s:RESTORED\\037\" \"$sid\"; }; }; ",
    "trap restore 0; trap \"exit 130\" 2 15; stty icanon isig intr ^C icrnl -inlcr -igncr -echo -noflsh || exit 125; ",
    "rd() { while IFS= read -r l; do [ -n \"$l\" ] && return 0; done; return 1; }; ",
    "printf \"\\036UNFOUR_RAW_V3\\037\"; rd || exit 125; sid=$l; ",
    // Only fixed protocol text and the non-secret correlation ID may use an
    // external printf. Never accept secret payload until builtin validation.
    "case $(command -v printf):$(LC_ALL=C command -V printf 2>/dev/null) in printf:*builtin*) ;; *) command printf \"\\036%s:PRINTF_UNSUPPORTED\\037\" \"$sid\"; exit 125;; esac; ",
    "command -v base64 >/dev/null 2>&1 || { command printf \"\\036%s:BASE64_MISSING\\037\" \"$sid\"; exit 125; }; ",
    "fmt=octal; x=$(command printf eA== | command base64 -d 2>/dev/null) && [ \"$x\" = x ] && fmt=b64; ",
    "printf \"\\036%s:READY:3:240:%s\\037\" \"$sid\" \"$fmt\"; ",
    "rd || exit 125; op=$l; printf \"\\036%s:ACCEPT\\037\" \"$op\"; p=; done=; n=0; ",
    "while rd; do case \"$l\" in D*) part=${l#D}; p=$p$part; n=$((n+1)); ",
    "command printf \"\\036%s:INPUT:%s:%s:%s\\037\" \"$op\" \"$n\" \"${#part}\" \"$part\";; ",
    "E:*) [ \"$l\" = \"E:$n:${#p}\" ] || break; done=1; ",
    "command printf \"\\036%s:INPUT_END:%s:%s\\037\" \"$op\" \"$n\" \"${#p}\"; break;; *) break;; esac; done; ",
    "[ \"$done\" ] || { command printf \"\\036%s:INPUT_INVALID\\037\" \"$op\"; exit 125; }; ",
    "case $fmt in b64) u_cmd=$(command printf %s \"$p\" | command base64 -d 2>/dev/null) || exit 125;; ",
    "*) u_cmd=$(command printf %b \"$p\") || exit 125;; esac; ",
    // Emit BEGIN from the execution child, after its signal handlers exist.
    // fd 3 is captured outside the encoding pipe and closed for user commands.
    "{ { ( trap - 0 2 15; command printf \"\\036%s:BEGIN\\037\" \"$op\" >&3; eval \"$u_cmd\" 3>&- ) </dev/null 2>/dev/null; u_rc=$?; ",
    "command printf \"\\036%s:STATUS:%s\\037\" \"$op\" \"$u_rc\"; } | command base64; } 3>&1; ",
    "u_rc=$?; printf \"\\036%s:END:%s\\037\" \"$op\" \"$u_rc\"; ",
    "rd; [ \"$l\" = Q ] || exit 125'\n",
);

pub(super) fn decode(
    encoded: &[u8],
    nonce: &str,
    max_bytes: usize,
) -> Result<(Vec<u8>, i32), &'static str> {
    let mut bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "SSH_TERMINAL_PROTOCOL_ERROR")?;
    let prefix = format!("\x1e{nonce}:STATUS:").into_bytes();
    let position = bytes
        .windows(prefix.len())
        .rposition(|part| part == prefix)
        .ok_or("SSH_TERMINAL_PROTOCOL_ERROR")?;
    let ending = &bytes[position + prefix.len()..];
    let end = ending
        .iter()
        .position(|b| *b == 0x1f)
        .ok_or("SSH_TERMINAL_PROTOCOL_ERROR")?;
    let digits = &ending[..end];
    if digits.is_empty() || digits.len() > 3 || !digits.iter().all(u8::is_ascii_digit) {
        return Err("SSH_TERMINAL_PROTOCOL_ERROR");
    }
    let code: u8 = std::str::from_utf8(digits)
        .map_err(|_| "SSH_TERMINAL_PROTOCOL_ERROR")?
        .parse()
        .map_err(|_| "SSH_TERMINAL_PROTOCOL_ERROR")?;
    bytes.drain(position..position + prefix.len() + end + 1);
    if bytes.len() > max_bytes {
        return Err("SSH_TERMINAL_OUTPUT_LIMIT");
    }
    Ok((bytes, i32::from(code)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_bytes_and_long_json_survive_base64_and_correlated_status() {
        let mut expected: Vec<u8> = (0..=255).cycle().take(90_000).collect();
        expected.extend_from_slice(b"\x1efake:STATUS:0\x1f");
        let mut wire = expected.clone();
        wire.extend_from_slice(b"\x1enonce:STATUS:23\x1f");
        assert_eq!(
            decode(STANDARD.encode(&wire).as_bytes(), "nonce", 100_000).unwrap(),
            (expected, 23)
        );
        assert_eq!(
            decode(STANDARD.encode(&wire).as_bytes(), "wrong", 100_000),
            Err("SSH_TERMINAL_PROTOCOL_ERROR")
        );
        assert_eq!(
            decode(STANDARD.encode(&wire).as_bytes(), "nonce", 64_000),
            Err("SSH_TERMINAL_OUTPUT_LIMIT")
        );
    }
    #[test]
    fn code_must_fit_a_shell_exit_and_background_output_is_preserved() {
        for bytes in [b"\x1en:STATUS:256\x1f".as_slice(), b"\x1en:STATUS:-1\x1f"] {
            assert!(decode(STANDARD.encode(bytes).as_bytes(), "n", 100).is_err());
        }
        assert_eq!(
            decode(
                STANDARD.encode(b"head\x1en:STATUS:0\x1ftail").as_bytes(),
                "n",
                100
            )
            .unwrap(),
            (b"headtail".to_vec(), 0)
        );
    }
    #[test]
    fn bootstrap_fits_terminal_line() {
        assert!(BOOTSTRAP.len() <= 1721);
    }
}
