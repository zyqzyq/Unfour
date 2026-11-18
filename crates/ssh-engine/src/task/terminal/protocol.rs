//! Parent history receives only this fixed, secret-free bootstrap. Commands
//! arrive later on the non-interactive child's stdin after echo is disabled.
pub(super) const BOOTSTRAP: &str = concat!(
    " sh -c '",
    "set +avx; unset u_payload u_line u_part u_cmd; u_tty=$(stty -g) || exit 125; u_session=; ",
    "u_restore() { stty \"$u_tty\" && { [ -z \"$u_session\" ] || printf \"\\036%s:RESTORED\\037\" \"$u_session\"; }; }; ",
    "trap u_restore 0; trap \"exit 130\" 2 15; stty icanon isig intr ^C icrnl -inlcr -igncr -echo -noflsh || exit 125; ",
    "case $(command -v printf):$(LC_ALL=C command -V printf 2>/dev/null) in printf:*builtin*) ;; *) exit 125;; esac; ",
    "printf \"\\036UNFOUR_TASK_V2\\037\"; IFS= read -r u_session || exit 125; ",
    "printf \"\\036%s:READY\\037\" \"$u_session\"; ",
    "while IFS= read -r u_id; do [ \"$u_id\" = Q ] && exit 0; ",
    "stty icanon isig intr ^C icrnl -inlcr -igncr -echo -noflsh || exit 125; printf \"\\036%s:ACCEPT\\037\" \"$u_id\"; u_payload=; u_n=0; u_complete=; ",
    "while IFS= read -r u_line; do case \"$u_line\" in D*) u_part=${u_line#D}; u_payload=$u_payload$u_part; u_n=$((u_n+1)); ",
    "command printf \"\\036%s:INPUT:%s:%s:%s\\037\" \"$u_id\" \"$u_n\" \"${#u_part}\" \"$u_part\";; E:*) [ \"$u_line\" = \"E:$u_n:${#u_payload}\" ] && u_complete=1; break;; *) exit 125;; esac; done; ",
    "[ \"$u_complete\" = 1 ] || { printf \"\\036%s:INPUT_INVALID\\037\" \"$u_id\"; exit 125; }; ",
    "printf \"\\036%s:INPUT_END:%s:%s\\037\" \"$u_id\" \"$u_n\" \"${#u_payload}\"; ",
    "u_cmd=$(command printf \"%b\" \"$u_payload\") || exit 125; printf \"\\036%s:BEGIN\\037\" \"$u_id\"; ",
    "{ ( trap - 0 2 15; eval \"$u_cmd\" ) </dev/null; u_rc=$?; ",
    "printf \"\\036%s:STATUS:%s\\037\" \"$u_id\" \"$u_rc\"; } 2>&1 | cat; ",
    "printf \"\\036%s:END\\037\" \"$u_id\"; done'\n",
);

pub(super) const STARTUP: &str = "UNFOUR_TASK_V2";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::terminal_wire::{Fragment, Parser};

    #[test]
    fn fixed_bootstrap_fits_one_canonical_line_and_cannot_echo_a_handshake() {
        assert!(BOOTSTRAP.len() < 2048);
        assert!(Parser::default()
            .push(BOOTSTRAP.as_bytes())
            .iter()
            .all(|fragment| matches!(fragment, Fragment::Data(_))));
    }
}
