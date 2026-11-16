use super::*;

fn fixture() -> Vec<String> {
    [
        "/T",
        "/N",
        "Synthetic asset@other.invalid",
        "/SSH2",
        "gateway.invalid",
        "/P",
        "22022",
        "/L",
        "synthetic-user",
        "/PASSWORD",
        "synthetic-grant",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn observed_scrt_order_uses_host_and_auth_fields_not_label() {
    let args = fixture();
    let Ok(Launch::Terminal(endpoint)) = parse(&args) else {
        panic!("synthetic SecureCRT argv rejected")
    };
    assert_eq!((endpoint.host, endpoint.port), ("gateway.invalid", 22022));
    assert_eq!(endpoint.label, args[2]);
    assert_eq!(
        (endpoint.identity, endpoint.credential),
        ("synthetic-user", "synthetic-grant")
    );
}

#[test]
fn separate_values_are_preserved_without_uri_decoding_or_switch_interpretation() {
    let mut args = fixture();
    args[2] = "合成 标签 /SCRIPT".into();
    args[4] = "127.0.0.1".into();
    args[8] = "用户@tenant/asset:%40".into();
    args[10] = " /SCRIPT secret:%40+&=\\\" ".into();
    for index in [0, 1, 3, 5, 7, 9] {
        args[index] = args[index].to_ascii_lowercase();
    }
    let Ok(Launch::Terminal(endpoint)) = parse(&args) else {
        panic!("literal synthetic credential rejected")
    };
    assert_eq!(endpoint.identity, args[8]);
    assert_eq!(endpoint.credential, args[10]);
    assert_eq!(endpoint.host, args[4]);
}

#[test]
fn missing_reordered_duplicate_and_execution_options_are_rejected() {
    for index in 0..11 {
        let mut args = fixture();
        args.remove(index);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for index in [0, 1, 3, 5, 7, 9] {
        let mut args = fixture();
        args[index] = "/SCRIPT".into();
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for suffix in [
        vec!["/SCRIPT", "synthetic.vbs"],
        vec!["/PASSWORD", "synthetic-duplicate"],
        vec!["--task", "synthetic-task"],
        vec!["--flow", "synthetic-flow"],
        vec!["-exec", "synthetic-command"],
    ] {
        let mut args = fixture();
        args.extend(suffix.into_iter().map(str::to_owned));
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    let mut args = fixture();
    args.swap(5, 7);
    args.swap(6, 8);
    assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
}

#[test]
fn ambiguous_endpoints_ports_and_forwarding_delimiters_fail_without_echo() {
    for host in [
        "",
        "-option",
        "ssh://gateway.invalid",
        "host:22",
        "127.1",
        "[::1]",
        "bad host",
        "host/path",
    ] {
        let mut args = fixture();
        args[4] = host.into();
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for port in ["", "0", "022", "65536", "-1", "+22", "22 ", "port"] {
        let mut args = fixture();
        args[6] = port.into();
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for index in [2, 4, 6, 8, 10] {
        for value in [
            "",
            "synthetic|extra",
            "synthetic\nextra",
            "synthetic\0extra",
        ] {
            let mut args = fixture();
            args[index] = value.into();
            assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
        }
    }
    let mut args = fixture();
    args[10] = "s".repeat(65536);
    assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
}

#[cfg(windows)]
#[test]
fn invalid_unicode_in_any_value_is_rejected() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    for index in [2, 4, 6, 8, 10] {
        let mut args: Vec<OsString> = fixture().into_iter().map(Into::into).collect();
        args[index] = OsString::from_wide(&[0xd800]);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
}

#[cfg(windows)]
#[test]
fn scrt_requests_expose_display_metadata_without_serializing_credentials() {
    let bus = tauri::async_runtime::block_on(CommandBus::ephemeral()).unwrap();
    let args = fixture();
    for _ in 0..2 {
        assert!(deliver(&args, Some(&bus)).unwrap());
    }
    let pending = bus.list_temporary_ssh("").unwrap();
    assert_eq!(pending.len(), 2);
    assert_ne!(pending[0].handle, pending[1].handle);
    assert!(pending
        .iter()
        .all(|p| p.workspace_id.is_none() && p.status == "pending"));
    let json = serde_json::to_string(&pending).unwrap();
    assert!(pending
        .iter()
        .all(|p| p.display_name == args[2] && p.endpoint == "gateway.invalid:22022"));
    for index in [8, 10] {
        assert!(!json.contains(&args[index]));
    }
    for entry in pending {
        bus.cancel_temporary_ssh("", &entry.handle).unwrap();
    }
}
