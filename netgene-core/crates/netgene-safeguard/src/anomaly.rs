//! Anomaly detection using statistical thresholds.
//!
//! Monitors metrics streams and detects deviations using
//! Z-score + moving average analysis.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use tracing::warn;

/// Severity classification of detected anomalies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for AnomalySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalySeverity::Low => write!(f, "LOW"),
            AnomalySeverity::Medium => write!(f, "MEDIUM"),
            AnomalySeverity::High => write!(f, "HIGH"),
            AnomalySeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A detected anomaly event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub metric: String,
    pub value: f64,
    pub expected_range: (f64, f64),
    pub z_score: f64,
    pub severity: AnomalySeverity,
    pub timestamp: DateTime<Utc>,
    pub description: String,
}

/// Statistical anomaly detector using moving window.
pub struct AnomalyDetector {
    /// Metric name → recent values window.
    windows: std::collections::HashMap<String, VecDeque<f64>>,
    /// Window size for moving statistics.
    window_size: usize,
    /// Z-score threshold for anomaly detection.
    z_threshold: f64,
    /// Detected anomaly history.
    pub events: Vec<AnomalyEvent>,
}

impl AnomalyDetector {
    pub fn new(window_size: usize, z_threshold: f64) -> Self {
        Self {
            windows: std::collections::HashMap::new(),
            window_size,
            z_threshold,
            events: vec![],
        }
    }

    pub fn default() -> Self {
        Self::new(50, 2.5)
    }

    /// Ingest a new metric value and check for anomalies.
    pub fn ingest(&mut self, metric: &str, value: f64) -> Option<AnomalyEvent> {
        let window = self.windows
            .entry(metric.to_string())
            .or_insert_with(VecDeque::new);

        window.push_back(value);
        if window.len() > self.window_size {
            window.pop_front();
        }

        if window.len() < 10 {
            return None; // Not enough data
        }

        let mean = window.iter().sum::<f64>() / window.len() as f64;
        let variance = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / window.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev < 1e-10 { return None; }

        let z_score = (value - mean).abs() / std_dev;

        if z_score > self.z_threshold {
            let severity = if z_score > self.z_threshold * 2.0 {
                AnomalySeverity::Critical
            } else if z_score > self.z_threshold * 1.5 {
                AnomalySeverity::High
            } else if z_score > self.z_threshold * 1.2 {
                AnomalySeverity::Medium
            } else {
                AnomalySeverity::Low
            };

            let event = AnomalyEvent {
                metric: metric.to_string(),
                value,
                expected_range: (mean - self.z_threshold * std_dev, mean + self.z_threshold * std_dev),
                z_score,
                severity: severity.clone(),
                timestamp: Utc::now(),
                description: format!(
                    "Anomaly on '{}': value={:.2}, z-score={:.2}, severity={}",
                    metric, value, z_score, severity
                ),
            };

            warn!(
                metric = metric,
                value = value,
                z_score = z_score,
                severity = %severity,
                "Anomaly detected"
            );

            self.events.push(event.clone());
            Some(event)
        } else {
            None
        }
    }

    /// Recent anomaly events (last n).
    pub fn recent_events(&self, n: usize) -> Vec<&AnomalyEvent> {
        self.events.iter().rev().take(n).collect()
    }

    /// Critical events count.
    pub fn critical_count(&self) -> usize {
        self.events.iter().filter(|e| e.severity == AnomalySeverity::Critical).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: warm up detector with `n` normal values around mean.
    fn warm_up(det: &mut AnomalyDetector, metric: &str, n: usize, mean: f64) {
        for i in 0..n {
            // small noise so std_dev > 0
            let noise = if i % 2 == 0 { 0.5 } else { -0.5 };
            det.ingest(metric, mean + noise);
        }
    }

    #[test]
    fn test_no_anomaly_for_normal_values() {
        let mut det = AnomalyDetector::new(50, 2.5);
        warm_up(&mut det, "latency", 20, 10.0);
        // Value within normal range — should NOT be flagged
        let result = det.ingest("latency", 10.3);
        assert!(result.is_none(), "Expected no anomaly for normal value");
    }

    #[test]
    fn test_anomaly_detected_on_spike() {
        let mut det = AnomalyDetector::new(50, 2.5);
        warm_up(&mut det, "cpu", 30, 20.0);
        // Large spike 10× the normal std_dev
        let event = det.ingest("cpu", 200.0);
        assert!(event.is_some(), "Expected anomaly on large spike");
        let e = event.unwrap();
        assert!(e.z_score > 2.5);
        assert_eq!(e.metric, "cpu");
    }

    #[test]
    fn test_severity_critical_on_extreme_spike() {
        let mut det = AnomalyDetector::new(50, 2.0);
        warm_up(&mut det, "packets", 30, 100.0);
        // Extreme deviation → Critical
        let event = det.ingest("packets", 10_000.0);
        assert!(event.is_some());
        assert_eq!(event.unwrap().severity, AnomalySeverity::Critical);
    }

    #[test]
    fn test_insufficient_data_returns_none() {
        let mut det = AnomalyDetector::default();
        // Only 5 values — below the 10-sample minimum
        for i in 0..5 {
            let r = det.ingest("rtt", i as f64);
            assert!(r.is_none(), "Should not detect anomaly with < 10 samples");
        }
    }

    #[test]
    fn test_events_accumulate() {
        let mut det = AnomalyDetector::new(50, 2.5);
        warm_up(&mut det, "bw", 30, 50.0);
        det.ingest("bw", 500.0);
        det.ingest("bw", 500.0);
        assert!(det.events.len() >= 1);
    }

    #[test]
    fn test_critical_count() {
        let mut det = AnomalyDetector::new(50, 2.0);
        warm_up(&mut det, "load", 30, 0.5);
        det.ingest("load", 1000.0);
        assert!(det.critical_count() >= 1);
    }

    #[test]
    fn test_recent_events_limit() {
        let mut det = AnomalyDetector::new(50, 2.0);
        warm_up(&mut det, "temp", 30, 30.0);
        for _ in 0..5 {
            det.ingest("temp", 10_000.0);
        }
        assert!(det.recent_events(3).len() <= 3);
    }

    #[test]
    fn test_anomaly_event_display_severity() {
        assert_eq!(format!("{}", AnomalySeverity::Low), "LOW");
        assert_eq!(format!("{}", AnomalySeverity::Medium), "MEDIUM");
        assert_eq!(format!("{}", AnomalySeverity::High), "HIGH");
        assert_eq!(format!("{}", AnomalySeverity::Critical), "CRITICAL");
    }
}
