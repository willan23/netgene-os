//! WebAssembly Sandbox & Gene Module Verifier.
//!
//! A [`GeneModule`] is a WASM binary signed (Ed25519) by a Gene. The sandbox
//! only runs modules whose author key is in its trust list and whose
//! signature covers the exact bytes being executed.
//!
//! ## Module ABI (v1)
//! The module must export `run: () -> i32` and import nothing: no host
//! functions, no WASI, so no filesystem, network or clock access. Execution
//! is bounded by fuel (CPU) and by a memory limit.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use anyhow::{Context, Result};
use tracing::info;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha3::{Digest, Sha3_256};
use wasmtime::{Config, Engine, Instance, Module, Store, StoreLimits, StoreLimitsBuilder};
use netgene_gene::crypto::{verify_signature, GeneKeyPair};
use netgene_gene::NetGene;

const MAX_MODULE_BYTES: usize = 10 * 1024 * 1024;
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const SIGNING_DOMAIN: &[u8] = b"netgene-wasm-module-v1\0";

/// Default CPU budget for one execution.
pub const DEFAULT_FUEL: u64 = 10_000_000;

/// Signed WASM Gene Module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneModule {
    pub module_id: Uuid,
    pub name: String,
    pub version: String,
    pub author_gene_id: String,
    /// Author's Ed25519 public key (base64).
    pub author_public_key_b64: String,
    /// Ed25519 signature (base64) over [`GeneModule::signing_payload`].
    pub signature_b64: String,
    pub wasm_bytes_b64: String,
    pub created_at: DateTime<Utc>,
}

impl GeneModule {
    /// Bytes covered by the signature: domain, id, name, version and the
    /// SHA3-256 digest of the WASM binary.
    pub fn signing_payload(module_id: Uuid, name: &str, version: &str, wasm: &[u8]) -> Vec<u8> {
        let mut payload = SIGNING_DOMAIN.to_vec();
        payload.extend_from_slice(module_id.as_bytes());
        for field in [name.as_bytes(), version.as_bytes()] {
            payload.extend_from_slice(&(field.len() as u32).to_be_bytes());
            payload.extend_from_slice(field);
        }
        payload.extend_from_slice(&Sha3_256::digest(wasm));
        payload
    }

    /// Build and sign a module with the author's Gene key.
    pub fn sign(name: &str, version: &str, wasm: &[u8], author: &NetGene, author_kp: &GeneKeyPair) -> Result<Self> {
        if author_kp.public_key_b64() != author.public_key_b64 {
            anyhow::bail!("Signing key does not belong to gene {}", author.id);
        }
        let module_id = Uuid::new_v4();
        let signature = author_kp.sign(&Self::signing_payload(module_id, name, version, wasm))?;

        Ok(Self {
            module_id,
            name: name.to_string(),
            version: version.to_string(),
            author_gene_id: author.id.to_string(),
            author_public_key_b64: author.public_key_b64.clone(),
            signature_b64: BASE64.encode(signature),
            wasm_bytes_b64: BASE64.encode(wasm),
            created_at: Utc::now(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub module_id: Uuid,
    pub execution_time_ms: u64,
    pub fuel_consumed: u64,
    /// Value returned by the module's `run` export.
    pub return_value: i32,
}

struct SandboxState {
    limits: StoreLimits,
}

pub struct WasmSandbox {
    memory_limit_bytes: usize,
    fuel: u64,
    trusted_author_keys: Vec<Vec<u8>>,
}

impl WasmSandbox {
    /// `trusted_author_keys`: raw Ed25519 public keys allowed to sign modules.
    pub fn new(memory_limit_bytes: usize, trusted_author_keys: Vec<Vec<u8>>) -> Self {
        Self { memory_limit_bytes, fuel: DEFAULT_FUEL, trusted_author_keys }
    }

    /// Override the CPU (fuel) budget per execution.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Verify the module's signature and trust, returning the WASM bytes.
    pub fn verify(&self, module: &GeneModule) -> Result<Vec<u8>> {
        let wasm_bytes = BASE64.decode(&module.wasm_bytes_b64).context("WASM payload is not valid base64")?;
        if wasm_bytes.len() > MAX_MODULE_BYTES {
            anyhow::bail!("Security Violation: WASM module size exceeds 10 MB limit");
        }
        if wasm_bytes.len() < 8 || wasm_bytes[..4] != WASM_MAGIC {
            anyhow::bail!("Corrupted WASM Module: Invalid WebAssembly magic header");
        }

        let author_key = BASE64.decode(&module.author_public_key_b64).context("Author key is not valid base64")?;
        if !self.trusted_author_keys.iter().any(|k| *k == author_key) {
            anyhow::bail!("Security Violation: module author is not trusted by this sandbox");
        }

        let signature = BASE64.decode(&module.signature_b64).context("Signature is not valid base64")?;
        let payload = GeneModule::signing_payload(module.module_id, &module.name, &module.version, &wasm_bytes);
        if !verify_signature(&author_key, &payload, &signature) {
            anyhow::bail!("Security Violation: invalid module signature");
        }

        Ok(wasm_bytes)
    }

    /// Verify, then execute the module's `run` export inside the sandbox.
    pub fn execute(&self, module: &GeneModule) -> Result<ExecutionResult> {
        info!("🧬 Verifying WASM Gene Module: '{}' ({})", module.name, module.module_id);
        let wasm_bytes = self.verify(module)?;

        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| anyhow::anyhow!("Failed to init wasmtime: {}", e))?;
        let compiled = Module::new(&engine, &wasm_bytes).map_err(|e| anyhow::anyhow!("WASM compilation failed: {}", e))?;

        if compiled.imports().len() > 0 {
            anyhow::bail!("Security Violation: module imports host functions, which ABI v1 does not grant");
        }

        let limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_limit_bytes)
            .instances(1)
            .memories(1)
            .tables(1)
            .build();
        let mut store = Store::new(&engine, SandboxState { limits });
        store.limiter(|state| &mut state.limits);
        store.set_fuel(self.fuel).map_err(|e| anyhow::anyhow!("Failed to set fuel: {}", e))?;

        let start = std::time::Instant::now();
        let instance = Instance::new(&mut store, &compiled, &[])
            .map_err(|e| anyhow::anyhow!("WASM instantiation failed: {}", e))?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .map_err(|e| anyhow::anyhow!("Module must export `run: () -> i32`: {}", e))?;
        let return_value = run
            .call(&mut store, ())
            .map_err(|e| anyhow::anyhow!("WASM execution trapped: {}", e))?;

        let remaining = store.get_fuel().unwrap_or(0);
        let result = ExecutionResult {
            module_id: module.module_id,
            execution_time_ms: start.elapsed().as_millis() as u64,
            fuel_consumed: self.fuel.saturating_sub(remaining),
            return_value,
        };
        info!("✅ WASM Module '{}' returned {}", module.name, return_value);
        Ok(result)
    }

    pub fn memory_limit_bytes(&self) -> usize {
        self.memory_limit_bytes
    }

    pub fn fuel(&self) -> u64 {
        self.fuel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn author() -> (NetGene, GeneKeyPair) {
        NetGene::generate_master("wasm-author").unwrap()
    }

    fn sandbox_trusting(gene: &NetGene) -> WasmSandbox {
        WasmSandbox::new(1024 * 1024, vec![gene.public_key_bytes().unwrap()])
    }

    fn wasm(src: &str) -> Vec<u8> {
        wat::parse_str(src).unwrap()
    }

    const RETURNS_42: &str = r#"(module (func (export "run") (result i32) i32.const 42))"#;

    #[test]
    fn test_signed_module_executes() {
        let (gene, kp) = author();
        let module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), &gene, &kp).unwrap();

        let result = sandbox_trusting(&gene).execute(&module).unwrap();
        assert_eq!(result.return_value, 42);
        assert!(result.fuel_consumed > 0);
    }

    #[test]
    fn test_untrusted_author_rejected() {
        let (gene, kp) = author();
        let (other, _) = author();
        let module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), &gene, &kp).unwrap();

        assert!(sandbox_trusting(&other).execute(&module).is_err());
    }

    #[test]
    fn test_tampered_bytes_rejected() {
        let (gene, kp) = author();
        let mut module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), &gene, &kp).unwrap();
        module.wasm_bytes_b64 = BASE64.encode(wasm(r#"(module (func (export "run") (result i32) i32.const 7))"#));

        assert!(sandbox_trusting(&gene).execute(&module).is_err());
    }

    #[test]
    fn test_tampered_name_rejected() {
        let (gene, kp) = author();
        let mut module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), &gene, &kp).unwrap();
        module.name = "renamed".to_string();

        assert!(sandbox_trusting(&gene).execute(&module).is_err());
    }

    #[test]
    fn test_infinite_loop_runs_out_of_fuel() {
        let (gene, kp) = author();
        let looping = wasm(r#"(module (func (export "run") (result i32) (loop (br 0)) i32.const 0))"#);
        let module = GeneModule::sign("spin", "1.0.0", &looping, &gene, &kp).unwrap();

        assert!(sandbox_trusting(&gene).with_fuel(100_000).execute(&module).is_err());
    }

    #[test]
    fn test_memory_limit_enforced() {
        let (gene, kp) = author();
        // 32 pages = 2 MiB, above the 1 MiB sandbox limit.
        let hungry = wasm(r#"(module (memory 32) (func (export "run") (result i32) i32.const 0))"#);
        let module = GeneModule::sign("hungry", "1.0.0", &hungry, &gene, &kp).unwrap();

        assert!(sandbox_trusting(&gene).execute(&module).is_err());
    }

    #[test]
    fn test_host_imports_rejected() {
        let (gene, kp) = author();
        let importing = wasm(r#"(module (import "env" "f" (func)) (func (export "run") (result i32) i32.const 0))"#);
        let module = GeneModule::sign("importer", "1.0.0", &importing, &gene, &kp).unwrap();

        assert!(sandbox_trusting(&gene).execute(&module).is_err());
    }
}
