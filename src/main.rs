use std::io::Write;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use color_eyre::eyre::WrapErr;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use vol::app::{App, Options};
use vol::backend::BackendEvent;
use vol::backend::pw::spawn_pipewire;
use vol::cli;
use vol::event::EventHandler;
use vol::theme;

/// How long quitting waits for the backend thread (VOL-ARCH-013).
const SHUTDOWN_BUDGET: Duration = Duration::from_secs(1);

/// Startup order per VOL-ARCH-060: CLI → color-eyre → file logger → backend →
/// terminal → app and events → run → backend shutdown → restore on every
/// exit path.
#[tokio::main]
async fn main() -> color_eyre::Result<ExitCode> {
    let args = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned());
    let cli = match cli::parse(args) {
        Ok(cli) => cli,
        Err(err) => {
            write!(std::io::stderr(), "vol: {err}\n\n{}", cli::USAGE)?;
            return Ok(ExitCode::from(2));
        }
    };
    if cli.help {
        write!(std::io::stdout(), "{}", cli::USAGE)?;
        return Ok(ExitCode::SUCCESS);
    }
    if cli.version {
        writeln!(std::io::stdout(), "{}", cli::VERSION)?;
        return Ok(ExitCode::SUCCESS);
    }
    color_eyre::install()?;
    // Still before raw mode, so stderr is usable for this one notice.
    if let Err(err) = vol::log::init() {
        writeln!(std::io::stderr(), "vol: file logging disabled: {err}")?;
    }
    let no_color = std::env::var_os("NO_COLOR").map(|v| v.to_string_lossy().into_owned());
    let options = Options {
        color: theme::color_enabled(cli.no_color, no_color.as_deref()),
        mouse: cli.mouse,
    };
    // Only fails if the backend thread cannot be spawned; the terminal is
    // untouched yet, so color-eyre reports it on stderr. A missing daemon is
    // not an error here: the backend reports it as `Disconnected`.
    let mut backend = spawn_pipewire()
        .inspect_err(|err| vol::log::error(format_args!("cannot start the backend: {err}")))
        .wrap_err("cannot start the PipeWire backend thread")?;
    // The event receiver goes to the event sources; the handle itself stays
    // whole in the app so its thread can be joined on exit.
    let events: UnboundedReceiver<BackendEvent> =
        std::mem::replace(&mut backend.events, unbounded_channel().1);
    let mut app = App::new(options, backend, Instant::now());
    // Raw mode, alternate screen and a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = run(&mut app, events, &mut terminal).await;
    // Before the restore: no PipeWire object outlives it (VOL-ARCH-013).
    // A timeout is logged by `shutdown`; exiting continues regardless.
    app.backend.shutdown(SHUTDOWN_BUDGET);
    ratatui::restore();
    result.map(|()| ExitCode::SUCCESS)
}

/// Starts the event sources, then runs the app loop until quit.
async fn run(
    app: &mut App,
    backend_events: UnboundedReceiver<BackendEvent>,
    terminal: &mut DefaultTerminal,
) -> color_eyre::Result<()> {
    let mut events = EventHandler::new(backend_events)?;
    app.run(terminal, &mut events).await
}
