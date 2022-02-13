use anyhow::Result;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

#[async_trait]
pub trait Server: Send {
    type Request: DeserializeOwned + Send;
    type Response: Serialize + Send;
    async fn process_request(&mut self, request: Self::Request) -> Result<Self::Response>;
}
