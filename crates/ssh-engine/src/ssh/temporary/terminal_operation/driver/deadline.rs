use std::time::Duration;
use tokio::time::Instant;

#[derive(Clone, Copy)]
pub(super) enum Stage {
    Startup,
    Ready,
    Accept,
    InputAck,
    Begin,
    End,
    Restore,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn phase_limits_do_not_extend_total_and_cleanup_has_separate_budget() {
        let mut deadline = Deadline::new(Some(15_000));
        tokio::time::advance(Duration::from_secs(9)).await;
        deadline.enter(Stage::Ready);
        tokio::time::advance(Duration::from_secs(6)).await;
        assert_eq!(deadline.check(), Err("SSH_TERMINAL_READY_TIMEOUT"));
        deadline.enter(Stage::Accept);
        assert_eq!(deadline.check(), Err("SSH_TERMINAL_ACCEPT_TIMEOUT"));
        deadline.enter(Stage::Restore);
        assert_eq!(deadline.check(), Ok(()));
        tokio::time::advance(Duration::from_secs(3)).await;
        assert_eq!(deadline.check(), Err("SSH_TERMINAL_RESTORE_TIMEOUT"));
    }

    #[tokio::test(start_paused = true)]
    async fn unset_and_zero_have_bounded_execution_and_each_handshake_has_its_own_code() {
        for timeout in [None, Some(0)] {
            let mut deadline = Deadline::new(timeout);
            deadline.enter(Stage::End);
            tokio::time::advance(Duration::from_secs(30)).await;
            assert_eq!(deadline.check(), Err("SSH_TERMINAL_END_TIMEOUT"));
        }
        for (stage, code) in [
            (Stage::Startup, "SSH_TERMINAL_STARTUP_TIMEOUT"),
            (Stage::Ready, "SSH_TERMINAL_READY_TIMEOUT"),
            (Stage::Accept, "SSH_TERMINAL_ACCEPT_TIMEOUT"),
            (Stage::InputAck, "SSH_TERMINAL_INPUT_ACK_TIMEOUT"),
            (Stage::Begin, "SSH_TERMINAL_BEGIN_TIMEOUT"),
        ] {
            let mut deadline = Deadline::new(Some(60_000));
            deadline.enter(stage);
            tokio::time::advance(Duration::from_secs(10)).await;
            assert_eq!(deadline.check(), Err(code));
        }
    }
}

impl Stage {
    fn error(self) -> &'static str {
        match self {
            Self::Startup => "SSH_TERMINAL_STARTUP_TIMEOUT",
            Self::Ready => "SSH_TERMINAL_READY_TIMEOUT",
            Self::Accept => "SSH_TERMINAL_ACCEPT_TIMEOUT",
            Self::InputAck => "SSH_TERMINAL_INPUT_ACK_TIMEOUT",
            Self::Begin => "SSH_TERMINAL_BEGIN_TIMEOUT",
            Self::End => "SSH_TERMINAL_END_TIMEOUT",
            Self::Restore => "SSH_TERMINAL_RESTORE_TIMEOUT",
        }
    }
}

pub(super) struct Deadline {
    total: Instant,
    current: Instant,
    stage: Stage,
}

impl Deadline {
    pub(super) fn new(timeout_ms: Option<u64>) -> Self {
        let now = Instant::now();
        let duration = Duration::from_millis(timeout_ms.filter(|ms| *ms > 0).unwrap_or(30_000));
        let total = now
            .checked_add(duration)
            .unwrap_or(now + Duration::from_secs(30));
        Self {
            total,
            current: total.min(now + Duration::from_secs(10)),
            stage: Stage::Startup,
        }
    }

    pub(super) fn enter(&mut self, stage: Stage) {
        self.stage = stage;
        self.current = match stage {
            // Cleanup has its own small budget even if the operation expired.
            Stage::Restore => Instant::now() + Duration::from_secs(3),
            Stage::End => self.total,
            _ => self.total.min(Instant::now() + Duration::from_secs(10)),
        };
    }

    pub(super) fn at(&self) -> Instant {
        self.current
    }
    pub(super) fn error(&self) -> &'static str {
        self.stage.error()
    }
    pub(super) fn check(&self) -> Result<(), &'static str> {
        if Instant::now() >= self.current {
            Err(self.error())
        } else {
            Ok(())
        }
    }
}
