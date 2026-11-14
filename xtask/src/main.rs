//! Developer tasks. Run `cargo xtask help`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "" | "help" | "--help" => {
            println!("cargo xtask <task>\n\nTasks:\n  help    show this message");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown task `{other}`\nhint: run `cargo xtask help`");
            ExitCode::from(2)
        }
    }
}
