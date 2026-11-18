//! Run against synthetic commands emitted by terminal-curl-qa for this build.
//! The loopback SSH peer delays each complete input line, returns 97 bytes,
//! and never executes shell commands or contacts an HTTP endpoint.
use super::*;

#[tokio::test]
#[ignore = "requires small-json and json emission directories under UNFOUR_LATENCY_FIXTURES"]
async fn measured_terminal_turnaround() {
    let dir = std::path::PathBuf::from(std::env::var_os("UNFOUR_LATENCY_FIXTURES").unwrap());
    for (case, delay_ms, expect_timeout) in [
        ("small-json", 680, false),
        ("json", 5, false),
        ("json", 680, true),
    ] {
        let command = std::fs::read_to_string(dir.join(case).join("command.sh")).unwrap();
        let f = raw_fixture().await;
        f.state.lock().unwrap().terminal_line_delay = Some(Duration::from_millis(delay_ms));
        let lease = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap();
        let start = std::time::Instant::now();
        let result = lease
            .run(&command, None, CancellationToken::new(), 1024)
            .await;
        let elapsed = start.elapsed().as_millis();
        let state = f.state.lock().unwrap();
        eprintln!("MEASURE case={case} delay_ms={delay_ms} bytes={} lines={} data_acks={} executions={} elapsed_ms={elapsed} success={}",
            command.len(), state.terminal_raw_input_lines.len(), state.terminal_raw_input_acks_sent,
            state.terminal_commands.len(), result.is_ok());
        let acks = state.terminal_raw_input_acks_sent;
        let executions = state.terminal_commands.clone();
        let pipelined = state.terminal_raw_pipelined_input;
        drop(state);
        if expect_timeout {
            assert!(result
                .err()
                .unwrap()
                .to_string()
                .contains("SSH_TERMINAL_INPUT_ACK_TIMEOUT"));
            assert!(executions.is_empty());
        } else {
            let result = result.unwrap();
            eprintln!("PHASE prepare_ms={} input_ms={} ack_count={} ack_wait_ms={} execution_output_ms={} restore_ms={}",
                result.timing.prepare_ms, result.timing.input_ms, result.timing.input_ack_count,
                result.timing.input_ack_wait_ms, result.timing.execution_and_output_ms, result.timing.restore_ms);
            assert_eq!(result.timing.input_ack_count, acks + 1);
            assert!(result.terminal_restored);
            assert_eq!(result.stdout, vec![b'x'; 97]);
            assert!(executions.len() == 1 && executions[0] == command);
        }
        assert!(!pipelined);
        assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    }
}
