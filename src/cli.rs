//! Command-line flags, parsed by hand (VOL-ARCH-005).

use std::fmt;

/// Usage text printed by `--help` and after a parse error.
pub const USAGE: &str = "\
Usage: vol [OPTIONS]

A terminal volume mixer for PipeWire.

Options:
      --mouse      Enable mouse support
      --no-color   Disable colors (also when NO_COLOR is set and non-empty)
      --no-meters  Disable peak meters (also when VOL_NO_METERS=1)
  -h, --help       Print this help and exit
  -V, --version    Print the version and exit
";

/// Version line printed by `--version`.
pub const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

/// Parsed command-line flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cli {
    /// `--mouse`: enable mouse capture.
    pub mouse: bool,
    /// `--no-color`: monochrome theme.
    pub no_color: bool,
    /// `--no-meters`: open no meter streams.
    pub no_meters: bool,
    /// `-h`, `--help`: print usage and exit.
    pub help: bool,
    /// `-V`, `--version`: print the version and exit.
    pub version: bool,
}

/// A command-line argument that is not a known flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    /// The offending argument, verbatim.
    pub arg: String,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown argument '{}'", self.arg)
    }
}

impl std::error::Error for CliError {}

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Cli, CliError> {
    let mut cli = Cli::default();
    for arg in args {
        let flag = match arg.as_str() {
            "--mouse" => &mut cli.mouse,
            "--no-color" => &mut cli.no_color,
            "--no-meters" => &mut cli.no_meters,
            "-h" | "--help" => &mut cli.help,
            "-V" | "--version" => &mut cli.version,
            _ => return Err(CliError { arg }),
        };
        *flag = true;
    }
    Ok(cli)
}

/// Whether meters are disabled: `--no-meters` or `VOL_NO_METERS=1`.
///
/// `env` is the value of `VOL_NO_METERS`, if set.
pub fn no_meters(cli: &Cli, env: Option<&str>) -> bool {
    cli.no_meters || env == Some("1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Cli, CliError> {
        parse(args.iter().map(|a| (*a).to_owned()))
    }

    fn err(arg: &str) -> Result<Cli, CliError> {
        Err(CliError {
            arg: arg.to_owned(),
        })
    }

    #[test]
    fn no_args_gives_defaults() {
        assert_eq!(p(&[]), Ok(Cli::default()));
    }

    #[test]
    fn help_short_and_long() {
        for a in ["-h", "--help"] {
            let cli = p(&[a]).unwrap();
            assert!(cli.help, "{a}");
            assert!(!cli.version);
        }
    }

    #[test]
    fn version_short_and_long() {
        for a in ["-V", "--version"] {
            let cli = p(&[a]).unwrap();
            assert!(cli.version, "{a}");
            assert!(!cli.help);
        }
        assert!(VERSION.starts_with("vol "));
    }

    #[test]
    fn mouse_flag() {
        let expected = Cli {
            mouse: true,
            ..Cli::default()
        };
        assert_eq!(p(&["--mouse"]), Ok(expected));
    }

    #[test]
    fn no_color_flag() {
        let expected = Cli {
            no_color: true,
            ..Cli::default()
        };
        assert_eq!(p(&["--no-color"]), Ok(expected));
    }

    #[test]
    fn no_meters_flag_and_env() {
        let flag = p(&["--no-meters"]).unwrap();
        assert!(flag.no_meters);
        assert!(no_meters(&flag, None));

        let plain = Cli::default();
        assert!(!no_meters(&plain, None));
        assert!(no_meters(&plain, Some("1")));
        assert!(!no_meters(&plain, Some("0")));
        assert!(!no_meters(&plain, Some("")));
    }

    #[test]
    fn unknown_flag_is_error() {
        for a in ["--bogus", "-x", "-", "--", "positional", "-hV", "--HELP"] {
            assert_eq!(p(&[a]), err(a), "{a}");
        }
        assert_eq!(p(&["--mouse", "--bogus"]), err("--bogus"));
        assert_eq!(
            CliError {
                arg: "--bogus".to_owned()
            }
            .to_string(),
            "unknown argument '--bogus'"
        );
    }

    #[test]
    fn flags_take_no_values() {
        assert_eq!(p(&["--mouse=1"]), err("--mouse=1"));
        assert_eq!(p(&["--no-color=yes"]), err("--no-color=yes"));
        assert_eq!(p(&["--no-meters=true"]), err("--no-meters=true"));
        assert_eq!(p(&["--help=x"]), err("--help=x"));
        assert_eq!(p(&["--mouse", "1"]), err("1"));
        assert_eq!(p(&["--no-color", "yes"]), err("yes"));
    }

    #[test]
    fn usage_mentions_every_flag() {
        for flag in [
            "--mouse",
            "--no-color",
            "--no-meters",
            "-h",
            "--help",
            "-V",
            "--version",
            "NO_COLOR",
            "VOL_NO_METERS",
        ] {
            assert!(USAGE.contains(flag), "{flag}");
        }
    }
}
