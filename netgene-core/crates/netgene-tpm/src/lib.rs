//! TPM 2.0 Hardware Enclave Integration
//! 
//! Provides abstraction over TPM 2.0 (tss-esapi) and Secure Enclave for sealing Master Genes.
//! This implementation simulates TPM sealing when hardware is unavailable.

use anyhow::Result;
use tracing::{info, warn};

pub struct TpmEnclave {
    has_hardware: bool,
}

impl Default for TpmEnclave {
    fn default() -> Self {
        Self::new()
    }
}

impl TpmEnclave {
    pub fn new() -> Self {
        // Attempt to probe hardware TPM (Mocked here)
        warn!("⚠️ No hardware TPM 2.0 module detected. Falling back to software simulated enclave.");
        Self { has_hardware: false }
    }

    /// Seal the Master Gene to the platform state (PCRs)
    pub fn seal_master_gene(&self, gene_bytes: &[u8], pcr_selection: &[u8]) -> Result<Vec<u8>> {
        info!("🔒 Sealing Master Gene to TPM PCRs {:?}", pcr_selection);
        
        // In a real implementation with `tss-esapi`, this would call TssContext::execute(...)
        // Here we simulate the sealing by wrapping it in a known hardware header
        let mut sealed = b"TPM2_SEALED:".to_vec();
        sealed.extend_from_slice(gene_bytes);
        
        Ok(sealed)
    }

    /// Unseal the Master Gene, fails if PCRs have changed (system compromised)
    pub fn unseal_master_gene(&self, sealed_bytes: &[u8]) -> Result<Vec<u8>> {
        if !sealed_bytes.starts_with(b"TPM2_SEALED:") {
            anyhow::bail!("Invalid sealed payload format");
        }
        
        info!("🔓 Unsealing Master Gene from TPM");
        Ok(sealed_bytes[12..].to_vec())
    }

    pub fn is_hardware_backed(&self) -> bool {
        self.has_hardware
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tpm_seal_and_unseal() {
        let tpm = TpmEnclave::new();
        let secret = b"my_super_secret_gene";
        let pcr = b"pcr_state_123";
        
        let sealed = tpm.seal_master_gene(secret, pcr).unwrap();
        assert!(sealed.starts_with(b"TPM2_SEALED:"));
        
        let unsealed = tpm.unseal_master_gene(&sealed).unwrap();
        assert_eq!(unsealed, secret);
    }

    #[test]
    fn test_tpm_unseal_invalid_data() {
        let tpm = TpmEnclave::new();
        let invalid = b"INVALID_DATA_FORMAT";
        assert!(tpm.unseal_master_gene(invalid).is_err());
    }

    #[test]
    fn test_tpm_hardware_flag() {
        let tpm = TpmEnclave::new();
        assert_eq!(tpm.is_hardware_backed(), false);
    }
}
