//! Reconnect schedule, restart budget and disconnected command replies
//! (VOL-ARCH-041, 043, 052). Pure: the thread in `pw/mod.rs` drives it.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::backend::{BackendError, BackendEvent, Command};

/// Delay before the first retry.
const BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Longest delay between two attempts.
const BACKOFF_CAP: Duration = Duration::from_secs(10);
/// Panic restarts allowed within [`RESTART_WINDOW`].
const MAX_RESTARTS: usize = 5;
/// Sliding window of the restart budget.
const RESTART_WINDOW: Duration = Duration::from_secs(60);
/// Status carried by command replies once the restart budget is spent.
const BACKEND_CRASHED: &str = "backend crashed";

/// `min(10 s, 0.5 s × 2^(attempt − 1))` for `attempt ≥ 1`.
fn backoff(attempt: u32) -> Duration {
    // 0.5 s × 2^5 already exceeds the cap, so the shift never overflows.
    let doublings = attempt.saturating_sub(1).min(5);
    BACKOFF_BASE.saturating_mul(1 << doublings).min(BACKOFF_CAP)
}

/// Counts disconnections since the last `Connected` (VOL-ARCH-041).
#[derive(Debug, Default)]
pub(super) struct Reconnect {
    attempt: u32,
}

impl Reconnect {
    /// A connection reached `Connected`: the next failure is attempt 1 again.
    pub(super) fn connected(&mut self) {
        self.attempt = 0;
    }

    /// A connection failed or was lost. Returns the number of the retry this
    /// schedules (the `attempt` of `Disconnected`) and the delay before it.
    pub(super) fn failed(&mut self) -> (u32, Duration) {
        self.attempt = self.attempt.saturating_add(1);
        (self.attempt, backoff(self.attempt))
    }
}

/// At most [`MAX_RESTARTS`] panic restarts within any [`RESTART_WINDOW`]
/// (VOL-ARCH-052).
#[derive(Debug, Default)]
pub(super) struct RestartBudget {
    recent: VecDeque<Instant>,
}

impl RestartBudget {
    /// True (and the restart is counted) if a restart at `now` is allowed.
    pub(super) fn allow(&mut self, now: Instant) -> bool {
        while self
            .recent
            .front()
            .is_some_and(|&at| now.saturating_duration_since(at) >= RESTART_WINDOW)
        {
            self.recent.pop_front();
        }
        if self.recent.len() >= MAX_RESTARTS {
            return false;
        }
        self.recent.push_back(now);
        true
    }
}

/// The reply to `cmd` while there is no connection (VOL-ARCH-043):
/// `CommandFailed{NotConnected}` for every command except `Shutdown` and
/// `SetMeters`, which get none. Once the backend has `crashed` for good, the
/// error is `Pipewire(0, "backend crashed")` instead (VOL-ARCH-052).
pub(super) fn reply_while_disconnected(cmd: &Command, crashed: bool) -> Option<BackendEvent> {
    let error = if crashed {
        BackendError::Pipewire(0, BACKEND_CRASHED.to_owned())
    } else {
        BackendError::NotConnected
    };
    cmd.kind()
        .map(|cmd| BackendEvent::CommandFailed { cmd, error })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{CommandKind, DefaultKind};
    use crate::model::ObjectId;

    #[test]
    fn backoff_schedule_is_0_5_1_2_4_8_10_10() {
        let mut reconnect = Reconnect::default();
        let schedule: Vec<_> = (0..8).map(|_| reconnect.failed()).collect();
        let ms = Duration::from_millis;
        assert_eq!(
            schedule,
            [
                (1, ms(500)),
                (2, ms(1000)),
                (3, ms(2000)),
                (4, ms(4000)),
                (5, ms(8000)),
                (6, ms(10_000)),
                (7, ms(10_000)),
                (8, ms(10_000)),
            ]
        );
        assert_eq!(backoff(u32::MAX), BACKOFF_CAP);
    }

    #[test]
    fn attempt_resets_after_connected() {
        let mut reconnect = Reconnect::default();
        reconnect.failed();
        reconnect.failed();
        reconnect.connected();
        assert_eq!(reconnect.failed(), (1, Duration::from_millis(500)));
    }

    #[test]
    fn restart_budget_allows_five_per_minute_then_denies() {
        let start = Instant::now();
        let at = |secs| start + Duration::from_secs(secs);
        let mut budget = RestartBudget::default();
        for secs in 0..5 {
            assert!(budget.allow(at(secs)), "restart {secs}");
        }
        assert!(!budget.allow(at(30)));
        assert!(!budget.allow(at(59)));
        // The first restart left the window: one more is allowed, then none.
        assert!(budget.allow(at(60)));
        assert!(!budget.allow(at(60)));
    }

    #[test]
    fn command_while_disconnected_maps_to_not_connected_except_shutdown() {
        let failed = |cmd| BackendEvent::CommandFailed {
            cmd,
            error: BackendError::NotConnected,
        };
        let node = ObjectId(5);
        let cases = [
            (
                Command::SetVolumes {
                    node,
                    channel_volumes: vec![0.5],
                    master: None,
                },
                Some(failed(CommandKind::SetVolumes)),
            ),
            (
                Command::SetMute { node, mute: true },
                Some(failed(CommandKind::SetMute)),
            ),
            (
                Command::SetDefault {
                    kind: DefaultKind::Sink,
                    node_name: "x".into(),
                },
                Some(failed(CommandKind::SetDefault)),
            ),
            (
                Command::MoveStream {
                    stream: node,
                    target: None,
                },
                Some(failed(CommandKind::MoveStream)),
            ),
            (
                Command::SetProfile {
                    device: node,
                    index: 1,
                },
                Some(failed(CommandKind::SetProfile)),
            ),
            (
                Command::SetRoute {
                    device: node,
                    route_index: 1,
                    card_profile_device: 0,
                },
                Some(failed(CommandKind::SetRoute)),
            ),
            (Command::SetMeters { targets: vec![] }, None),
            (Command::Shutdown, None),
        ];
        for (cmd, expected) in cases {
            assert_eq!(reply_while_disconnected(&cmd, false), expected, "{cmd:?}");
        }
    }

    #[test]
    fn command_after_crash_budget_reports_backend_crashed() {
        let mute = Command::SetMute {
            node: ObjectId(5),
            mute: false,
        };
        assert_eq!(
            reply_while_disconnected(&mute, true),
            Some(BackendEvent::CommandFailed {
                cmd: CommandKind::SetMute,
                error: BackendError::Pipewire(0, BACKEND_CRASHED.to_owned()),
            })
        );
        assert_eq!(reply_while_disconnected(&Command::Shutdown, true), None);
    }
}
