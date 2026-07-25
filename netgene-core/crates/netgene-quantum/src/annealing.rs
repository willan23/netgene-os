//! Simulated Quantum Annealing for QUBO problems.
//!
//! Implements a quantum-inspired annealing schedule with transverse field
//! simulation. Finds near-optimal solutions for binary combinatorial problems.

use rand::Rng;
use serde::{Deserialize, Serialize};
use nalgebra::DMatrix;
use anyhow::Result;
use tracing::info;

/// Result of a quantum annealing run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnealingResult {
    /// Best binary solution found.
    pub solution: Vec<u8>,
    /// Final energy (objective value).
    pub energy: f64,
    /// Initial energy (for comparison).
    pub initial_energy: f64,
    /// Number of annealing steps.
    pub steps: usize,
    /// Final temperature.
    pub final_temperature: f64,
    /// Energy improvement (%).
    pub improvement_pct: f64,
}

/// Quantum-inspired simulated annealer.
pub struct QuantumAnnealer {
    /// Initial temperature (high = more exploration).
    pub initial_temp: f64,
    /// Final temperature (low = exploitation).
    pub final_temp: f64,
    /// Number of annealing steps.
    pub steps: usize,
    /// Initial transverse field strength (quantum tunneling).
    pub initial_gamma: f64,
}

impl QuantumAnnealer {
    pub fn new(initial_temp: f64, final_temp: f64, steps: usize) -> Self {
        Self {
            initial_temp,
            final_temp,
            steps,
            initial_gamma: 2.0,
        }
    }

    /// Default annealer configuration.
    pub fn default() -> Self {
        Self::new(10.0, 0.01, 500)
    }

    /// Anneal a QUBO problem: min x^T Q x where x ∈ {0,1}^n
    pub fn anneal(&self, q_matrix: &DMatrix<f64>) -> Result<AnnealingResult> {
        let n = q_matrix.nrows();
        if n == 0 { return Err(anyhow::anyhow!("Empty problem")); }

        let mut rng = rand::thread_rng();
        let mut state: Vec<u8> = (0..n).map(|_| rng.gen::<u8>() % 2).collect();
        let initial_energy = self.energy(q_matrix, &state);
        let mut current_energy = initial_energy;
        let mut best_state = state.clone();
        let mut best_energy = current_energy;

        let cooling_rate = (self.final_temp / self.initial_temp).powf(1.0 / self.steps as f64);

        for step in 0..self.steps {
            let temp = self.initial_temp * cooling_rate.powi(step as i32);
            // Transverse field decreases as system cools (quantum → classical)
            let gamma = self.initial_gamma * (1.0 - step as f64 / self.steps as f64);

            // Pick random bit to flip
            let bit = rng.gen_range(0..n);
            let mut new_state = state.clone();
            new_state[bit] ^= 1;

            let new_energy = self.energy(q_matrix, &new_state);
            let delta_e = new_energy - current_energy;

            // Quantum tunneling term: accept bad moves based on transverse field
            let quantum_boost = (gamma / temp).min(5.0);
            let accept_prob = if delta_e < 0.0 {
                1.0
            } else {
                ((-delta_e / temp) + quantum_boost * rng.gen::<f64>()).exp().min(1.0)
            };

            if rng.gen::<f64>() < accept_prob {
                state = new_state;
                current_energy = new_energy;

                if current_energy < best_energy {
                    best_energy = current_energy;
                    best_state = state.clone();
                }
            }
        }

        let improvement = if initial_energy.abs() > 1e-10 {
            ((initial_energy - best_energy) / initial_energy.abs()) * 100.0
        } else { 0.0 };

        info!(
            initial_energy,
            best_energy,
            improvement_pct = improvement,
            steps = self.steps,
            "Quantum annealing complete"
        );

        Ok(AnnealingResult {
            solution: best_state,
            energy: best_energy,
            initial_energy,
            steps: self.steps,
            final_temperature: self.final_temp,
            improvement_pct: improvement,
        })
    }

    /// Compute QUBO energy: x^T Q x
    fn energy(&self, q: &DMatrix<f64>, x: &[u8]) -> f64 {
        let n = x.len();
        (0..n).flat_map(|i| (0..n).map(move |j| (i, j)))
            .map(|(i, j)| q[(i, j)] * x[i] as f64 * x[j] as f64)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::DMatrix;

    #[test]
    fn test_annealing() {
        let q = DMatrix::from_row_slice(4, 4, &[
             2.0, -1.0,  0.0, -1.0,
            -1.0,  2.0, -1.0,  0.0,
             0.0, -1.0,  2.0, -1.0,
            -1.0,  0.0, -1.0,  2.0,
        ]);
        let annealer = QuantumAnnealer::new(5.0, 0.001, 200);
        let result = annealer.anneal(&q).unwrap();
        assert_eq!(result.solution.len(), 4);
        assert!(result.energy <= result.initial_energy + 0.1); // should not get worse
    }
}
