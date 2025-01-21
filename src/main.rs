mod config;
mod commands;
mod tui;
mod issue_viewer;
mod sentry;
mod dashboard;

fn main() -> anyhow::Result<()> {
    commands::Cli::run()
}
