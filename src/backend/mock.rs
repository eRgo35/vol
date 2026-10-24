//! Scriptable in-process backend for tests (compiled under `cfg(test)` or feature `mock`).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::{
    AudioBackend, BackendError, BackendEvent, BackendGone, BackendHandle, Command, DisconnectReason,
};

/// What the mock backend emits on spawn.
#[derive(Debug, Clone, PartialEq)]
pub struct MockScript {
    /// Events delivered immediately, in order.
    pub initial: Vec<BackendEvent>,
}

/// A script that reports a connected daemon with nothing in it.
pub fn empty_connected() -> MockScript {
    MockScript {
        initial: vec![BackendEvent::Connected {
            server_version: "1.6.9".to_owned(),
            socket: "pipewire-0".to_owned(),
        }],
    }
}

/// A script that reports a failed first connection attempt.
pub fn disconnected(reason: DisconnectReason) -> MockScript {
    MockScript {
        initial: vec![BackendEvent::Disconnected {
            reason,
            retry_in: Some(Duration::from_millis(500)),
            attempt: 1,
        }],
    }
}

/// State shared between [`MockBackend`] and [`MockControl`].
#[derive(Default)]
struct Shared {
    commands: Vec<Command>,
    fail_next: Option<BackendError>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Test-side control of a running mock backend.
#[derive(Clone)]
pub struct MockControl {
    events: UnboundedSender<BackendEvent>,
    shared: Arc<Mutex<Shared>>,
}

impl MockControl {
    /// Delivers `event` to the app side.
    pub fn push(&self, event: BackendEvent) -> Result<(), BackendGone> {
        self.events.send(event).map_err(|_| BackendGone)
    }

    /// Every command sent so far, in order.
    pub fn commands(&self) -> Vec<Command> {
        lock(&self.shared).commands.clone()
    }

    /// Makes the next failable command (one with a [`Command::kind`]) emit
    /// `CommandFailed` with `error`.
    pub fn fail_next(&self, error: BackendError) {
        lock(&self.shared).fail_next = Some(error);
    }
}

/// The [`AudioBackend`] side of the mock.
struct MockBackend {
    events: UnboundedSender<BackendEvent>,
    shared: Arc<Mutex<Shared>>,
}

impl AudioBackend for MockBackend {
    fn send(&self, cmd: Command) -> Result<(), BackendGone> {
        if self.events.is_closed() {
            return Err(BackendGone);
        }
        let mut shared = lock(&self.shared);
        let kind = cmd.kind();
        shared.commands.push(cmd);
        if let Some(cmd) = kind
            && let Some(error) = shared.fail_next.take()
        {
            self.events
                .send(BackendEvent::CommandFailed { cmd, error })
                .map_err(|_| BackendGone)?;
        }
        Ok(())
    }
}

/// Starts a mock backend: `script.initial` is queued at once, then events come
/// only from [`MockControl::push`] and armed failures.
pub fn spawn_mock(script: MockScript) -> (BackendHandle, MockControl) {
    let (sender, events) = unbounded_channel();
    for event in script.initial {
        // The receiver is alive (owned locally), so this cannot fail.
        let _ = sender.send(event);
    }
    let shared = Arc::new(Mutex::new(Shared::default()));
    let backend = MockBackend {
        events: sender.clone(),
        shared: Arc::clone(&shared),
    };
    let handle = BackendHandle {
        commands: Box::new(backend),
        events,
        thread: None,
    };
    (
        handle,
        MockControl {
            events: sender,
            shared,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{BackendError, BackendGone, BackendHandle, Command, CommandKind};
    use crate::model::ObjectId;
    use tokio::sync::mpsc::error::TryRecvError;

    fn mute(node: u32) -> Command {
        Command::SetMute {
            node: ObjectId(node),
            mute: true,
        }
    }

    #[test]
    fn initial_events_are_delivered_in_order() {
        let mut script = empty_connected();
        script.initial.push(BackendEvent::Batch(vec![]));
        let (mut handle, _control) = spawn_mock(script);
        assert!(matches!(
            handle.events.try_recv(),
            Ok(BackendEvent::Connected { .. })
        ));
        assert_eq!(handle.events.try_recv(), Ok(BackendEvent::Batch(vec![])));
        assert_eq!(handle.events.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn push_after_spawn_is_delivered() {
        let (mut handle, control) = spawn_mock(MockScript { initial: vec![] });
        assert_eq!(handle.events.try_recv(), Err(TryRecvError::Empty));
        let event = BackendEvent::Meters(vec![]);
        control.push(event.clone()).unwrap();
        assert_eq!(handle.events.try_recv(), Ok(event));
    }

    #[test]
    fn commands_are_recorded_in_order() {
        let (handle, control) = spawn_mock(MockScript { initial: vec![] });
        handle.commands.send(mute(1)).unwrap();
        handle.commands.send(Command::Shutdown).unwrap();
        handle.commands.send(mute(2)).unwrap();
        assert_eq!(control.commands(), [mute(1), Command::Shutdown, mute(2)]);
    }

    #[test]
    fn fail_next_yields_command_failed_once() {
        let (mut handle, control) = spawn_mock(MockScript { initial: vec![] });
        control.fail_next(BackendError::NodeGone(ObjectId(1)));
        handle.commands.send(mute(1)).unwrap();
        assert_eq!(
            handle.events.try_recv(),
            Ok(BackendEvent::CommandFailed {
                cmd: CommandKind::SetMute,
                error: BackendError::NodeGone(ObjectId(1)),
            })
        );
        handle.commands.send(mute(1)).unwrap();
        assert_eq!(handle.events.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(control.commands(), [mute(1), mute(1)]);
    }

    #[test]
    fn send_after_receiver_dropped_returns_backend_gone() {
        let (handle, control) = spawn_mock(disconnected(DisconnectReason::NoDaemon));
        let BackendHandle {
            commands, events, ..
        } = handle;
        drop(events);
        assert_eq!(commands.send(mute(1)), Err(BackendGone));
        assert_eq!(control.push(BackendEvent::Meters(vec![])), Err(BackendGone));
    }
}
