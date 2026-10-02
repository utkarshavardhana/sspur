use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("http {0}")]
    Status(u16),
    #[error(transparent)]
    Decode(#[from] reqwest::Error),
    #[error("gave up after {0} attempts")]
    Exhausted(u32),
}

pub async fn fetch_user(client: &reqwest::Client, url: &str, attempts: u32) -> Result<User, FetchError> {
    for n in 0..attempts {
        match client.get(url).timeout(Duration::from_secs(2)).send().await {
            Ok(r) if r.status().is_server_error() => {}
            Ok(r) if !r.status().is_success() => return Err(FetchError::Status(r.status().as_u16())),
            Ok(r) => return Ok(r.json::<User>().await?),
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_millis(100 << n)).await;
    }
    Err(FetchError::Exhausted(attempts))
}
