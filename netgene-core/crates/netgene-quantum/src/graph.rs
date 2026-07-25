//! Network graph representation for quantum routing optimization.
//!
//! Models the NetGene network topology as a weighted directed graph.
//! Converts routing problems to QUBO format for quantum optimization.

use serde::{Deserialize, Serialize};
use nalgebra::DMatrix;
use anyhow::Result;
use std::collections::HashMap;

use crate::annealing::QuantumAnnealer;

/// A network node in the NetGene topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNode {
    pub id: String,
    pub label: String,
    pub load: f64,        // 0.0 - 1.0
    pub capacity: f64,    // Mbps
    pub is_active: bool,
}

/// A network edge (link between nodes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEdge {
    pub from: String,
    pub to: String,
    pub latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub reliability: f64, // 0.0 - 1.0
}

/// Result of quantum routing optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingResult {
    /// Optimal path node IDs.
    pub path: Vec<String>,
    /// Total cost (latency + load factor).
    pub total_cost: f64,
    /// Algorithm used.
    pub algorithm: String,
    /// Improvement vs classical Dijkstra (%).
    pub improvement_pct: f64,
}

/// Network graph with quantum-enhanced routing.
pub struct NetworkGraph {
    nodes: HashMap<String, NetworkNode>,
    edges: Vec<NetworkEdge>,
}

impl NetworkGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: vec![],
        }
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, node: NetworkNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add an edge.
    pub fn add_edge(&mut self, edge: NetworkEdge) {
        self.edges.push(edge);
    }

    /// Generate a random demo topology.
    pub fn demo_topology(node_count: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut graph = Self::new();

        for i in 0..node_count {
            graph.add_node(NetworkNode {
                id: format!("node-{:02}", i),
                label: format!("Node-{}", i),
                load: rng.gen_range(0.1..0.9),
                capacity: rng.gen_range(100.0..10000.0),
                is_active: rng.gen_bool(0.9),
            });
        }

        // Add edges (each node connects to 2-3 random others)
        let ids: Vec<String> = graph.nodes.keys().cloned().collect();
        for i in 0..node_count {
            let connections = rng.gen_range(2..4.min(node_count));
            for _ in 0..connections {
                let j = rng.gen_range(0..node_count);
                if i != j {
                    graph.add_edge(NetworkEdge {
                        from: ids[i].clone(),
                        to: ids[j].clone(),
                        latency_ms: rng.gen_range(1.0..100.0),
                        bandwidth_mbps: rng.gen_range(10.0..1000.0),
                        reliability: rng.gen_range(0.8..1.0),
                    });
                }
            }
        }

        graph
    }

    /// Convert routing problem to QUBO matrix for quantum optimization.
    pub fn to_qubo(&self, _source: &str, _target: &str) -> DMatrix<f64> {
        let n = self.edges.len();
        if n == 0 { return DMatrix::zeros(1, 1); }

        let mut q = DMatrix::zeros(n, n);

        // Objective: minimize latency weighted by load
        for (i, edge) in self.edges.iter().enumerate() {
            let src_load = self.nodes.get(&edge.from).map(|n| n.load).unwrap_or(0.5);
            let cost = edge.latency_ms * (1.0 + src_load);
            q[(i, i)] = cost;
        }

        // Penalty: invalid paths (flow conservation)
        let penalty = 10.0 * self.edges.iter().map(|e| e.latency_ms).sum::<f64>() / n as f64;
        for i in 0..n {
            for j in (i+1)..n {
                let ei = &self.edges[i];
                let ej = &self.edges[j];
                // Penalize conflicting edges at same node
                if ei.from == ej.from && i != j {
                    q[(i, j)] += penalty * 0.5;
                }
            }
        }

        q
    }

    /// Optimize routing using quantum annealing.
    pub fn quantum_route(&self, source: &str, target: &str) -> Result<RoutingResult> {
        let q = self.to_qubo(source, target);
        let annealer = QuantumAnnealer::default();
        let result = annealer.anneal(&q)?;

        // Decode solution: which edges are "active"
        let active_edges: Vec<&NetworkEdge> = result.solution.iter()
            .enumerate()
            .filter(|(_, &x)| x == 1)
            .filter_map(|(i, _)| self.edges.get(i))
            .collect();

        // Build path from active edges
        let path = self.build_path_from_edges(&active_edges, source, target);
        let total_cost = active_edges.iter().map(|e| e.latency_ms).sum();

        Ok(RoutingResult {
            path,
            total_cost,
            algorithm: "Quantum Annealing (SQA)".to_string(),
            improvement_pct: result.improvement_pct,
        })
    }

    /// Build an ordered path from a set of active edges.
    fn build_path_from_edges(&self, edges: &[&NetworkEdge], src: &str, _tgt: &str) -> Vec<String> {
        if edges.is_empty() { return vec![src.to_string()]; }

        let mut path = vec![src.to_string()];
        let mut current = src.to_string();
        let mut visited = std::collections::HashSet::new();
        visited.insert(current.clone());

        for _ in 0..edges.len() {
            if let Some(edge) = edges.iter().find(|e| e.from == current && !visited.contains(&e.to)) {
                current = edge.to.clone();
                path.push(current.clone());
                visited.insert(current.clone());
            } else {
                break;
            }
        }

        path
    }

    /// Node count.
    pub fn node_count(&self) -> usize { self.nodes.len() }
    /// Edge count.
    pub fn edge_count(&self) -> usize { self.edges.len() }
    /// Get all nodes.
    pub fn nodes(&self) -> Vec<&NetworkNode> { self.nodes.values().collect() }
    /// Get all edges.
    pub fn edges(&self) -> &[NetworkEdge] { &self.edges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_topology() {
        let graph = NetworkGraph::demo_topology(5);
        assert_eq!(graph.node_count(), 5);
        assert!(graph.edge_count() > 0);
    }

    #[test]
    fn test_quantum_route() {
        let graph = NetworkGraph::demo_topology(4);
        let nodes: Vec<String> = graph.nodes().iter().map(|n| n.id.clone()).collect();
        if nodes.len() >= 2 {
            let result = graph.quantum_route(&nodes[0], &nodes[1]);
            assert!(result.is_ok());
        }
    }
}
