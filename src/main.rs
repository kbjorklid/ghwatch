pub mod app;
pub mod config;
pub mod domain;
pub mod github;
pub mod logging;
pub mod notify;
pub mod polling;
pub mod storage;
pub mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Phase 1: Minimal main to run the TUI with dummy data
    let mut app = app::App::new()?;
    app.run().await
}
