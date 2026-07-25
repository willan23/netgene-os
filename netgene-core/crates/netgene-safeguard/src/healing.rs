//! Self-healing state machine.
//!
//! When anomalies are detected, the SelfHealingEngine votes on and applies
//! remediation actions automatically.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::{info, warn};

use crate::anomaly::{AnomalyEvent, AnomalySeverity};

/// Available healing actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealingAction {
    RestartNode { node_id: String },
    RerouteTraffic { from: String, to: String },
    ScaleUp { resource: String, amount: f64 },
    IsolateNode { node_id: String },
    ApplyPatch { component: String },
    Alert { message: String },
}

impl std::fmt::Display for HealingAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealingAction::RestartNode { node_id } => write!(f, "Restart({})", node_id),
            HealingAction::RerouteTraffic { from, to } => write!(f, "Reroute({} → {})", from, to),
            HealingAction::ScaleUp { resource, amount } => write!(f, "ScaleUp({}, +{:.0}%)", resource, amount),
            HealingAction::IsolateNode { node_id } => write!(f, "Isolate({})", node_id),
            HealingAction::ApplyPatch { component } => write!(f, "Patch({})", component),
            HealingAction::Alert { message } => write!(f, "Alert: {}", message),
        }
    }
}

/// Result of a healing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingResult {
    pub action: String,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    pub notes: String,
}

/// Self-healing state machine engine.
pub struct SelfHealingEngine {
    pub healing_history: Vec<HealingResult>,
    pub auto_heal: bool,
}

impl SelfHealingEngine {
    pub fn new(auto_heal: bool) -> Self {
        Self {
            healing_history: vec![],
            auto_heal,
        }
    }

    /// Evaluate an anomaly and decide healing action.
    pub fn evaluate(&self, event: &AnomalyEvent) -> HealingAction {
        match event.severity {
            AnomalySeverity::Critical => {
                // Aggressive: isolate the affected node
                HealingAction::IsolateNode {
                    node_id: format!("node-{}", &event.metric[..4.min(event.metric.len())]),
                }
            }
            AnomalySeverity::High => {
                // Reroute traffic away from problematic path
                HealingAction::RerouteTraffic {
                    from: event.metric.clone(),
                    to: "backup-path-01".to_string(),
                }
            }
            AnomalySeverity::Medium => {
                // Scale up resources
                HealingAction::ScaleUp {
                    resource: event.metric.clone(),
                    amount: 50.0,
                }
            }
            AnomalySeverity::Low => {
                // Just send alert
                HealingAction::Alert {
                    message: event.description.clone(),
                }
            }
        }
    }

    /// Apply a healing action (simulated in Phase 1).
    pub async fn apply(&mut self, action: HealingAction) -> Result<HealingResult> {
        // Simulate async healing operation
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let action_str = action.to_string();
        info!(action = %action_str, "Applying self-healing action");

        let result = HealingResult {
            action: action_str.clone(),
            success: true,
            timestamp: Utc::now(),
            notes: format!("Simulated healing: {}", action_str),
        };

        self.healing_history.push(result.clone());
        Ok(result)
    }

    /// Auto-heal from an anomaly: evaluate + apply.
    pub async fn auto_heal_from_anomaly(&mut self, event: &AnomalyEvent) -> Option<HealingResult> {
        if !self.auto_heal { return None; }

        let action = self.evaluate(event);
        match self.apply(action).await {
            Ok(result) => Some(result),
            Err(e) => {
                warn!(err = %e, "Self-healing failed");
                None
            }
        }
    }

    /// Total heals applied.
    pub fn heal_count(&self) -> usize {
        self.healing_history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anomaly::{AnomalyDetector};

    /// Helper: build a fake anomaly event for testing.
    fn make_anomaly(severity: AnomalySeverity) -> AnomalyEvent {
        AnomalyEvent {
            metric: "cpu_load".to_string(),
            value: 99.0,
            expected_range: (0.0, 50.0),
            z_score: 5.0,
            severity,
            timestamp: chrono::Utc::now(),
            description: "Test anomaly".to_string(),
        }
    }

    #[test]
    fn test_evaluate_critical_isolates_node() {
        let engine = SelfHealingEngine::new(true);
        let event = make_anomaly(AnomalySeverity::Critical);
        let action = engine.evaluate(&event);
        assert!(matches!(action, HealingAction::IsolateNode { .. }));
    }

    #[test]
    fn test_evaluate_high_reroutes_traffic() {
        let engine = SelfHealingEngine::new(true);
        let event = make_anomaly(AnomalySeverity::High);
        let action = engine.evaluate(&event);
        assert!(matches!(action, HealingAction::RerouteTraffic { .. }));
    }

    #[test]
    fn test_evaluate_medium_scales_up() {
        let engine = SelfHealingEngine::new(true);
        let event = make_anomaly(AnomalySeverity::Medium);
        let action = engine.evaluate(&event);
        assert!(matches!(action, HealingAction::ScaleUp { .. }));
    }

    #[test]
    fn test_evaluate_low_sends_alert() {
        let engine = SelfHealingEngine::new(true);
        let event = make_anomaly(AnomalySeverity::Low);
        let action = engine.evaluate(&event);
        assert!(matches!(action, HealingAction::Alert { .. }));
    }

    #[tokio::test]
    async fn test_apply_healing_action_succeeds() {
        let mut engine = SelfHealingEngine::new(true);
        let action = HealingAction::RestartNode { node_id: "node-01".to_string() };
        let result = engine.apply(action).await.unwrap();
        assert!(result.success);
        assert!(result.action.contains("node-01"));
        assert_eq!(engine.heal_count(), 1);
    }

    #[tokio::test]
    async fn test_auto_heal_from_anomaly_applies_action() {
        let mut engine = SelfHealingEngine::new(true);
        let event = make_anomaly(AnomalySeverity::High);
        let result = engine.auto_heal_from_anomaly(&event).await;
        assert!(result.is_some());
        assert!(result.unwrap().success);
        assert_eq!(engine.heal_count(), 1);
    }

    #[tokio::test]
    async fn test_auto_heal_disabled_returns_none() {
        let mut engine = SelfHealingEngine::new(false);
        let event = make_anomaly(AnomalySeverity::Critical);
        let result = engine.auto_heal_from_anomaly(&event).await;
        assert!(result.is_none());
        assert_eq!(engine.heal_count(), 0);
    }

    #[test]
    fn test_healing_action_display() {
        let a = HealingAction::RestartNode { node_id: "n-1".to_string() };
        assert!(format!("{}", a).contains("n-1"));
        let b = HealingAction::ScaleUp { resource: "mem".to_string(), amount: 25.0 };
        assert!(format!("{}", b).contains("mem"));
    }

    #[tokio::test]
    async fn test_heal_count_accumulates() {
        let mut engine = SelfHealingEngine::new(true);
        for _ in 0..5 {
            let event = make_anomaly(AnomalySeverity::Low);
            engine.auto_heal_from_anomaly(&event).await;
        }
        assert_eq!(engine.heal_count(), 5);
    }

    #[tokio::test]
    async fn test_full_anomaly_detect_and_heal_pipeline() {
        let mut detector = AnomalyDetector::new(50, 2.0);
        let mut engine = SelfHealingEngine::new(true);

        // Warm up detector
        for i in 0..30 {
            let noise = if i % 2 == 0 { 0.5 } else { -0.5 };
            detector.ingest("latency", 10.0 + noise);
        }

        // Inject extreme spike
        let anomaly = detector.ingest("latency", 5000.0);
        assert!(anomaly.is_some(), "Spike should be detected");

        let result = engine.auto_heal_from_anomaly(anomaly.as_ref().unwrap()).await;
        assert!(result.is_some());
        assert!(result.unwrap().success);
    }
}
