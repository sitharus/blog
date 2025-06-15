use bsky_sdk::BskyAgent;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = BskyAgent::builder().build().await?;
    let session = agent.login("", "").await?;
    Ok(())
}
