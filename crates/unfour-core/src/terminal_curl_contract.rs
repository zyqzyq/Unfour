//! Shared, non-sensitive shell preflight for terminal requests and tool probes.
//! The synthetic CLI arguments are parsed strictly even on 7.29, which would
//! silently ignore unknown options in a config file. --version must come last.
pub const CURL_SETUP: &str = concat!(
    "command -v curl >/dev/null 2>&1 || exit 127; ",
    "case $(command -v printf):$(LC_ALL=C command -V printf 2>/dev/null) in printf:*builtin*) ;; *) exit 126;; esac; ",
    "u_c() { LC_ALL=C SSLKEYLOGFILE= command curl -q --silent --include --globoff --no-location --no-netrc --retry 0 --proxy '' --noproxy '*' --proto '=http,https' --proto-redir '=http,https' --max-redirs 0 \"$@\"; }; ",
    "v=$(u_c --config - --url http://127.0.0.1:1/ --request POST --header X:Y --data-binary '{}' --max-filesize 1 --max-time 1 --write-out x --version </dev/null 2>/dev/null) || exit 125; ",
    // For modern/unknown versions pass the flag on the actual CLI, where an
    // unsupported option fails before transfer. Only old HTTP/1-only versions
    // may use the default. A backport advertising HTTP2/3 still needs the flag.
    "h=--http1.1; case \"$v\" in *HTTP2*|*HTTP3*) ;; 'curl 7.29.'*|'curl 7.3'[0-2].*) h=;; esac; "
);
