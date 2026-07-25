//! # NetGene Safeguard Layer
//!
//! Proactive security, anomaly detection, and self-healing capabilities.

pub mod anomaly;
pub mod healing;

pub use anomaly::{AnomalyDetector, AnomalyEvent, AnomalySeverity};
pub use healing::{SelfHealingEngine, HealingAction, HealingResult};
