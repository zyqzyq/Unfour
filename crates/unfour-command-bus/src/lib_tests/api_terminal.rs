use super::*;

#[tokio::test]
async fn terminal_http_budget_does_not_change_desktop_unlimited_or_positive_preferences() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = api_script_test_input(ws, "http://unused.invalid".into());
    for requested in [None, Some(0)] {
        input.timeout_ms = requested;
        let ordinary = bus
            .resolve_desktop_api_timeout(input.clone())
            .await
            .unwrap();
        assert_eq!(ordinary.timeout_ms, Some(0));
        let terminal = unfour_http_engine::prepare_terminal_curl(ordinary.clone(), &[]).unwrap();
        assert_eq!(terminal.timeout_ms(), Some(30_000));
        assert_eq!(ordinary.timeout_ms, Some(0));
        assert_eq!(
            bus.api_client_preferences()
                .await
                .unwrap()
                .request_timeout_ms,
            0
        );
    }
    bus.update_api_client_preferences(ApiClientPreferences {
        request_timeout_ms: 90_000,
    })
    .await
    .unwrap();
    for (requested, expected) in [(None, 90_000), (Some(5_000), 5_000), (Some(0), 30_000)] {
        input.timeout_ms = requested;
        let resolved = bus
            .resolve_desktop_api_timeout(input.clone())
            .await
            .unwrap();
        let terminal = unfour_http_engine::prepare_terminal_curl(resolved, &[]).unwrap();
        assert_eq!(terminal.timeout_ms(), Some(expected));
    }
    assert_eq!(
        bus.api_client_preferences()
            .await
            .unwrap()
            .request_timeout_ms,
        90_000
    );
}

#[tokio::test]
async fn terminal_http_rejects_unconfirmed_or_ambiguous_execution_before_scripts() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let env = bus
        .workspace_environment_create(ws.clone(), "terminal-http".into())
        .await
        .unwrap();
    let mut input = api_script_test_input(ws.clone(), "http://unused.invalid".into());
    input.pre_request_script = Some(r#"pm.environment.set("unexpected", "changed")"#.into());
    for (mode, handle, confirmed, code) in [
        ("auto", None, true, "SSH_TERMINAL_HTTP_MODE_INVALID"),
        (
            "terminalCurl",
            None,
            true,
            "SSH_TERMINAL_HTTP_TARGET_REQUIRED",
        ),
        (
            "terminalCurl",
            Some(" "),
            true,
            "SSH_TERMINAL_HTTP_TARGET_REQUIRED",
        ),
        (
            "terminalCurl",
            Some("unselected"),
            false,
            "SSH_TERMINAL_HTTP_CONFIRMATION_REQUIRED",
        ),
    ] {
        let err = bus
            .send_api_request_via_with_mode(
                "terminal-http-preflight",
                input.clone(),
                Some(env.id.clone()),
                None,
                None,
                handle,
                mode,
                confirmed,
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), code);
    }
    assert!(bus
        .workspace_environments_list(ws)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == env.id)
        .unwrap()
        .variables
        .is_empty());
}
