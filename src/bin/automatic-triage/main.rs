use reqwest::Client;
use triagebot::github::GithubClient;

mod old_label;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let client = GithubClient::new_with_default_token(Client::new());
    old_label::issues_with_minimum_label_and_last_comment_age(
        "rust-lang",
        "rust",
        "E-needs-mcve",
        chrono::Duration::days(30 * 12 * 4), // 3 years
        &client,
    )
    .await?;

    Ok(())
}
