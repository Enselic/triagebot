use reqwest::Client;
use triagebot::github::{GithubClient, Repository};

mod too_old_label;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let client = GithubClient::new_with_default_token(Client::new());
    too_old_label::issues_with_minimum_label_age(
        "rust-lang",
        "rust",
        "T-needs-mcve",
        std::time::Duration::from_secs(60 * 60 * 24 * 7 * 4),
        &client,
    );

    Ok(())
}
