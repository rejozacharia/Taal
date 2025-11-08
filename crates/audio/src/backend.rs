use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct StreamConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            buffer_size: 512,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamHandle {
    config: StreamConfig,
}

impl StreamHandle {
    pub fn config(&self) -> StreamConfig {
        self.config
    }
}

pub trait AudioBackend: Send + Sync {
    fn open_stream(&self, config: &StreamConfig) -> Result<StreamHandle>;
    fn measure_latency(&self, _handle: &StreamHandle) -> Result<Duration> {
        Ok(Duration::from_millis(5))
    }
}

pub struct NullBackend;

impl AudioBackend for NullBackend {
    fn open_stream(&self, config: &StreamConfig) -> Result<StreamHandle> {
        debug!(?config, "opening null audio stream");
        Ok(StreamHandle { config: *config })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_backend_returns_config() {
        let backend = NullBackend;
        let config = StreamConfig {
            buffer_size: 128,
            ..Default::default()
        };
        let handle = backend.open_stream(&config).unwrap();
        assert_eq!(handle.config().buffer_size, 128);
    }
}
