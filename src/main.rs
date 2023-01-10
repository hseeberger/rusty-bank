use tracing::error;

#[tokio::main]
async fn main() {
    if let Err(error) = rusty_bank::run().await {
        error!(error = format!("{error:#}"), "rusty-bank exited with ERROR");
    };
}
