//! Kubernetes Custom Resource Definitions (CRDs) for NetGene OS.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// `GeneNode` CRD Spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneNodeSpec {
    pub gene_fingerprint: String,
    pub role: String,
    pub template: String,
    pub replicas: usize,
    pub enable_quantum: bool,
    pub enable_ebpf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneNodeStatus {
    pub phase: String,
    pub active_replicas: usize,
    pub ip_address: String,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneNodeCrd {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub spec: GeneNodeSpec,
    pub status: GeneNodeStatus,
}

/// `QuantumRoutePolicy` CRD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumRoutePolicySpec {
    pub target_ingress: String,
    pub max_latency_ms: u32,
    pub qaoa_layers: usize,
    pub auto_remediate: bool,
}

impl GeneNodeCrd {
    pub fn new_demo(name: &str, namespace: &str) -> Self {
        Self {
            api_version: "netgene.io/v1alpha1".to_string(),
            kind: "GeneNode".to_string(),
            name: name.to_string(),
            namespace: namespace.to_string(),
            spec: GeneNodeSpec {
                gene_fingerprint: "fp-k8s-master-01".to_string(),
                role: "EDGE_ROUTER".to_string(),
                template: "quantum-edge".to_string(),
                replicas: 3,
                enable_quantum: true,
                enable_ebpf: true,
            },
            status: GeneNodeStatus {
                phase: "Running".to_string(),
                active_replicas: 3,
                ip_address: "10.244.0.15".to_string(),
                last_heartbeat: Utc::now(),
            },
        }
    }

    /// Generate YAML manifest
    pub fn to_yaml_manifest(&self) -> String {
        format!(
            r#"apiVersion: netgene.io/v1alpha1
kind: GeneNode
metadata:
  name: {}
  namespace: {}
spec:
  geneFingerprint: "{}"
  role: "{}"
  template: "{}"
  replicas: {}
  enableQuantum: {}
  enableEbpf: {}
status:
  phase: "{}"
  activeReplicas: {}
  ipAddress: "{}"
"#,
            self.name,
            self.namespace,
            self.spec.gene_fingerprint,
            self.spec.role,
            self.spec.template,
            self.spec.replicas,
            self.spec.enable_quantum,
            self.spec.enable_ebpf,
            self.status.phase,
            self.status.active_replicas,
            self.status.ip_address,
        )
    }
}
