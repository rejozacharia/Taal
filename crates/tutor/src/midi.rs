use anyhow::Result;
use midir::MidiInput;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MidiDevice {
    pub name: String,
}

pub struct MidiManager;

impl MidiManager {
    pub fn list_inputs() -> Result<Vec<MidiDevice>> {
        let input = MidiInput::new("taal")?;
        Ok(input
            .ports()
            .iter()
            .map(|port| MidiDevice {
                name: input.port_name(port).unwrap_or_else(|_| "Unknown".into()),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_inputs_returns_vector() {
        let devices = MidiManager::list_inputs();
        assert!(devices.is_ok());
    }
}
