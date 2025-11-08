use std::path::Path;

use anyhow::Result;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierOutput {
    pub label: String,
    pub confidence: f32,
}

pub trait DrumClassifier {
    fn infer(&self, features: &Array1<f32>) -> Result<ClassifierOutput>;
}

pub struct MockClassifier;

impl DrumClassifier for MockClassifier {
    fn infer(&self, features: &Array1<f32>) -> Result<ClassifierOutput> {
        let mean = features.mean().unwrap_or(0.0);
        let label = if mean > 0.5 { "snare" } else { "kick" };
        Ok(ClassifierOutput {
            label: label.to_string(),
            confidence: mean.clamp(0.0, 1.0) as f32,
        })
    }
}

pub fn load_mock_classifier<P: AsRef<Path>>(path: P) -> Result<MockClassifier> {
    info!("loading classifier placeholder: {:?}", path.as_ref());
    Ok(MockClassifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier_returns_label() {
        let classifier = MockClassifier;
        let features = Array1::from(vec![0.2, 0.3, 0.4]);
        let output = classifier.infer(&features).unwrap();
        assert_eq!(output.label, "kick");
    }
}
