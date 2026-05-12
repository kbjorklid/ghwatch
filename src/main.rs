use ghwatch::app;
use ghwatch::storage::get_data_dir;
use ghwatch::logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Some(data_dir) = get_data_dir() {
        let _ = logging::init_logging(&data_dir);
    }

    let mut app = app::App::new()?;
    app.renderer.init()?;
    let res = app.run(false).await;
    app.renderer.restore()?;
    res
}
