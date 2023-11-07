use reqwest::Client;
use triagebot::github::{GithubClient, Repository};

mod too_old_label;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let client = GithubClient::new_with_default_token(Client::new());


    print!("asdf");
    Ok(())
}
