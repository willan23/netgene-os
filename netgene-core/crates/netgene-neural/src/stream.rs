//! Neural & BCI intent stream processor.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use tokio::sync::mpsc;
use tracing::info;

/// BCI EEG frequency bands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EegSignals {
    pub alpha: f64, // 8-12 Hz (relaxation/focus)
    pub beta: f64,  // 12-30 Hz (active thought/intent)
    pub gamma: f64, // 30-100 Hz (high cognitive processing)
    pub signal_quality: f64, // 0.0 - 1.0
}

/// Neural focus intent payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralIntentEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub focus_target: String,
    pub cognitive_load: f64,
    pub raw_signals: EegSignals,
    pub converted_action: String,
}

pub struct NeuralStreamAdapter {
    out_tx: mpsc::Sender<NeuralIntentEvent>,
}

impl NeuralStreamAdapter {
    pub fn new() -> (Self, mpsc::Receiver<NeuralIntentEvent>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { out_tx: tx }, rx)
    }

    /// Process raw BCI signal and stream converted intent
    pub async fn process_signal(&self, target: &str, beta: f64, gamma: f64) -> anyhow::Result<NeuralIntentEvent> {
        let load = (beta * 0.6 + gamma * 0.4).min(1.0);
        let action = if load > 0.75 {
            format!("EMERGENCY_HEAL_{}", target.to_uppercase())
        } else if load > 0.5 {
            format!("OPTIMIZE_ROUTE_{}", target.to_uppercase())
        } else {
            format!("MONITOR_{}", target.to_uppercase())
        };

        let event = NeuralIntentEvent {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            focus_target: target.to_string(),
            cognitive_load: load,
            raw_signals: EegSignals {
                alpha: 0.82,
                beta,
                gamma,
                signal_quality: 0.98,
            },
            converted_action: action,
        };

        info!("🧠 Neural Intent Processed: action='{}' load={:.2}", event.converted_action, event.cognitive_load);
        let _ = self.out_tx.send(event.clone()).await;
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_neural_signal_processing() -> anyhow::Result<()> {
        let (adapter, mut rx) = NeuralStreamAdapter::new();
        let event = adapter.process_signal("node-01", 0.9, 0.8).await?;

        assert_eq!(event.focus_target, "node-01");
        assert!(event.converted_action.contains("EMERGENCY_HEAL"));
        
        let received = rx.try_recv()?;
        assert_eq!(received.event_id, event.event_id);

        Ok(())
    }
}
