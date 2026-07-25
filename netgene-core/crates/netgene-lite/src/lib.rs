#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Estrutura do Telemetry enviada pelos sensores IoT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteTelemetry {
    pub device_id: String,
    pub temperature: f32,
    pub status: u8, // 0 = OK, 1 = Warning, 2 = Critical
    pub anomaly_flag: bool,
}

impl LiteTelemetry {
    pub fn new(device_id: &str, temperature: f32, status: u8, anomaly_flag: bool) -> Self {
        Self {
            device_id: String::from(device_id),
            temperature,
            status,
            anomaly_flag,
        }
    }

    /// Serializa para formato binário leve (postcard)
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    /// Desserializa do formato binário leve
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

/// Modo de operação do nó Lite
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LiteNodeMode {
    /// Sensor puro (lê e envia)
    Sensor,
    /// Relay (Recebe de Sensores BLE e encaminha via TCP para o Kernel)
    Relay,
}

/// Nó Lite mínimo para embarcados
pub struct LiteNode {
    pub mode: LiteNodeMode,
    pub id: String,
}

impl LiteNode {
    pub fn new(id: &str, mode: LiteNodeMode) -> Self {
        Self {
            id: String::from(id),
            mode,
        }
    }

    pub fn generate_mock_telemetry(&self) -> LiteTelemetry {
        // Num ambiente real, leríamos dos pinos ADC do ESP32 ou STM32
        LiteTelemetry::new(&self.id, 24.5, 0, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_serialization() {
        let telemetry = LiteTelemetry::new("sensor-01", 25.0, 0, false);
        let bytes = telemetry.to_bytes().unwrap();
        let decoded = LiteTelemetry::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.device_id, "sensor-01");
        assert_eq!(decoded.temperature, 25.0);
    }

    #[test]
    fn test_lite_node_mock_telemetry() {
        let node = LiteNode::new("relay-01", LiteNodeMode::Relay);
        let telemetry = node.generate_mock_telemetry();
        assert_eq!(telemetry.device_id, "relay-01");
        assert_eq!(telemetry.status, 0);
    }

    #[test]
    fn test_lite_node_mode_equality() {
        assert_eq!(LiteNodeMode::Sensor, LiteNodeMode::Sensor);
        assert_ne!(LiteNodeMode::Sensor, LiteNodeMode::Relay);
    }
}
