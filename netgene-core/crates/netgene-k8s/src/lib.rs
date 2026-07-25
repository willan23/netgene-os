//! # NetGene Kubernetes Operator & CRD Ecosystem (`netgene-k8s`)

pub mod crd;
pub mod controller;

pub use crd::{GeneNodeCrd, GeneNodeSpec, GeneNodeStatus};
pub use controller::K8sController;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crd_manifest_generation() {
        let crd = GeneNodeCrd::new_demo("gene-node-01", "default");
        let manifest = crd.to_yaml_manifest();
        assert!(manifest.contains("kind: GeneNode"));
        assert!(manifest.contains("name: gene-node-01"));
    }
}
