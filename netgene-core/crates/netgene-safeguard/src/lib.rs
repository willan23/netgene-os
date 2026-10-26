//! # NetGene Safeguard Layer
//!
//! Proactive security, anomaly detection, and self-healing capabilities.

pub mod anomaly;

pub use anomaly::{AnomalyDetector, AnomalyEvent, AnomalySeverity};
