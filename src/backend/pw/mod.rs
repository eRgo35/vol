//! PipeWire backend (`docs/specs/01-architecture.md` §3). The only place that uses `pipewire`.
//!
//! [`spawn_pipewire`] starts the OS thread `vol-pw`, which owns every PipeWire
//! object (VOL-ARCH-010). Commands reach it through a `pipewire::channel`
//! attached to its main loop (VOL-ARCH-011); events leave through a tokio
//! unbounded channel, whose non-async `send` never touches PipeWire.
//!
//! # Shared state
//!
//! Each connection is a `conn::Session` owning one `State` behind
//! `Rc<RefCell<…>>`. Listener closures hold only `Weak` references (to the
//! state, the registry, the main loop), so there are no reference cycles;
//! they borrow the state briefly through `conn::with_state` and never call
//! into PipeWire while it is borrowed (binding, `sync`, `quit` and dropping
//! proxies all happen outside the borrow).
//!
//! # Teardown order
//!
//! Teardown never happens inside a PipeWire callback: a fatal core error only
//! sets a flag and quits the loop, and `run` drops the session after
//! `MainLoop::run` returns. A session drops, in this order (its field order):
//!
//! 1. the state, with every bound node/device and the `default` metadata:
//!    each listener before its proxy (a proxy destroy frees the hook list
//!    its listener is linked into);
//! 2. the registry listener, then the registry proxy;
//! 3. the core listener, then the core (`pw_core_disconnect` destroys any
//!    proxy still alive, so all of ours must be gone by then);
//! 4. the context.
//!
//! On shutdown `run` then drops the attached command receiver (a loop
//! source) and the main loop. Only after that does the thread drop its
//! liveness token and the exit signal of [`BackendHandle::shutdown`], and last
//! the event sender, so an app that sees the event channel close knows no
//! PipeWire object is left.

mod conn;
mod metadata;
pub mod params;
mod registry;

use std::cell::Cell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

use pipewire::main_loop::MainLoopRc;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::{
    AudioBackend, BackendEvent, BackendGone, BackendHandle, BackendThread, Command,
    DisconnectReason, StoreEvent,
};
use crate::log;
use conn::Session;

/// Name of the backend thread.
const THREAD_NAME: &str = "vol-pw";

/// Starts the backend on the default socket (`$PIPEWIRE_REMOTE`, else
/// `pipewire-0` in the runtime directory).
pub fn spawn_pipewire() -> io::Result<BackendHandle> {
    spawn(None)
}

/// Starts the backend on the socket at `remote`. `$PIPEWIRE_REMOTE`, when
/// set, still takes precedence (libpipewire behavior), so callers that must
/// not reach another daemon check that it is unset.
pub fn spawn_pipewire_at(remote: PathBuf) -> io::Result<BackendHandle> {
    spawn(Some(remote))
}

fn spawn(remote: Option<PathBuf>) -> io::Result<BackendHandle> {
    let (events_tx, events) = unbounded_channel();
    let (commands_tx, commands_rx) = pipewire::channel::channel();
    let (exited_tx, exited) = std::sync::mpsc::channel::<()>();
    let alive = Arc::new(());
    let commands = PwCommands {
        sender: commands_tx,
        alive: Arc::downgrade(&alive),
    };
    let handle = spawn_named(move || {
        run(remote, commands_rx, &events_tx);
        drop(alive);
        drop(exited_tx);
        drop(events_tx);
    })?;
    Ok(BackendHandle {
        commands: Box::new(commands),
        events,
        thread: Some(BackendThread { handle, exited }),
    })
}

/// Spawns `f` on a thread named [`THREAD_NAME`].
fn spawn_named<F, T>(f: F) -> io::Result<JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name(THREAD_NAME.to_owned())
        .spawn(f)
}

/// Command side of the backend, held by the app.
struct PwCommands {
    sender: pipewire::channel::Sender<Command>,
    /// Dead once the thread has finished; the channel itself accepts
    /// messages even without a receiver.
    alive: Weak<()>,
}

impl AudioBackend for PwCommands {
    fn send(&self, cmd: Command) -> Result<(), BackendGone> {
        if self.alive.strong_count() == 0 {
            return Err(BackendGone);
        }
        self.sender.send(cmd).map_err(|_| BackendGone)
    }
}

/// Body of `vol-pw`: connects once and runs the main loop until
/// [`Command::Shutdown`]. A failed or lost connection is reported as
/// `Disconnected` and the loop keeps serving commands.
fn run(
    remote: Option<PathBuf>,
    commands: pipewire::channel::Receiver<Command>,
    events: &UnboundedSender<BackendEvent>,
) {
    pipewire::init();
    let main_loop = match MainLoopRc::new(None) {
        Ok(main_loop) => main_loop,
        Err(err) => {
            log::error(format_args!("vol-pw: cannot create main loop: {err}"));
            return;
        }
    };
    let out = Outbox(events.clone());
    let shutdown = Rc::new(Cell::new(false));
    let commands = commands.attach(main_loop.loop_(), {
        let shutdown = Rc::clone(&shutdown);
        let main_loop = main_loop.downgrade();
        move |cmd| match cmd {
            Command::Shutdown => {
                shutdown.set(true);
                if let Some(main_loop) = main_loop.upgrade() {
                    main_loop.quit();
                }
            }
            other => log::debug(format_args!("vol-pw: command not supported: {other:?}")),
        }
    });
    let mut session = match Session::connect(&main_loop, remote.as_deref(), out.clone()) {
        Ok(session) => Some(session),
        Err(reason) => {
            out.send(BackendEvent::Disconnected {
                reason,
                retry_in: None,
                attempt: 1,
            });
            None
        }
    };
    loop {
        main_loop.run();
        if shutdown.get() {
            break;
        }
        // Otherwise a fatal core error quit the loop: tear the session down
        // here, outside its callbacks.
        if let Some(lost) = session.take() {
            let reason = lost.lost().unwrap_or(DisconnectReason::ConnectionLost);
            drop(lost);
            log::info(format_args!("vol-pw: connection lost ({reason:?})"));
            out.send(BackendEvent::Disconnected {
                reason,
                retry_in: None,
                attempt: 0,
            });
        }
    }
    log::info(format_args!("vol-pw: shutting down"));
    drop(session);
    drop(commands);
    drop(main_loop);
}

/// Where the thread's events go.
#[derive(Clone)]
struct Outbox(UnboundedSender<BackendEvent>);

impl Outbox {
    /// Delivers one registry/param change. Until the 16 ms flush timer exists
    /// (M1-T13) every change is its own `Batch`; this is the single place
    /// where coalescing will hook in.
    fn store(&self, event: StoreEvent) {
        self.send(BackendEvent::Batch(vec![event]));
    }

    /// Delivers `event`; dropped if the app is gone (it is exiting).
    fn send(&self, event: BackendEvent) {
        let _ = self.0.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_name_is_vol_pw() {
        let name = spawn_named(|| std::thread::current().name().map(str::to_owned))
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(name.as_deref(), Some("vol-pw"));
    }
}
