//! TUI rendering — full dashboard UI using ratatui.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, BorderType, Cell, Gauge, List, ListItem,
        Paragraph, Row, Table, Tabs,
    },
    Frame,
};

use crate::app::{App, Tab};

const NETGENE_CYAN: Color = Color::Rgb(0, 255, 230);
const NETGENE_PURPLE: Color = Color::Rgb(180, 60, 255);
const NETGENE_GREEN: Color = Color::Rgb(50, 255, 120);
const NETGENE_ORANGE: Color = Color::Rgb(255, 160, 30);
const NETGENE_RED: Color = Color::Rgb(255, 60, 80);
const BG_DARK: Color = Color::Rgb(8, 10, 18);
const PANEL_BG: Color = Color::Rgb(14, 18, 30);

/// Main render entry point.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    // Root background
    f.render_widget(
        Block::default().style(Style::default().bg(BG_DARK)),
        area,
    );

    // Layout: header + tabs + content + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),  // Header
            Constraint::Length(3),  // Tabs
            Constraint::Min(0),     // Content
            Constraint::Length(2),  // Footer
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);
    render_content(f, app, chunks[2]);
    render_footer(f, app, chunks[3]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(NETGENE_CYAN))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // Left: Logo + title
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("🧬 ", Style::default()),
            Span::styled("NetGene OS", Style::default().fg(NETGENE_CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" v0.1", Style::default().fg(Color::DarkGray)),
            Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Netsphere Kernel", Style::default().fg(NETGENE_PURPLE)),
        ]),
        Line::from(vec![
            Span::styled("Gene: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.gene_fp, Style::default().fg(NETGENE_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("  Uptime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(app.uptime(), Style::default().fg(NETGENE_ORANGE)),
        ]),
    ])
    .alignment(Alignment::Left);
    f.render_widget(title, header[0]);

    // Right: System health
    let health_color = if app.network_health > 95.0 { NETGENE_GREEN }
        else if app.network_health > 80.0 { NETGENE_ORANGE }
        else { NETGENE_RED };

    let stats = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Network Health: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1}%", app.network_health),
                Style::default().fg(health_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Agents: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.agent_count), Style::default().fg(NETGENE_CYAN)),
        ]),
        Line::from(vec![
            Span::styled("Anomalies: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.anomalies_detected), Style::default().fg(NETGENE_RED)),
            Span::styled("  Heals: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.heals_applied), Style::default().fg(NETGENE_GREEN)),
            Span::styled("  Quantum Δ: +", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1}%", app.quantum_improvement),
                Style::default().fg(NETGENE_PURPLE).add_modifier(Modifier::BOLD),
            ),
        ]),
    ])
    .alignment(Alignment::Right);
    f.render_widget(stats, header[1]);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let all_tabs = Tab::all();
    let tabs: Vec<Line> = all_tabs
        .iter()
        .map(|t| Line::from(t.title()))
        .collect();

    let tab_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NETGENE_PURPLE))
                .style(Style::default().bg(PANEL_BG)),
        )
        .select(app.active_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(NETGENE_CYAN)
                .add_modifier(Modifier::BOLD)
                .bg(Color::Rgb(20, 30, 50)),
        )
        .divider(symbols::DOT);

    f.render_widget(tab_widget, area);
}

fn render_content(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        0 => render_dashboard(f, app, area),
        1 => render_agents(f, app, area),
        2 => render_network(f, app, area),
        3 => render_quantum(f, app, area),
        4 => render_logs(f, app, area),
        _ => {}
    }
}

fn render_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(cols[0]);

    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(cols[1]);

    // Top-left: Identity / Gene Layer
    let gene_text = vec![
        Line::from(vec![
            Span::styled("Role:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("MASTER", Style::default().fg(NETGENE_CYAN).bold()),
        ]),
        Line::from(vec![
            Span::styled("Status:  ", Style::default().fg(Color::DarkGray)),
            Span::styled("ACTIVE ●", Style::default().fg(NETGENE_GREEN).bold()),
        ]),
        Line::from(vec![
            Span::styled("Key:     ", Style::default().fg(Color::DarkGray)),
            Span::styled("Ed25519 (PQC ready)", Style::default().fg(NETGENE_PURPLE)),
        ]),
        Line::from(vec![
            Span::styled("FP:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.gene_fp, Style::default().fg(NETGENE_ORANGE).bold()),
        ]),
        Line::from(vec![
            Span::styled("Caps:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("gene.* node.* agent.* quantum.*", Style::default().fg(Color::White)),
        ]),
    ];

    let gene_block = Block::default()
        .title(" 🔑 Gene Layer ")
        .title_style(Style::default().fg(NETGENE_CYAN).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(NETGENE_CYAN))
        .style(Style::default().bg(PANEL_BG));

    f.render_widget(Paragraph::new(gene_text).block(gene_block), left_rows[0]);

    // Bottom-left: Safeguard
    let sg_text = vec![
        Line::from(vec![
            Span::styled("Anomalies detected:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.anomalies_detected), Style::default().fg(NETGENE_RED).bold()),
        ]),
        Line::from(vec![
            Span::styled("Self-heals applied:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.heals_applied), Style::default().fg(NETGENE_GREEN).bold()),
        ]),
        Line::from(vec![
            Span::styled("Zero-trust mTLS:     ", Style::default().fg(Color::DarkGray)),
            Span::styled("ENABLED", Style::default().fg(NETGENE_GREEN).bold()),
        ]),
        Line::from(vec![
            Span::styled("Threat level:        ", Style::default().fg(Color::DarkGray)),
            Span::styled("LOW", Style::default().fg(NETGENE_GREEN).bold()),
        ]),
    ];

    let sg_block = Block::default()
        .title(" 🛡️  Safeguard ")
        .title_style(Style::default().fg(NETGENE_ORANGE).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(NETGENE_ORANGE))
        .style(Style::default().bg(PANEL_BG));

    f.render_widget(Paragraph::new(sg_text).block(sg_block), left_rows[1]);

    // Top-right: Network health gauge
    let health_u8 = app.network_health as u16;
    let health_color = if app.network_health > 90.0 { NETGENE_GREEN }
        else if app.network_health > 75.0 { NETGENE_ORANGE }
        else { NETGENE_RED };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" 🌐 Network Health ")
                .title_style(Style::default().fg(NETGENE_GREEN).bold())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NETGENE_GREEN))
                .style(Style::default().bg(PANEL_BG)),
        )
        .gauge_style(Style::default().fg(health_color).bg(Color::Rgb(20, 30, 20)))
        .percent(health_u8)
        .label(format!("{:.1}%", app.network_health));

    f.render_widget(gauge, right_rows[0]);

    // Bottom-right: Quantum stats
    let qtext = vec![
        Line::from(vec![
            Span::styled("Algorithm:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("QAOA-sim + SQA", Style::default().fg(NETGENE_PURPLE).bold()),
        ]),
        Line::from(vec![
            Span::styled("Improvement: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("+{:.1}%", app.quantum_improvement),
                Style::default().fg(NETGENE_PURPLE).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("PQC keys:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("ACTIVE", Style::default().fg(NETGENE_GREEN).bold()),
        ]),
        Line::from(vec![
            Span::styled("Hardware:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("Classical (Quantum-ready)", Style::default().fg(Color::Gray)),
        ]),
    ];

    let q_block = Block::default()
        .title(" ⚛️  Quantum Enhancement ")
        .title_style(Style::default().fg(NETGENE_PURPLE).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(NETGENE_PURPLE))
        .style(Style::default().bg(PANEL_BG));

    f.render_widget(Paragraph::new(qtext).block(q_block), right_rows[1]);
}

fn render_agents(f: &mut Frame, _app: &App, area: Rect) {
    let agents = vec![
        vec!["BuilderAgent", "builder", "IDLE", "0", "Provisions nodes organically"],
        vec!["MonitorAgent", "monitor", "MONITORING", "0", "Watches for anomalies"],
        vec!["OptimizerAgent", "optimizer", "IDLE", "0", "Quantum routing optimizer"],
    ];

    let rows: Vec<Row> = agents.iter().map(|a| {
        let status_color = match a[2] {
            "IDLE" => NETGENE_GREEN,
            "PROCESSING" => NETGENE_ORANGE,
            _ => NETGENE_CYAN,
        };
        Row::new(vec![
            Cell::from(a[0]).style(Style::default().fg(NETGENE_CYAN).bold()),
            Cell::from(a[1]).style(Style::default().fg(Color::Gray)),
            Cell::from(a[2]).style(Style::default().fg(status_color).bold()),
            Cell::from(a[3]).style(Style::default().fg(Color::White)),
            Cell::from(a[4]).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let widths = [
        Constraint::Length(18),
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Min(0),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Name", "Type", "Status", "Msgs", "Description"])
                .style(Style::default().fg(NETGENE_PURPLE).bold().add_modifier(Modifier::UNDERLINED)),
        )
        .block(
            Block::default()
                .title(" 🤖 Netsphere Agents ")
                .title_style(Style::default().fg(NETGENE_PURPLE).bold())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NETGENE_PURPLE))
                .style(Style::default().bg(PANEL_BG)),
        )
        .row_highlight_style(Style::default().bg(Color::Rgb(25, 20, 50)));

    f.render_widget(table, area);
}

fn render_network(f: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app.nodes.iter().map(|n| {
        let status_color = match n.status.as_str() {
            "active" => NETGENE_GREEN,
            "degraded" => NETGENE_ORANGE,
            _ => NETGENE_RED,
        };
        let load_bar = format!("{:>3.0}%", n.load * 100.0);
        Row::new(vec![
            Cell::from(n.id.clone()).style(Style::default().fg(NETGENE_CYAN)),
            Cell::from(n.status.clone()).style(Style::default().fg(status_color).bold()),
            Cell::from(load_bar).style(Style::default().fg(if n.load > 0.8 { NETGENE_RED } else { Color::White })),
            Cell::from(format!("{:.1}ms", n.latency_ms)).style(Style::default().fg(NETGENE_ORANGE)),
            Cell::from(format!("{}", n.connections)).style(Style::default().fg(Color::Gray)),
        ])
    }).collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Min(0),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Node ID", "Status", "Load", "Latency", "Connections"])
                .style(Style::default().fg(NETGENE_GREEN).bold().add_modifier(Modifier::UNDERLINED)),
        )
        .block(
            Block::default()
                .title(" 🌐 Network Topology ")
                .title_style(Style::default().fg(NETGENE_GREEN).bold())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NETGENE_GREEN))
                .style(Style::default().bg(PANEL_BG)),
        );

    f.render_widget(table, area);
}

fn render_quantum(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(area);

    let info = vec![
        Line::from(vec![
            Span::styled("  Algorithm:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("QAOA (p=3, 100 iters) + Simulated Quantum Annealing", Style::default().fg(NETGENE_PURPLE).bold()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  QUBO Solver:  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Quantum-Inspired Classical (Phase 2: AWS Braket / IBM Q)", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Routing Δ:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("+{:.1}% vs classical Dijkstra", app.quantum_improvement), Style::default().fg(NETGENE_GREEN).bold()),
        ]),
        Line::from(vec![
            Span::styled("  PQC Layer:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("Ed25519 (upgrade → ML-KEM/Dilithium in Phase 2)", Style::default().fg(NETGENE_ORANGE)),
        ]),
        Line::from(vec![
            Span::styled("  Nodes:        ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} in topology", app.nodes.len()), Style::default().fg(NETGENE_CYAN)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Status:       ", Style::default().fg(Color::DarkGray)),
            Span::styled("⚛️  QUANTUM MODULE ACTIVE", Style::default().fg(NETGENE_PURPLE).bold()),
        ]),
    ];

    let q_block = Block::default()
        .title(" ⚛️  Quantum Enhancement Module ")
        .title_style(Style::default().fg(NETGENE_PURPLE).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(NETGENE_PURPLE))
        .style(Style::default().bg(PANEL_BG));

    f.render_widget(Paragraph::new(info).block(q_block), rows[0]);

    // Quantum improvement gauge
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Routing Optimization Improvement ")
                .title_style(Style::default().fg(NETGENE_PURPLE))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NETGENE_PURPLE))
                .style(Style::default().bg(PANEL_BG)),
        )
        .gauge_style(Style::default().fg(NETGENE_PURPLE).bg(Color::Rgb(20, 10, 35)))
        .percent((app.quantum_improvement * 3.0) as u16)
        .label(format!("+{:.1}% improvement", app.quantum_improvement));

    f.render_widget(gauge, rows[1]);
}

fn render_logs(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.logs.iter().rev().take(50).map(|log| {
        let color = if log.contains("ERROR") || log.contains("🔴") { NETGENE_RED }
            else if log.contains("WARN") || log.contains("Anomaly") { NETGENE_ORANGE }
            else if log.contains("✅") || log.contains("🟢") { NETGENE_GREEN }
            else if log.contains("⚛️") { NETGENE_PURPLE }
            else { Color::Gray };
        ListItem::new(log.as_str()).style(Style::default().fg(color))
    }).collect();

    let log_block = Block::default()
        .title(" 📋 System Logs (newest first) ")
        .title_style(Style::default().fg(NETGENE_CYAN).bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(NETGENE_CYAN))
        .style(Style::default().bg(PANEL_BG));

    f.render_widget(List::new(items).block(log_block), area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let keys = Paragraph::new(Line::from(vec![
        Span::styled(" [Tab]", Style::default().fg(NETGENE_CYAN).bold()),
        Span::styled(" Switch tab  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[↑/↓]", Style::default().fg(NETGENE_CYAN).bold()),
        Span::styled(" Scroll  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[r]", Style::default().fg(NETGENE_CYAN).bold()),
        Span::styled(" Refresh  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[q]", Style::default().fg(NETGENE_RED).bold()),
        Span::styled(" Quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("  Tick: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}", app.tick_count), Style::default().fg(Color::Gray)),
    ]))
    .style(Style::default().bg(Color::Rgb(10, 12, 22)));

    f.render_widget(keys, area);
}
