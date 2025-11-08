use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

use taal_domain::LessonDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceItem {
    pub id: String,
    pub title: String,
    pub author: String,
}

#[derive(Clone)]
pub struct MarketplaceClient {
    pub endpoint: String,
}

impl MarketplaceClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub async fn list_items(&self) -> Result<Vec<MarketplaceItem>> {
        info!("listing marketplace items", endpoint = %self.endpoint);
        Ok(Vec::new())
    }

    pub async fn upload_lesson(&self, lesson: &LessonDescriptor) -> Result<()> {
        info!("uploading lesson", id = %lesson.id, endpoint = %self.endpoint);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_items_returns_empty() {
        let client = MarketplaceClient::new("https://example.com");
        let items = client.list_items().await.unwrap();
        assert!(items.is_empty());
    }
}
