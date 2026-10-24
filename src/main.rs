use std::io::Write;
use std::process::ExitCode;

use vol::app::App;
use vol::cli;

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
    let terminal = ratatui::init();
    let result = App::new().run(terminal).await;
    ratatui::restore();
    result.map(|()| ExitCode::SUCCESS)
}
