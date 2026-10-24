use std::io::Write;
use std::process::ExitCode;

use std::time::Instant;

use vol::app::{App, Options};
use vol::backend::BackendHandle;
use vol::cli;
use vol::event::EventHandler;
use vol::theme;

#[tokio::main]
async fn main() -> color_eyre::Result<ExitCode> {
    color_eyre::install()?;
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
    let options = Options {
        color: theme::color_enabled(cli.no_color, std::env::var("NO_COLOR").ok().as_deref()),
        mouse: cli.mouse,
    };
    let BackendHandle { commands, events } = BackendHandle::inert();
    let mut terminal = ratatui::init();
    let result = run(options, commands, events, &mut terminal).await;
    ratatui::restore();
    result.map(|()| ExitCode::SUCCESS)
}

/// Builds the app and its event sources, then runs the loop.
async fn run(
    options: Options,
    commands: Box<dyn vol::backend::AudioBackend>,
    events: tokio::sync::mpsc::UnboundedReceiver<vol::backend::BackendEvent>,
    terminal: &mut ratatui::DefaultTerminal,
) -> color_eyre::Result<()> {
    let mut app = App::new(options, commands, Instant::now());
    let mut events = EventHandler::new(events)?;
    app.run(terminal, &mut events).await
}
