//! Conservative argument forms for diagnostics that also have write modes.
//! Unknown options and abbreviations must use the separately authorized exec path.

pub(super) fn readonly_arguments(head: &str, args: &[&str]) -> bool {
    match head {
        "hostname" => args.iter().all(|a| {
            matches!(
                *a,
                "-s" | "--short"
                    | "-f"
                    | "--fqdn"
                    | "-d"
                    | "--domain"
                    | "-i"
                    | "--ip-address"
                    | "-I"
                    | "--all-ip-addresses"
                    | "-A"
                    | "--all-fqdns"
            )
        }),
        "date" => args.iter().all(|a| {
            a.starts_with('+')
                || matches!(
                    *a,
                    "-u" | "--utc" | "--universal" | "-R" | "--rfc-email" | "-I" | "--iso-8601"
                )
        }),
        "mount" => args.is_empty() || args == ["-l"],
        "ifconfig" => {
            args.is_empty() || (args.len() == 1 && (args[0] == "-a" || identifier(args[0])))
        }
        "ip" => readonly_ip(args),
        "ss" => args.iter().all(|a| {
            // In particular exclude -K/--kill and -D/--diag (writes a file).
            a.starts_with('-')
                && a.len() > 1
                && !a.starts_with("--")
                && a[1..].chars().all(|c| "46tulnapeorismx".contains(c))
        }),
        "journalctl" => readonly_journal(args),
        _ => true,
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c))
}

fn readonly_ip(mut args: &[&str]) -> bool {
    while args
        .first()
        .is_some_and(|a| matches!(*a, "-4" | "-6" | "-s" | "-d" | "-o" | "-j" | "-br"))
    {
        args = &args[1..];
    }
    if !args.first().is_some_and(|a| {
        matches!(
            *a,
            "address" | "addr" | "link" | "route" | "neigh" | "neighbor" | "rule"
        )
    }) {
        return false;
    }
    args = &args[1..];
    if args.is_empty() {
        return true;
    }
    if !matches!(args[0], "show" | "list") {
        return false;
    }
    args = &args[1..];
    args.is_empty() || (args.len() == 2 && args[0] == "dev" && identifier(args[1]))
}

fn readonly_journal(mut args: &[&str]) -> bool {
    while let Some(arg) = args.first() {
        args = &args[1..];
        if matches!(
            *arg,
            "--no-pager"
                | "--no-hostname"
                | "--utc"
                | "--reverse"
                | "-r"
                | "-f"
                | "--follow"
                | "-k"
                | "--dmesg"
                | "--disk-usage"
                | "--list-boots"
                | "--system"
                | "--user"
        ) {
            continue;
        }
        let (flag, inline) = arg
            .split_once('=')
            .map_or((*arg, None), |(f, v)| (f, Some(v)));
        if !matches!(
            flag,
            "-u" | "--unit"
                | "-n"
                | "--lines"
                | "-b"
                | "--boot"
                | "--since"
                | "--until"
                | "-p"
                | "--priority"
                | "-o"
                | "--output"
                | "--identifier"
        ) {
            return false;
        }
        let value = if let Some(value) = inline {
            value
        } else {
            let Some(value) = args.first() else {
                return false;
            };
            args = &args[1..];
            value
        };
        if value.is_empty() || value.starts_with('-') {
            return false;
        }
    }
    true
}
