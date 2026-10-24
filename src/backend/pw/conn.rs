//! Connection to the daemon: context, core and core listeners (VOL-ARCH-040, 042).
//!
//! A [`Session`] is one connection. It owns the PipeWire objects of that
//! connection and the shared [`State`] the listeners write to; dropping it
//! tears the connection down in the order documented in `pw/mod.rs`.

use std::cell::RefCell;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::rc::{Rc, Weak};

use pipewire::context::ContextRc;
use pipewire::core::{CoreRc, PW_ID_CORE};
use pipewire::main_loop::MainLoopRc;
use pipewire::properties::PropertiesBox;
use pipewire::registry::RegistryRc;

use super::Outbox;
use super::registry::{self, Objects};
use crate::backend::{BackendEvent, DisconnectReason};
use crate::log;

/// Default socket name inside the runtime directory.
const DEFAULT_SOCKET: &str = "pipewire-0";

/// `ENOENT`, `ECONNREFUSED`, `EHOSTDOWN` (and anything unknown) →
/// [`DisconnectReason::NoDaemon`]; `EACCES`, `EPERM` →
/// [`DisconnectReason::PermissionDenied`]; `EPIPE` →
/// [`DisconnectReason::ConnectionLost`]. `errno` is positive.
///
/// Observed on PipeWire 1.6.9: connecting to a socket path that does not
/// exist fails with `EHOSTDOWN`; a daemon dying under a connected client
/// yields core `error(id 0, seq, -EPIPE, "connection error")` (VF-02).
pub(super) fn reason_for_errno(errno: i32) -> DisconnectReason {
    match io::Error::from_raw_os_error(errno).kind() {
        io::ErrorKind::PermissionDenied => DisconnectReason::PermissionDenied,
        io::ErrorKind::BrokenPipe => DisconnectReason::ConnectionLost,
        _ => DisconnectReason::NoDaemon,
    }
}

/// Human-readable socket for `Connected`/logs, resolved like libpipewire:
/// `remote` (`$PIPEWIRE_REMOTE`, else an explicit socket) if absolute, else
/// `remote` or `pipewire-0` inside `$PIPEWIRE_RUNTIME_DIR`, else inside
/// `$XDG_RUNTIME_DIR`; just the name when neither directory is set.
pub(super) fn socket_description(
    remote: Option<&OsStr>,
    pipewire_runtime_dir: Option<&OsStr>,
    xdg_runtime_dir: Option<&OsStr>,
) -> String {
    let name = Path::new(remote.unwrap_or(OsStr::new(DEFAULT_SOCKET)));
    let path = match pipewire_runtime_dir.or(xdg_runtime_dir) {
        Some(dir) if !name.is_absolute() => Path::new(dir).join(name),
        _ => name.to_path_buf(),
    };
    path.display().to_string()
}

/// Everything the listeners of one connection share. Borrowed only briefly,
/// never across a call into PipeWire (see [`with_state`]).
pub(super) struct State {
    pub(super) out: Outbox,
    /// Bound proxies and tracked ids; dropped with the state, before the
    /// registry and the core.
    pub(super) objects: Objects,
    socket: String,
    server_version: String,
    /// Seq of the initial `core.sync`, until its `done` arrives.
    initial_sync: Option<i32>,
    /// Set by a fatal core error; the loop is quit and the session torn down
    /// outside the callback.
    lost: Option<DisconnectReason>,
}

impl State {
    fn on_done(&mut self, seq: i32) {
        if self.initial_sync == Some(seq) {
            self.initial_sync = None;
            log::info(format_args!(
                "vol-pw: connected to {} (PipeWire {})",
                self.socket, self.server_version
            ));
            log::debug(format_args!("vol-pw: snapshot: {}", self.objects.counts()));
            self.out.send(BackendEvent::Connected {
                server_version: self.server_version.clone(),
                socket: self.socket.clone(),
            });
        }
    }
}

/// Runs `f` on the state behind `state`, if it still exists and is not
/// borrowed (a nested borrow would be a bug; it is logged instead of
/// panicking inside a C callback).
pub(super) fn with_state<R>(
    state: &Weak<RefCell<State>>,
    f: impl FnOnce(&mut State) -> R,
) -> Option<R> {
    let state = state.upgrade()?;
    let Ok(mut state) = state.try_borrow_mut() else {
        log::error(format_args!(
            "vol-pw: state already borrowed; event dropped"
        ));
        return None;
    };
    Some(f(&mut state))
}

/// One connection to the daemon. Field order is drop order (`pw/mod.rs`).
pub(super) struct Session {
    state: Rc<RefCell<State>>,
    _registry_listener: pipewire::registry::Listener,
    _registry: RegistryRc,
    _core_listener: pipewire::core::Listener,
    _core: CoreRc,
    _context: ContextRc,
}

impl Session {
    /// Connects to `remote` (or the default socket), registers the core
    /// listeners and requests the initial sync. `Err` is the reason to report
    /// in `Disconnected`.
    pub(super) fn connect(
        main_loop: &MainLoopRc,
        remote: Option<&Path>,
        out: Outbox,
    ) -> Result<Self, DisconnectReason> {
        let env_remote = std::env::var_os("PIPEWIRE_REMOTE");
        let socket = socket_description(
            env_remote.as_deref().or(remote.map(Path::as_os_str)),
            std::env::var_os("PIPEWIRE_RUNTIME_DIR").as_deref(),
            std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        );
        let context = ContextRc::new(main_loop, None).map_err(|err| {
            log::error(format_args!("vol-pw: cannot create context: {err}"));
            DisconnectReason::NoDaemon
        })?;
        let props = remote.map(|remote| {
            let mut props = PropertiesBox::new();
            props.insert("remote.name", remote.as_os_str().as_bytes());
            props
        });
        let core = context.connect_rc(props).map_err(|err| {
            // Read errno before anything else can overwrite it.
            let errno = io::Error::last_os_error().raw_os_error().unwrap_or(0);
            let reason = reason_for_errno(errno);
            log::info(format_args!(
                "vol-pw: cannot connect to {socket}: {err} (errno {errno}) -> {reason:?}"
            ));
            reason
        })?;

        let state = Rc::new(RefCell::new(State {
            out,
            objects: Objects::default(),
            socket,
            server_version: String::new(),
            initial_sync: None,
            lost: None,
        }));
        let core_listener = core
            .add_listener_local()
            .info({
                let state = Rc::downgrade(&state);
                move |info| {
                    with_state(&state, |s| s.server_version = info.version().to_owned());
                }
            })
            .done({
                let state = Rc::downgrade(&state);
                move |_id, seq| {
                    with_state(&state, |s| s.on_done(seq.seq()));
                }
            })
            .error({
                let state = Rc::downgrade(&state);
                let main_loop = main_loop.downgrade();
                move |id, seq, res, message| {
                    log::warn(format_args!(
                        "vol-pw: core error: id {id}, seq {seq}, res {res}, message {message:?}"
                    ));
                    if id != PW_ID_CORE {
                        return;
                    }
                    with_state(&state, |s| s.lost = Some(reason_for_errno(-res)));
                    if let Some(main_loop) = main_loop.upgrade() {
                        main_loop.quit();
                    }
                }
            })
            .register();
        let registry = core.get_registry_rc().map_err(|err| {
            log::warn(format_args!("vol-pw: cannot get registry: {err}"));
            DisconnectReason::ConnectionLost
        })?;
        let registry_listener = registry
            .add_listener_local()
            .global({
                let state = Rc::downgrade(&state);
                let registry = registry.downgrade();
                move |global| {
                    if let Some(registry) = registry.upgrade() {
                        registry::on_global(&state, &registry, global);
                    }
                }
            })
            .global_remove({
                let state = Rc::downgrade(&state);
                move |id| registry::on_global_remove(&state, id)
            })
            .register();
        // Its `done` arrives after the daemon has sent every existing global.
        let seq = core.sync(0).map_err(|err| {
            log::warn(format_args!("vol-pw: initial sync failed: {err}"));
            DisconnectReason::ConnectionLost
        })?;
        state.borrow_mut().initial_sync = Some(seq.seq());
        Ok(Self {
            state,
            _registry_listener: registry_listener,
            _registry: registry,
            _core_listener: core_listener,
            _core: core,
            _context: context,
        })
    }

    /// Why the connection broke, once a fatal core error was reported.
    pub(super) fn lost(&self) -> Option<DisconnectReason> {
        self.state.borrow().lost.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Linux errno values.
    const EPERM: i32 = 1;
    const ENOENT: i32 = 2;
    const EACCES: i32 = 13;
    const EPIPE: i32 = 32;
    const ECONNREFUSED: i32 = 111;
    const EHOSTDOWN: i32 = 112;

    #[test]
    fn errno_maps_to_disconnect_reason() {
        assert_eq!(reason_for_errno(ENOENT), DisconnectReason::NoDaemon);
        assert_eq!(reason_for_errno(ECONNREFUSED), DisconnectReason::NoDaemon);
        assert_eq!(reason_for_errno(EHOSTDOWN), DisconnectReason::NoDaemon);
        assert_eq!(reason_for_errno(EACCES), DisconnectReason::PermissionDenied);
        assert_eq!(reason_for_errno(EPERM), DisconnectReason::PermissionDenied);
        assert_eq!(reason_for_errno(EPIPE), DisconnectReason::ConnectionLost);
        assert_eq!(reason_for_errno(0), DisconnectReason::NoDaemon);
    }

    #[test]
    fn socket_description_uses_remote_then_runtime_dir() {
        let os = |s| Some(OsStr::new(s));
        assert_eq!(
            socket_description(os("/tmp/x/pipewire-0"), os("/pw"), os("/run/user/1")),
            "/tmp/x/pipewire-0"
        );
        assert_eq!(
            socket_description(os("other"), None, os("/run/user/1")),
            "/run/user/1/other"
        );
        assert_eq!(
            socket_description(None, None, os("/run/user/1")),
            "/run/user/1/pipewire-0"
        );
        assert_eq!(
            socket_description(None, os("/pw"), os("/run/user/1")),
            "/pw/pipewire-0"
        );
        assert_eq!(socket_description(None, None, None), "pipewire-0");
    }
}
