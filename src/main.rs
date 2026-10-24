use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use ratatui::DefaultTerminal;
use vol::app::{App, Options};
use vol::backend::BackendHandle;
use vol::cli;
use vol::event::EventHandler;
use vol::theme;

/// Startup order per VOL-ARCH-060: CLI → color-eyre → backend → terminal →
/// app and events → run → restore on every exit path.
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
    let no_color = std::env::var_os("NO_COLOR").map(|v| v.to_string_lossy().into_owned());
    let options = Options {
        color: theme::color_enabled(cli.no_color, no_color.as_deref()),
        mouse: cli.mouse,
    };
    // Replaced by the PipeWire backend in M1.
    let backend = BackendHandle::inert();
    // Raw mode, alternate screen and a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = run(options, backend, &mut terminal).await;
    ratatui::restore();
    result.map(|()| ExitCode::SUCCESS)
}

/// Builds the app and its event sources, then runs the loop until quit.
async fn run(
    options: Options,
    backend: BackendHandle,
    terminal: &mut DefaultTerminal,
) -> color_eyre::Result<()> {
    let mut app = App::new(options, backend.commands, Instant::now());
    let mut events = EventHandler::new(backend.events)?;
    app.run(terminal, &mut events).await
}
