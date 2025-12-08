mod app;

#[tokio::main]
async fn main() {
    if let Err(err) = app::run().await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
