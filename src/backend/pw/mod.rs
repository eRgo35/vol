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
//! state, the registry, the core), so there are no reference cycles; they
//! borrow the state briefly through `conn::with_state` and never call into
//! PipeWire while it is borrowed (binding, `sync` and dropping proxies all
//! happen outside the borrow).
//!
//! # Teardown order
//!
//! Teardown never happens inside a PipeWire callback: a fatal core error only
//! sets a flag, and `drive` drops the session between two loop iterations.
//! A session drops, in this order (its field order):
//!
//! 1. the state, with every bound node/device and the `default` metadata:
//!    each listener before its proxy (a proxy destroy frees the hook list
//!    its listener is linked into);
//! 2. the registry listener, then the registry proxy;
//! 3. the core listener, then the core (`pw_core_disconnect` destroys any
//!    proxy still alive, so all of ours must be gone by then);
//! 4. the context.
//!
//! On shutdown `drive` drops the session and its loop timers, then `run`
//! drops the attached command receiver (a loop source) and the main loop.
//! Only after that does the thread drop its liveness token and the exit
//! signal of [`BackendHandle::shutdown`], and last the event sender, so an
//! app that sees the event channel close knows no PipeWire object is left.

mod conn;
mod metadata;
pub mod params;
mod reconnect;
mod registry;

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pipewire::loop_::{Timeout, TimerSource};
use pipewire::main_loop::MainLoopRc;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::coalesce::Batch;
use super::{
    AudioBackend, BackendEvent, BackendGone, BackendHandle, BackendThread, Command,
    DisconnectReason, StoreEvent,
};
use crate::log;
use conn::Session;
use reconnect::{Reconnect, RestartBudget};

/// Name of the backend thread.
const THREAD_NAME: &str = "vol-pw";

/// Delay of the one-shot flush timer, armed by the first store event that
/// enters an empty batch (VOL-ARCH-030).
const FLUSH_DELAY: Duration = Duration::from_millis(16);

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

/// State the command callback shares with the connection driver.
#[derive(Default)]
struct Flags {
    /// `Command::Shutdown` arrived.
    shutdown: Cell<bool>,
    /// A session exists; without one, commands are answered as failed.
    live: Cell<bool>,
    /// The restart budget is spent; the backend stays disconnected.
    crashed: Cell<bool>,
}

/// Body of `vol-pw`: creates the main loop, attaches the command channel and
/// runs [`drive`] until [`Command::Shutdown`]. A panic in `drive` is caught
/// here, reported as `Disconnected{ConnectionLost}` and `drive` restarted
/// after the backoff, at most 5 times in 60 s; after that the thread only
/// answers commands until shutdown (VOL-ARCH-052).
///
/// The guard sits inside `run`, around everything but the loop and the
/// command receiver, because `attach` consumes the receiver and it must
/// survive a restart. Panics inside PipeWire callbacks cannot be caught:
/// they would unwind into C, which aborts the process.
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
    let out = Outbox::new(events.clone());
    let flags = Rc::new(Flags::default());
    let commands = commands.attach(main_loop.loop_(), {
        let flags = Rc::clone(&flags);
        let out = out.clone();
        move |cmd| {
            if cmd == Command::Shutdown {
                flags.shutdown.set(true);
            } else if flags.live.get() {
                log::debug(format_args!("vol-pw: command not supported: {cmd:?}"));
            } else if let Some(reply) =
                reconnect::reply_while_disconnected(&cmd, flags.crashed.get())
            {
                out.send(reply);
            }
        }
    });
    let mut reconnect = Reconnect::default();
    let mut budget = RestartBudget::default();
    let mut retry_in = None;
    loop {
        // AssertUnwindSafe: after a panic only plain counters and flags are
        // reused, and each of them is valid after any partial update.
        let driven = panic::catch_unwind(AssertUnwindSafe(|| {
            drive(
                &main_loop,
                remote.as_deref(),
                &out,
                &flags,
                &mut reconnect,
                retry_in,
            );
        }));
        let Err(panic) = driven else { break };
        flags.live.set(false);
        log::error(format_args!(
            "vol-pw: backend panicked: {}",
            panic_message(panic.as_ref())
        ));
        let (attempt, delay) = reconnect.failed();
        let allowed = budget.allow(Instant::now());
        out.send(BackendEvent::Disconnected {
            reason: DisconnectReason::ConnectionLost,
            retry_in: allowed.then_some(delay),
            attempt,
        });
        if !allowed {
            log::error(format_args!(
                "vol-pw: too many restarts; staying disconnected"
            ));
            flags.crashed.set(true);
            while !flags.shutdown.get() {
                main_loop.loop_().iterate(Timeout::Infinite);
            }
            break;
        }
        retry_in = Some(delay);
    }
    log::info(format_args!("vol-pw: shutting down"));
    drop(commands);
    drop(main_loop);
}

/// Connects (now, or after `retry_in`), runs the loop and reconnects with
/// backoff whenever the connection fails or is lost (VOL-ARCH-040…043),
/// until shutdown.
///
/// The loop is iterated here instead of by `MainLoop::run`, so that between
/// iterations, outside every callback, this function can arm the flush timer
/// for events the callbacks queued (VOL-ARCH-030), tear down a lost session
/// (VOL-ARCH-042) and start a due retry. The retry wait is a loop timer, so
/// commands are served throughout (VOL-ARCH-043).
fn drive(
    main_loop: &MainLoopRc,
    remote: Option<&Path>,
    out: &Outbox,
    flags: &Flags,
    reconnect: &mut Reconnect,
    retry_in: Option<Duration>,
) {
    let loop_ = main_loop.loop_();
    let flush_timer = loop_.add_timer({
        let out = out.clone();
        move |_| out.flush()
    });
    let retry_due = Rc::new(Cell::new(retry_in.is_none()));
    let retry_timer = loop_.add_timer({
        let retry_due = Rc::clone(&retry_due);
        move |_| retry_due.set(true)
    });
    if let Some(delay) = retry_in {
        arm(&retry_timer, delay);
    }
    let mut session: Option<Session> = None;
    while !flags.shutdown.get() {
        if retry_due.replace(false) {
            match Session::connect(main_loop, remote, out.clone()) {
                Ok(connected) => {
                    session = Some(connected);
                    flags.live.set(true);
                }
                Err(reason) => {
                    let (attempt, delay) = reconnect.failed();
                    out.send(BackendEvent::Disconnected {
                        reason,
                        retry_in: Some(delay),
                        attempt,
                    });
                    arm(&retry_timer, delay);
                }
            }
        }
        loop_.iterate(Timeout::Infinite);
        if out.take_arm_request() {
            arm(&flush_timer, FLUSH_DELAY);
        }
        if session.as_ref().is_some_and(Session::is_connected) {
            reconnect.connected();
        }
        // A fatal core error only flagged the session: tear it down here,
        // outside its callbacks.
        if let Some(reason) = session.as_ref().and_then(Session::lost) {
            drop(session.take());
            flags.live.set(false);
            log::info(format_args!("vol-pw: connection lost ({reason:?})"));
            let (attempt, delay) = reconnect.failed();
            out.send(BackendEvent::Disconnected {
                reason,
                retry_in: Some(delay),
                attempt,
            });
            arm(&retry_timer, delay);
        }
    }
    drop(session);
    flags.live.set(false);
}

/// Arms the one-shot `timer` to fire after `delay`.
fn arm(timer: &TimerSource<'_>, delay: Duration) {
    if let Err(err) = timer.update_timer(Some(delay), None).into_result() {
        log::error(format_args!("vol-pw: cannot arm timer: {err}"));
    }
}

/// Text of a caught panic payload.
fn panic_message(panic: &(dyn Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

/// Where the thread's events go. Store events are queued in a coalescing
/// [`Batch`] and leave as one `BackendEvent::Batch` when the flush timer
/// fires or the batch is full (VOL-ARCH-030); any other event first flushes
/// the queue, so the app receives events in emission order.
#[derive(Clone)]
struct Outbox(Rc<Queue>);

struct Queue {
    events: UnboundedSender<BackendEvent>,
    batch: RefCell<Batch>,
    /// Set when an event entered an empty batch: the flush timer must be
    /// (re)armed.
    arm: Cell<bool>,
}

impl Outbox {
    fn new(events: UnboundedSender<BackendEvent>) -> Self {
        Self(Rc::new(Queue {
            events,
            batch: RefCell::new(Batch::new()),
            arm: Cell::new(false),
        }))
    }

    /// Queues one registry/param change; flushes at once when the batch is
    /// full.
    fn store(&self, event: StoreEvent) {
        let mut batch = self.0.batch.borrow_mut();
        if batch.is_empty() {
            self.0.arm.set(true);
        }
        batch.push(event);
        if batch.is_full() {
            let events = batch.take();
            drop(batch);
            self.deliver(BackendEvent::Batch(events));
        }
    }

    /// True once after an event entered an empty batch. The driver arms the
    /// flush timer in response, right after the loop iteration that queued
    /// the event.
    fn take_arm_request(&self) -> bool {
        self.0.arm.replace(false)
    }

    /// Sends the queued store events, if any, as one `Batch`.
    fn flush(&self) {
        let events = self.0.batch.borrow_mut().take();
        if !events.is_empty() {
            self.deliver(BackendEvent::Batch(events));
        }
    }

    /// Delivers `event` after the queued store events.
    fn send(&self, event: BackendEvent) {
        self.flush();
        self.deliver(event);
    }

    /// Dropped if the app is gone (it is exiting).
    fn deliver(&self, event: BackendEvent) {
        let _ = self.0.events.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::coalesce::FLUSH_THRESHOLD;
    use crate::model::ObjectId;
    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::sync::mpsc::error::TryRecvError;

    fn outbox() -> (Outbox, UnboundedReceiver<BackendEvent>) {
        let (tx, rx) = unbounded_channel();
        (Outbox::new(tx), rx)
    }

    fn removed(id: u32) -> StoreEvent {
        StoreEvent::NodeRemoved { id: ObjectId(id) }
    }

    #[test]
    fn thread_name_is_vol_pw() {
        let name = spawn_named(|| std::thread::current().name().map(str::to_owned))
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(name.as_deref(), Some("vol-pw"));
    }

    #[test]
    fn outbox_queues_store_events_until_flush() {
        let (out, mut rx) = outbox();
        out.store(removed(1));
        out.store(removed(2));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        out.flush();
        assert_eq!(
            rx.try_recv(),
            Ok(BackendEvent::Batch(vec![removed(1), removed(2)]))
        );
        out.flush();
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn outbox_requests_timer_only_for_first_event_into_empty_batch() {
        let (out, _rx) = outbox();
        assert!(!out.take_arm_request());
        out.store(removed(1));
        out.store(removed(2));
        assert!(out.take_arm_request());
        assert!(!out.take_arm_request());
        out.flush();
        out.store(removed(3));
        assert!(out.take_arm_request());
    }

    #[test]
    fn outbox_flushes_immediately_at_threshold() {
        let (out, mut rx) = outbox();
        let ids = 0..u32::try_from(FLUSH_THRESHOLD).unwrap();
        for id in ids.clone() {
            out.store(removed(id));
        }
        assert_eq!(
            rx.try_recv(),
            Ok(BackendEvent::Batch(ids.map(removed).collect()))
        );
    }

    #[test]
    fn outbox_send_flushes_queued_store_events_first() {
        let (out, mut rx) = outbox();
        out.store(removed(1));
        let lost = BackendEvent::Disconnected {
            reason: DisconnectReason::ConnectionLost,
            retry_in: None,
            attempt: 1,
        };
        out.send(lost.clone());
        assert_eq!(rx.try_recv(), Ok(BackendEvent::Batch(vec![removed(1)])));
        assert_eq!(rx.try_recv(), Ok(lost));
    }
}
