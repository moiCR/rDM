mod args;
mod error;
mod format;
mod progress;
mod runner;

use std::process::ExitCode;

use crate::{args::CliArgs, runner::CliRunner};

#[tokio::main]
async fn main() -> ExitCode {
    let args = match CliArgs::parse() {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[ERR]  {err}\n");
            CliArgs::print_help();
            return ExitCode::FAILURE;
        }
    };

    let runner = match CliRunner::new() {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("[ERR]  Failed to initialize engine: {err}");
            return ExitCode::FAILURE;
        }
    };

    match runner.run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[ERR]  {err}");
            ExitCode::FAILURE
        }
    }
}
