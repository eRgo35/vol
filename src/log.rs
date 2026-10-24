//! File logger (VOL-ARCH-054).
//!
//! Enabled only when `VOL_LOG` is one of `error`, `warn`, `info`, `debug`.
//! Lines go to `$XDG_STATE_HOME/vol/vol.log` (fallback
//! `~/.local/state/vol/vol.log`), never to stdout/stderr: those belong to the
//! terminal. The path and level logic are pure functions of the environment
//! values so they can be tested without touching the process environment.

use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

/// Severity of a log line; a logger at level `L` keeps every line `<= L`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Something failed.
    Error,
    /// Something unexpected that vol recovered from.
    Warn,
    /// Lifecycle events (connect, disconnect, shutdown).
    Info,
    /// Verbose diagnostics.
    Debug,
}

impl Level {
    /// Parses a `VOL_LOG` value; `None` (logging disabled) for anything but
    /// the four level names.
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value? {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
        }
    }
}

/// Log file path from `$XDG_STATE_HOME` and `$HOME`. An empty or relative
/// `XDG_STATE_HOME` is ignored, as the XDG base directory spec requires.
pub fn log_path(xdg_state_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let state = match xdg_state_home.map(Path::new) {
        Some(dir) if dir.is_absolute() => dir.to_path_buf(),
        _ => Path::new(home?).join(".local").join("state"),
    };
    Some(state.join("vol").join("vol.log"))
}

/// An open log file and the most verbose level it records.
#[derive(Debug)]
pub struct Logger {
    level: Level,
    file: Mutex<File>,
}

impl Logger {
    /// Opens `path` for appending, creating its parent directories.
    pub fn open(path: &Path, level: Level) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            level,
            file: Mutex::new(file),
        })
    }

    /// Builds the logger described by the environment values: `None` when
    /// `VOL_LOG` is unset or not a level name, or when no path can be formed.
    pub fn from_env(
        vol_log: Option<&str>,
        xdg_state_home: Option<&OsStr>,
        home: Option<&OsStr>,
    ) -> io::Result<Option<Self>> {
        let Some(level) = Level::parse(vol_log) else {
            return Ok(None);
        };
        let Some(path) = log_path(xdg_state_home, home) else {
            return Ok(None);
        };
        Self::open(&path, level).map(Some)
    }

    /// Appends one line if `level` is enabled. Write errors are ignored:
    /// there is nowhere else to report them.
    pub fn log(&self, level: Level, args: fmt::Arguments<'_>) {
        if level > self.level {
            return;
        }
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let line = format!(
            "{}.{:03} {:<5} {args}\n",
            since_epoch.as_secs(),
            since_epoch.subsec_millis(),
            level.label()
        );
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = file.write_all(line.as_bytes());
    }
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Installs the process-wide logger from `VOL_LOG`, `XDG_STATE_HOME` and
/// `HOME`. Does nothing when logging is disabled or already installed.
pub fn init() -> io::Result<()> {
    let vol_log = std::env::var("VOL_LOG").ok();
    let xdg_state_home = std::env::var_os("XDG_STATE_HOME");
    let home = std::env::var_os("HOME");
    if let Some(logger) = Logger::from_env(
        vol_log.as_deref(),
        xdg_state_home.as_deref(),
        home.as_deref(),
    )? {
        let _ = LOGGER.set(logger);
    }
    Ok(())
}

/// Logs through the process-wide logger, if one is installed.
pub fn log(level: Level, args: fmt::Arguments<'_>) {
    if let Some(logger) = LOGGER.get() {
        logger.log(level, args);
    }
}

/// Logs at [`Level::Error`].
pub fn error(args: fmt::Arguments<'_>) {
    log(Level::Error, args);
}

/// Logs at [`Level::Warn`].
pub fn warn(args: fmt::Arguments<'_>) {
    log(Level::Warn, args);
}

/// Logs at [`Level::Info`].
pub fn info(args: fmt::Arguments<'_>) {
    log(Level::Info, args);
}

/// Logs at [`Level::Debug`].
pub fn debug(args: fmt::Arguments<'_>) {
    log(Level::Debug, args);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("vol-log-test-{}-{n}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn disabled_without_env() {
        let tmp = TempDir::new("disabled");
        for value in [None, Some(""), Some("trace"), Some("DEBUG")] {
            let logger = Logger::from_env(value, Some(tmp.0.as_os_str()), None).unwrap();
            assert!(logger.is_none(), "VOL_LOG={value:?}");
        }
        assert!(!tmp.0.exists(), "nothing is created when disabled");
    }

    #[test]
    fn level_filtering() {
        let tmp = TempDir::new("levels");
        let path = tmp.0.join("vol.log");
        let logger = Logger::open(&path, Level::Warn).unwrap();
        logger.log(Level::Error, format_args!("e1"));
        logger.log(Level::Warn, format_args!("w1"));
        logger.log(Level::Info, format_args!("i1"));
        logger.log(Level::Debug, format_args!("d1"));
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].ends_with("ERROR e1"), "{text}");
        assert!(lines[1].ends_with("WARN  w1"), "{text}");
        assert_eq!(Level::parse(Some("info")), Some(Level::Info));
        assert_eq!(Level::parse(Some("error")), Some(Level::Error));
        assert!(Level::Error < Level::Warn && Level::Info < Level::Debug);
    }

    #[test]
    fn path_prefers_xdg_state_home() {
        let path = log_path(Some(OsStr::new("/x/state")), Some(OsStr::new("/home/u")));
        assert_eq!(path, Some(PathBuf::from("/x/state/vol/vol.log")));
    }

    #[test]
    fn path_falls_back_to_home_local_state() {
        let expected = Some(PathBuf::from("/home/u/.local/state/vol/vol.log"));
        let home = Some(OsStr::new("/home/u"));
        assert_eq!(log_path(None, home), expected);
        assert_eq!(log_path(Some(OsStr::new("")), home), expected);
        assert_eq!(log_path(Some(OsStr::new("rel/dir")), home), expected);
        assert_eq!(log_path(None, None), None);
    }

    #[test]
    fn creates_directory_and_appends() {
        let tmp = TempDir::new("append");
        let path = log_path(Some(tmp.0.as_os_str()), None).unwrap();
        let first = Logger::from_env(Some("info"), Some(tmp.0.as_os_str()), None)
            .unwrap()
            .unwrap();
        first.log(Level::Info, format_args!("one"));
        drop(first);
        let second = Logger::open(&path, Level::Info).unwrap();
        second.log(Level::Info, format_args!("two {}", 2));
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].ends_with("INFO  one"), "{text}");
        assert!(lines[1].ends_with("INFO  two 2"), "{text}");
    }
}
