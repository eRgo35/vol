//! Helpers for live tests against a **private** PipeWire daemon (VOL-QA-007).
//!
//! Nothing here runs unless `VOL_TEST_PIPEWIRE=1`, and every process it
//! starts gets an explicit environment (`env_clear` + [`PrivateDaemon::env`]),
//! so the user's audio session is never reached. See `tests/live.rs` for the
//! procedure and the verified environment recipe.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// `sockaddr_un.sun_path` is 108 bytes including the terminator; the longest
/// socket the daemon creates is `<runtime>/pipewire-0-manager`.
const MAX_SOCKET_PATH: usize = 107;

/// WirePlumber drop-in: no ALSA/Bluetooth/camera monitors, so the private
/// session never opens or reserves the machine's real devices.
const WIREPLUMBER_DROP_IN: &str = "\
wireplumber.profiles = {
  main = {
    hardware.audio = disabled
    hardware.bluetooth = disabled
    hardware.video-capture = disabled
  }
}
";

/// True when live tests are enabled (`VOL_TEST_PIPEWIRE=1`); otherwise
/// prints a skip note and returns false. Call first in every live test.
pub fn require_live() -> bool {
    if std::env::var("VOL_TEST_PIPEWIRE").as_deref() == Ok("1") {
        return true;
    }
    let _ = writeln!(
        std::io::stderr(),
        "skipped: set VOL_TEST_PIPEWIRE=1 (or run `just test-live`)"
    );
    false
}

/// A `pipewire` (and optionally `wireplumber`) instance in a fresh temporary
/// runtime directory. Killed and removed on drop.
pub struct PrivateDaemon {
    root: PathBuf,
    pipewire: Child,
    wireplumber: Option<Child>,
}

impl PrivateDaemon {
    /// Starts `pipewire`, waits for its socket, and verifies with
    /// `pw-cli info 0` that the socket is served by the process just spawned.
    /// With `with_wireplumber`, also starts `wireplumber` if it is installed.
    pub fn start(with_wireplumber: bool) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("vol-pw-{}-{n}", std::process::id()));
        let socket_len = root.join("pipewire-0-manager").as_os_str().len();
        assert!(
            socket_len <= MAX_SOCKET_PATH,
            "{} is too long for a PipeWire socket; set TMPDIR to a shorter directory",
            root.display()
        );
        let _ = fs::remove_dir_all(&root);
        let drop_in_dir = root.join("config/wireplumber/wireplumber.conf.d");
        fs::create_dir_all(&drop_in_dir).unwrap();
        fs::create_dir_all(root.join("state")).unwrap();
        fs::create_dir_all(root.join("home")).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(drop_in_dir.join("90-vol-test.conf"), WIREPLUMBER_DROP_IN).unwrap();

        let pipewire = spawn_logged(&root, "pipewire");
        let mut daemon = Self {
            root,
            pipewire,
            wireplumber: None,
        };
        daemon.wait_for_socket();
        daemon.verify_private();
        if with_wireplumber {
            daemon.wireplumber = daemon.spawn_wireplumber();
        }
        daemon
    }

    /// The complete environment for a process that must talk to this daemon
    /// (and only to it). Apply it after `env_clear()`; see [`Self::command`].
    pub fn env(&self) -> Vec<(&'static str, OsString)> {
        private_env(&self.root)
    }

    /// Absolute path of this daemon's socket, for in-process clients that
    /// connect to it explicitly.
    pub fn socket(&self) -> PathBuf {
        self.root.join("pipewire-0")
    }

    /// `program` with a cleared environment plus [`Self::env`].
    pub fn command(&self, program: &str) -> Command {
        let mut cmd = Command::new(program);
        cmd.env_clear().envs(self.env());
        cmd
    }

    /// True when `wireplumber` was requested, found, and is still running.
    pub fn has_wireplumber(&mut self) -> bool {
        self.wireplumber
            .as_mut()
            .is_some_and(|wp| matches!(wp.try_wait(), Ok(None)))
    }

    /// Kills the private `pipewire` (our own child, nothing else), as if the
    /// daemon crashed. Its socket file is left behind, as after a real crash.
    pub fn kill_pipewire(&mut self) {
        let _ = self.pipewire.kill();
        let _ = self.pipewire.wait();
    }

    /// Starts a new private `pipewire` on the same socket path (killing the
    /// current one first if it still runs), waits for its socket and checks
    /// that it is served by the new child.
    pub fn restart_pipewire(&mut self) {
        self.kill_pipewire();
        for stale in ["pipewire-0", "pipewire-0-manager"] {
            let _ = fs::remove_file(self.root.join(stale));
        }
        self.pipewire = spawn_logged(&self.root, "pipewire");
        self.wait_for_socket();
        self.verify_private();
    }

    /// Kills the private `wireplumber` and starts a new one with the same
    /// environment and drop-in (a session-manager restart while the daemon
    /// stays up). Returns false if no WirePlumber was running.
    pub fn restart_wireplumber(&mut self) -> bool {
        let Some(mut old) = self.wireplumber.take() else {
            return false;
        };
        let _ = old.kill();
        let _ = old.wait();
        self.wireplumber = Some(spawn_logged(&self.root, "wireplumber"));
        true
    }

    /// Creates a stereo `support.null-audio-sink` named `name` that outlives
    /// the `pw-cli` process (`object.linger`).
    pub fn create_null_sink(&self, name: &str) {
        let props = format!(
            "{{ factory.name=support.null-audio-sink node.name={name} \
             media.class=Audio/Sink object.linger=true audio.position=[FL FR] }}"
        );
        let out = self.run("pw-cli", &["create-node", "adapter", &props]);
        assert!(
            !out.to_lowercase().contains("error"),
            "pw-cli create-node failed: {out}"
        );
    }

    /// `pw-dump` of the private session.
    pub fn pw_dump(&self) -> String {
        self.run("pw-dump", &[])
    }

    /// Runs a PipeWire CLI against this daemon; panics on a non-zero exit.
    fn run(&self, program: &str, args: &[&str]) -> String {
        let out = self
            .command(program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("cannot run {program}: {e}"));
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{program} {args:?} failed: {text}");
        text
    }

    fn wait_for_socket(&mut self) {
        let socket = self.root.join("pipewire-0");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !socket.exists() {
            let exited = self.pipewire.try_wait().unwrap();
            assert!(
                exited.is_none() && Instant::now() < deadline,
                "private pipewire did not create {} ({exited:?}); log:\n{}",
                socket.display(),
                fs::read_to_string(self.root.join("pipewire.log")).unwrap_or_default()
            );
            sleep(Duration::from_millis(20));
        }
    }

    /// The core object's `application.process.id` must be our child's pid.
    fn verify_private(&self) {
        let info = self.run("pw-cli", &["info", "0"]);
        let expected = format!("application.process.id = \"{}\"", self.pipewire.id());
        assert!(
            info.contains(&expected),
            "socket is not served by the private daemon:\n{info}"
        );
    }

    fn spawn_wireplumber(&self) -> Option<Child> {
        let found = self
            .command("wireplumber")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        found.then(|| spawn_logged(&self.root, "wireplumber"))
    }
}

impl Drop for PrivateDaemon {
    fn drop(&mut self) {
        for child in self.wireplumber.iter_mut().chain([&mut self.pipewire]) {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Spawns a daemon with the private environment of `root`, logging to
/// `<root>/<program>.log`.
fn spawn_logged(root: &Path, program: &str) -> Child {
    let log = fs::File::create(root.join(format!("{program}.log"))).unwrap();
    Command::new(program)
        .env_clear()
        .envs(private_env(root))
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap_or_else(|e| panic!("cannot start {program}: {e}"))
}

/// Environment that confines a process to the private daemon in `root`.
/// `env_clear()` drops any inherited `PIPEWIRE_REMOTE` (it would take
/// precedence over everything else) and the user's D-Bus sessions.
fn private_env(root: &Path) -> Vec<(&'static str, OsString)> {
    let path = |p: &Path| p.as_os_str().to_owned();
    let no_bus = OsString::from(format!("unix:path={}", root.join("no-bus").display()));
    vec![
        ("PATH", std::env::var_os("PATH").unwrap_or_default()),
        ("XDG_RUNTIME_DIR", path(root)),
        ("PIPEWIRE_RUNTIME_DIR", path(root)),
        ("XDG_CONFIG_HOME", path(&root.join("config"))),
        ("XDG_STATE_HOME", path(&root.join("state"))),
        ("HOME", path(&root.join("home"))),
        ("DBUS_SESSION_BUS_ADDRESS", no_bus.clone()),
        ("DBUS_SYSTEM_BUS_ADDRESS", no_bus),
    ]
}
