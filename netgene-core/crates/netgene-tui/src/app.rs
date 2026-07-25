//! TUI application state.

use std::time::Instant;
use chrono::{DateTime, Utc};
use netgene_gene::identity::NetGene;

/// Active tab in the TUI.
#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Dashboard,
    Agents,
    Network,
    Quantum,
    Logs,
}

impl Tab {
    pub fn all() -> Vec<Tab> {
        vec![Tab::Dashboard, Tab::Agents, Tab::Network, Tab::Quantum, Tab::Logs]
    }

    pub fn title(&self) -> &str {
        match self {
            Tab::Dashboard => "  Dashboard  ",
            Tab::Agents => "  Agents  ",
            Tab::Network => "  Network  ",
            Tab::Quantum => "  Quantum  ",
            Tab::Logs => "  Logs  ",
        }
    }
}

/// Simulated node state for display.
#[derive(Debug, Clone)]
pub struct NodeState {
    pub id: String,
    pub status: String,
    pub load: f64,
    pub latency_ms: f64,
    pub connections: usize,
}

/// Application state.
pub struct App {
    pub active_tab: usize,
    pub scroll: usize,
    pub tick_count: u64,
    pub start_time: Instant,
    pub logs: Vec<String>,
    pub nodes: Vec<NodeState>,
    pub agent_count: usize,
    pub gene_id: String,
    pub gene_fp: String,
    pub quantum_improvement: f64,
    pub anomalies_detected: usize,
    pub heals_applied: usize,
    pub network_health: f64,
    pub last_refresh: DateTime<Utc>,
}

impl App {
    pub async fn new() -> Self {
        // Generate demo gene for display
        let (gene, _kp) = NetGene::generate_master("NetGene Core").unwrap_or_else(|_| {
            // Fallback demo values
            panic!("Could not generate master gene")
        });

        let logs = vec![
            format!("[{}] 🧬 NetGene OS v0.1 booting...", Utc::now().format("%H:%M:%S")),
            format!("[{}] ✅ Gene Layer initialized — fp:{}", Utc::now().format("%H:%M:%S"), &gene.short_fp),
            format!("[{}] ✅ Netsphere Kernel online (3 agents)", Utc::now().format("%H:%M:%S")),
            format!("[{}] ✅ Quantum Module ready (QAOA-sim + SQA)", Utc::now().format("%H:%M:%S")),
            format!("[{}] ✅ Safeguard Layer armed", Utc::now().format("%H:%M:%S")),
            format!("[{}] 🟢 System ONLINE — All layers active", Utc::now().format("%H:%M:%S")),
        ];

        let nodes = (0..8).map(|i| NodeState {
            id: format!("node-{:02}", i),
            status: if i == 3 { "degraded".to_string() } else { "active".to_string() },
            load: 0.2 + (i as f64 * 0.08),
            latency_ms: 5.0 + i as f64 * 3.5,
            connections: 2 + i % 4,
        }).collect();

        Self {
            active_tab: 0,
            scroll: 0,
            tick_count: 0,
            start_time: Instant::now(),
            logs,
            nodes,
            agent_count: 3,
            gene_id: gene.id.to_string(),
            gene_fp: gene.short_fp.clone(),
            quantum_improvement: 18.4,
            anomalies_detected: 0,
            heals_applied: 0,
            network_health: 96.2,
            last_refresh: Utc::now(),
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % Tab::all().len();
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab == 0 {
            self.active_tab = Tab::all().len() - 1;
        } else {
            self.active_tab -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub async fn refresh(&mut self) {
        self.last_refresh = Utc::now();
        self.log(format!("Manual refresh triggered"));
    }

    pub fn log(&mut self, msg: String) {
        self.logs.push(format!("[{}] {}", Utc::now().format("%H:%M:%S"), msg));
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    pub async fn tick(&mut self) {
        self.tick_count += 1;

        // Simulate live metrics changing
        if self.tick_count % 10 == 0 {
            for node in &mut self.nodes {
                let jitter = (self.tick_count as f64 * 0.017).sin() * 0.05;
                node.load = (node.load + jitter).clamp(0.05, 0.95);
                node.latency_ms = (node.latency_ms + jitter * 10.0).clamp(1.0, 200.0);
            }
        }

        // Simulate occasional anomaly
        if self.tick_count % 50 == 0 {
            self.anomalies_detected += 1;
            self.heals_applied += 1;
            self.network_health = (self.network_health - 0.1 + 0.2).clamp(80.0, 100.0);
            self.log(format!("🔴 Anomaly on node-03 — z-score 3.2 — Auto-healed"));
        }

        // Update quantum improvement
        if self.tick_count % 30 == 0 {
            self.quantum_improvement = 15.0 + (self.tick_count as f64 * 0.03).sin() * 8.0;
        }
    }

    pub fn uptime(&self) -> String {
        let secs = self.start_time.elapsed().as_secs();
        format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_initializes_with_nodes() {
        let app = App::new().await;
        assert_eq!(app.nodes.len(), 8);
        assert_eq!(app.active_tab, 0);
        assert!(!app.gene_fp.is_empty());
        assert!(app.network_health > 0.0);
    }

    #[tokio::test]
    async fn test_tab_navigation_next() {
        let mut app = App::new().await;
        let total = Tab::all().len();
        app.next_tab();
        assert_eq!(app.active_tab, 1);
        // Wrap around
        for _ in 0..total {
            app.next_tab();
        }
        assert_eq!(app.active_tab, 1); // back to 1 after full cycle
    }

    #[tokio::test]
    async fn test_tab_navigation_prev_wraps() {
        let mut app = App::new().await;
        assert_eq!(app.active_tab, 0);
        app.prev_tab(); // Should wrap to last tab
        assert_eq!(app.active_tab, Tab::all().len() - 1);
    }

    #[tokio::test]
    async fn test_scroll_down_and_up() {
        let mut app = App::new().await;
        assert_eq!(app.scroll, 0);
        app.scroll_down();
        app.scroll_down();
        assert_eq!(app.scroll, 2);
        app.scroll_up();
        assert_eq!(app.scroll, 1);
        // Can't go below 0
        app.scroll_up();
        app.scroll_up();
        assert_eq!(app.scroll, 0);
    }

    #[tokio::test]
    async fn test_tick_increments_count() {
        let mut app = App::new().await;
        assert_eq!(app.tick_count, 0);
        app.tick().await;
        assert_eq!(app.tick_count, 1);
    }

    #[tokio::test]
    async fn test_log_appends_messages() {
        let mut app = App::new().await;
        let initial_len = app.logs.len();
        app.log("test-log-entry".to_string());
        assert_eq!(app.logs.len(), initial_len + 1);
        assert!(app.logs.last().unwrap().contains("test-log-entry"));
    }

    #[tokio::test]
    async fn test_log_limits_to_200() {
        let mut app = App::new().await;
        // Flood with 250 messages
        for i in 0..250 {
            app.log(format!("log-{}", i));
        }
        assert!(app.logs.len() <= 200, "Log should be capped at 200");
    }

    #[tokio::test]
    async fn test_refresh_updates_timestamp() {
        let mut app = App::new().await;
        let before = app.last_refresh;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        app.refresh().await;
        assert!(app.last_refresh >= before);
    }

    #[tokio::test]
    async fn test_uptime_format() {
        let app = App::new().await;
        let uptime = app.uptime();
        // Format should be HH:MM:SS
        assert_eq!(uptime.len(), 8);
        assert_eq!(uptime.chars().nth(2), Some(':'));
        assert_eq!(uptime.chars().nth(5), Some(':'));
    }

    #[test]
    fn test_tab_titles() {
        assert_eq!(Tab::Dashboard.title(), "  Dashboard  ");
        assert_eq!(Tab::Agents.title(), "  Agents  ");
        assert_eq!(Tab::Network.title(), "  Network  ");
        assert_eq!(Tab::Quantum.title(), "  Quantum  ");
        assert_eq!(Tab::Logs.title(), "  Logs  ");
    }

    #[test]
    fn test_tab_all_has_five_elements() {
        assert_eq!(Tab::all().len(), 5);
    }
}
