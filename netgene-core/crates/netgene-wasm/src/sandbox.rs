//! WebAssembly Sandbox & Gene Module Verifier.
//!
//! A [`GeneModule`] is a WASM binary signed by a Gene with a hybrid
//! (Ed25519 + ML-DSA-65) signature. The sandbox only runs modules whose
//! author key exactly matches a trusted key and whose signature covers the
//! exact bytes and the declared permissions.
//!
//! ## Module ABI (v2)
//! The module exports `run: () -> i32` and may import host functions from
//! the `netgene` namespace, but **only those of permissions it declared in
//! its signed manifest and that the sandbox allows**:
//!
//! | Permission | Imports |
//! |---|---|
//! | `log` | `log(ptr: i32, len: i32)` |
//! | `io` | `input_len() -> i32`, `input_read(ptr: i32, cap: i32) -> i32`, `output_write(ptr: i32, len: i32) -> i32` |
//! | `kv` | `kv_get(kptr, klen, vptr, vcap) -> i32`, `kv_set(kptr, klen, vptr, vlen) -> i32` |
//! | `clock` | `now_ms() -> i64` |
//!
//! There is no WASI, no filesystem and no network. Execution is bounded by
//! fuel (CPU), a memory limit and per-permission quotas.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use anyhow::{Context, Result};
use tracing::info;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha3::{Digest, Sha3_256};
use wasmtime::{Caller, Config, Engine, Extern, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::{verify_hybrid, HybridPublicKey, HybridSignature, NetGene};

const MAX_MODULE_BYTES: usize = 10 * 1024 * 1024;
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const SIGNING_DOMAIN: &[u8] = b"netgene-wasm-module-v3\0";
const HOST_NAMESPACE: &str = "netgene";

const MAX_LOG_ENTRIES: usize = 100;
const MAX_LOG_BYTES: usize = 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_KV_KEYS: usize = 256;
const MAX_KV_KEY_BYTES: usize = 256;
const MAX_KV_VALUE_BYTES: usize = 4 * 1024;

/// Default CPU budget for one execution.
pub const DEFAULT_FUEL: u64 = 10_000_000;

/// Stack of the thread that runs guest code.
const WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

/// Every permission a module may declare, with the host functions it unlocks.
pub const PERMISSIONS: &[(&str, &[&str])] = &[
    ("log", &["log"]),
    ("io", &["input_len", "input_read", "output_write"]),
    ("kv", &["kv_get", "kv_set"]),
    ("clock", &["now_ms"]),
];

fn permission_for(function: &str) -> Option<&'static str> {
    PERMISSIONS.iter().find(|(_, fns)| fns.contains(&function)).map(|(p, _)| *p)
}

/// Signed WASM Gene Module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneModule {
    pub module_id: Uuid,
    pub name: String,
    pub version: String,
    pub author_gene_id: String,
    /// Host permissions the module needs (covered by the signature).
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Author's public key (Ed25519 + optional ML-DSA-65).
    pub author_key: HybridPublicKey,
    /// Hybrid signature over [`GeneModule::signing_payload`].
    pub signature: HybridSignature,
    pub wasm_bytes_b64: String,
    pub created_at: DateTime<Utc>,
}

impl GeneModule {
    /// Bytes covered by the signature: domain, id, name, version, the sorted
    /// permissions and the SHA3-256 digest of the WASM binary.
    pub fn signing_payload(module_id: Uuid, name: &str, version: &str, permissions: &[String], wasm: &[u8]) -> Vec<u8> {
        fn push(buf: &mut Vec<u8>, field: &[u8]) {
            buf.extend_from_slice(&(field.len() as u32).to_be_bytes());
            buf.extend_from_slice(field);
        }
        let mut payload = SIGNING_DOMAIN.to_vec();
        payload.extend_from_slice(module_id.as_bytes());
        push(&mut payload, name.as_bytes());
        push(&mut payload, version.as_bytes());
        let sorted: BTreeSet<&str> = permissions.iter().map(String::as_str).collect();
        payload.extend_from_slice(&(sorted.len() as u32).to_be_bytes());
        for p in sorted {
            push(&mut payload, p.as_bytes());
        }
        payload.extend_from_slice(&Sha3_256::digest(wasm));
        payload
    }

    /// Build and sign a module with the author's Gene key.
    pub fn sign(
        name: &str,
        version: &str,
        wasm: &[u8],
        permissions: Vec<String>,
        author: &NetGene,
        author_kp: &GeneKeyPair,
    ) -> Result<Self> {
        if author_kp.public_key_b64() != author.public_key_b64 {
            anyhow::bail!("Signing key does not belong to gene {}", author.id);
        }
        if let Some(unknown) = permissions.iter().find(|p| !PERMISSIONS.iter().any(|(k, _)| k == p)) {
            anyhow::bail!("Unknown permission '{}' (known: log, io, kv, clock)", unknown);
        }
        let module_id = Uuid::new_v4();
        let signature = author_kp.sign_hybrid(&Self::signing_payload(module_id, name, version, &permissions, wasm))?;

        Ok(Self {
            module_id,
            name: name.to_string(),
            version: version.to_string(),
            author_gene_id: author.id.to_string(),
            permissions,
            author_key: HybridPublicKey::from_gene(author),
            signature,
            wasm_bytes_b64: BASE64.encode(wasm),
            created_at: Utc::now(),
        })
    }
}

/// Data handed to a module (for `io` and `kv`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionInput {
    pub input: Vec<u8>,
    pub kv: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub module_id: Uuid,
    pub execution_time_ms: u64,
    pub fuel_consumed: u64,
    /// Value returned by the module's `run` export.
    pub return_value: i32,
    pub output: Vec<u8>,
    pub logs: Vec<String>,
    /// Key-value state after the run.
    pub kv: BTreeMap<String, Vec<u8>>,
}

struct SandboxState {
    limits: StoreLimits,
    input: Vec<u8>,
    output: Vec<u8>,
    logs: Vec<String>,
    kv: BTreeMap<String, Vec<u8>>,
}

fn memory(caller: &mut Caller<'_, SandboxState>) -> wasmtime::Result<wasmtime::Memory> {
    match caller.get_export("memory") {
        Some(Extern::Memory(m)) => Ok(m),
        _ => Err(wasmtime::Error::msg("module must export its memory as `memory`")),
    }
}

fn read_bytes(caller: &mut Caller<'_, SandboxState>, ptr: i32, len: i32, max: usize) -> wasmtime::Result<Vec<u8>> {
    let (ptr, len) = (usize::try_from(ptr)?, usize::try_from(len)?);
    if len > max {
        return Err(wasmtime::Error::msg("guest buffer exceeds host limit"));
    }
    let mem = memory(caller)?;
    let data = mem.data(&*caller);
    let end = ptr.checked_add(len).filter(|&e| e <= data.len()).ok_or_else(|| wasmtime::Error::msg("out-of-bounds guest pointer"))?;
    Ok(data[ptr..end].to_vec())
}

fn write_bytes(caller: &mut Caller<'_, SandboxState>, ptr: i32, bytes: &[u8]) -> wasmtime::Result<()> {
    let ptr = usize::try_from(ptr)?;
    let mem = memory(caller)?;
    let data = mem.data_mut(&mut *caller);
    let end = ptr.checked_add(bytes.len()).filter(|&e| e <= data.len()).ok_or_else(|| wasmtime::Error::msg("out-of-bounds guest pointer"))?;
    data[ptr..end].copy_from_slice(bytes);
    Ok(())
}

/// Register the host functions of `permissions` (and nothing else).
fn link(linker: &mut Linker<SandboxState>, permissions: &BTreeSet<String>) -> Result<()> {
    let wrap = |e: wasmtime::Error| anyhow::anyhow!("Failed to link host function: {}", e);
    if permissions.contains("log") {
        linker
            .func_wrap(HOST_NAMESPACE, "log", |mut caller: Caller<'_, SandboxState>, ptr: i32, len: i32| -> wasmtime::Result<()> {
                let bytes = read_bytes(&mut caller, ptr, len, MAX_LOG_BYTES)?;
                let state = caller.data_mut();
                if state.logs.len() < MAX_LOG_ENTRIES {
                    state.logs.push(String::from_utf8_lossy(&bytes).into_owned());
                }
                Ok(())
            })
            .map_err(wrap)?;
    }
    if permissions.contains("io") {
        linker
            .func_wrap(HOST_NAMESPACE, "input_len", |caller: Caller<'_, SandboxState>| -> i32 {
                caller.data().input.len() as i32
            })
            .map_err(wrap)?;
        linker
            .func_wrap(HOST_NAMESPACE, "input_read", |mut caller: Caller<'_, SandboxState>, ptr: i32, cap: i32| -> wasmtime::Result<i32> {
                let input = caller.data().input.clone();
                let n = input.len().min(usize::try_from(cap)?);
                write_bytes(&mut caller, ptr, &input[..n])?;
                Ok(n as i32)
            })
            .map_err(wrap)?;
        linker
            .func_wrap(HOST_NAMESPACE, "output_write", |mut caller: Caller<'_, SandboxState>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
                let bytes = read_bytes(&mut caller, ptr, len, MAX_OUTPUT_BYTES)?;
                let state = caller.data_mut();
                if state.output.len() + bytes.len() > MAX_OUTPUT_BYTES {
                    return Ok(-1);
                }
                state.output.extend_from_slice(&bytes);
                Ok(bytes.len() as i32)
            })
            .map_err(wrap)?;
    }
    if permissions.contains("kv") {
        linker
            .func_wrap(HOST_NAMESPACE, "kv_get", |mut caller: Caller<'_, SandboxState>, kptr: i32, klen: i32, vptr: i32, vcap: i32| -> wasmtime::Result<i32> {
                let key = String::from_utf8(read_bytes(&mut caller, kptr, klen, MAX_KV_KEY_BYTES)?)?;
                let Some(value) = caller.data().kv.get(&key).cloned() else { return Ok(-1) };
                if value.len() > usize::try_from(vcap)? {
                    return Ok(-2);
                }
                write_bytes(&mut caller, vptr, &value)?;
                Ok(value.len() as i32)
            })
            .map_err(wrap)?;
        linker
            .func_wrap(HOST_NAMESPACE, "kv_set", |mut caller: Caller<'_, SandboxState>, kptr: i32, klen: i32, vptr: i32, vlen: i32| -> wasmtime::Result<i32> {
                let key = String::from_utf8(read_bytes(&mut caller, kptr, klen, MAX_KV_KEY_BYTES)?)?;
                let value = read_bytes(&mut caller, vptr, vlen, MAX_KV_VALUE_BYTES)?;
                let kv = &mut caller.data_mut().kv;
                if !kv.contains_key(&key) && kv.len() >= MAX_KV_KEYS {
                    return Ok(-1);
                }
                kv.insert(key, value);
                Ok(0)
            })
            .map_err(wrap)?;
    }
    if permissions.contains("clock") {
        linker
            .func_wrap(HOST_NAMESPACE, "now_ms", || -> i64 { Utc::now().timestamp_millis() })
            .map_err(wrap)?;
    }
    Ok(())
}

pub struct WasmSandbox {
    memory_limit_bytes: usize,
    fuel: u64,
    trusted_author_keys: Vec<HybridPublicKey>,
    allowed_permissions: BTreeSet<String>,
}

impl WasmSandbox {
    /// `trusted_author_keys`: keys allowed to sign modules (matched exactly,
    /// including the post-quantum half). No permission is allowed by default.
    pub fn new(memory_limit_bytes: usize, trusted_author_keys: Vec<HybridPublicKey>) -> Self {
        Self { memory_limit_bytes, fuel: DEFAULT_FUEL, trusted_author_keys, allowed_permissions: BTreeSet::new() }
    }

    /// Override the CPU (fuel) budget per execution.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Permissions this sandbox may grant to modules that declare them.
    pub fn allow_permissions<I: IntoIterator<Item = S>, S: Into<String>>(mut self, permissions: I) -> Self {
        self.allowed_permissions.extend(permissions.into_iter().map(Into::into));
        self
    }

    /// Verify trust, signature and permissions, returning the WASM bytes.
    pub fn verify(&self, module: &GeneModule) -> Result<Vec<u8>> {
        let wasm_bytes = BASE64.decode(&module.wasm_bytes_b64).context("WASM payload is not valid base64")?;
        if wasm_bytes.len() > MAX_MODULE_BYTES {
            anyhow::bail!("Security Violation: WASM module size exceeds 10 MB limit");
        }
        if wasm_bytes.len() < 8 || wasm_bytes[..4] != WASM_MAGIC {
            anyhow::bail!("Corrupted WASM Module: Invalid WebAssembly magic header");
        }

        if !self.trusted_author_keys.contains(&module.author_key) {
            anyhow::bail!("Security Violation: module author is not trusted by this sandbox");
        }

        let payload = GeneModule::signing_payload(module.module_id, &module.name, &module.version, &module.permissions, &wasm_bytes);
        if !verify_hybrid(&module.author_key, &payload, &module.signature) {
            anyhow::bail!("Security Violation: invalid module signature");
        }

        if let Some(denied) = module.permissions.iter().find(|p| !self.allowed_permissions.contains(*p)) {
            anyhow::bail!("Security Violation: module requests permission '{}', which this sandbox does not grant", denied);
        }

        Ok(wasm_bytes)
    }

    /// Verify, then execute the module's `run` export inside the sandbox.
    ///
    /// Runs on a dedicated thread with a large stack: guest code uses the host
    /// thread's stack (up to wasmtime's 512 KiB `max_wasm_stack`), which would
    /// overflow a 1 MiB Windows main thread.
    pub fn execute(&self, module: &GeneModule, input: ExecutionInput) -> Result<ExecutionResult> {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("netgene-wasm".into())
                .stack_size(WORKER_STACK_BYTES)
                .spawn_scoped(scope, || self.execute_on_current_thread(module, input))
                .map_err(|e| anyhow::anyhow!("Cannot start WASM worker thread: {}", e))?
                .join()
                .map_err(|_| anyhow::anyhow!("WASM worker thread panicked"))?
        })
    }

    fn execute_on_current_thread(&self, module: &GeneModule, input: ExecutionInput) -> Result<ExecutionResult> {
        info!("🧬 Verifying WASM Gene Module: '{}' ({})", module.name, module.module_id);
        let wasm_bytes = self.verify(module)?;
        let granted: BTreeSet<String> = module.permissions.iter().cloned().collect();

        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| anyhow::anyhow!("Failed to init wasmtime: {}", e))?;
        let compiled = Module::new(&engine, &wasm_bytes).map_err(|e| anyhow::anyhow!("WASM compilation failed: {}", e))?;

        for import in compiled.imports() {
            let allowed = import.module() == HOST_NAMESPACE
                && permission_for(import.name()).is_some_and(|p| granted.contains(p));
            if !allowed {
                let needs = permission_for(import.name())
                    .map(|p| format!(" (needs permission '{}')", p))
                    .unwrap_or_default();
                anyhow::bail!("Security Violation: module imports {}.{} without a declared permission{}", import.module(), import.name(), needs);
            }
        }

        let mut linker = Linker::new(&engine);
        link(&mut linker, &granted)?;

        let limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_limit_bytes)
            .instances(1)
            .memories(1)
            .tables(1)
            .build();
        let state = SandboxState { limits, input: input.input, output: vec![], logs: vec![], kv: input.kv };
        let mut store = Store::new(&engine, state);
        store.limiter(|state| &mut state.limits);
        store.set_fuel(self.fuel).map_err(|e| anyhow::anyhow!("Failed to set fuel: {}", e))?;

        let start = std::time::Instant::now();
        let instance = linker
            .instantiate(&mut store, &compiled)
            .map_err(|e| anyhow::anyhow!("WASM instantiation failed: {}", e))?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .map_err(|e| anyhow::anyhow!("Module must export `run: () -> i32`: {}", e))?;
        let return_value = run
            .call(&mut store, ())
            .map_err(|e| anyhow::anyhow!("WASM execution trapped: {}", e))?;

        let remaining = store.get_fuel().unwrap_or(0);
        let elapsed = start.elapsed().as_millis() as u64;
        let state = store.into_data();
        info!("✅ WASM Module '{}' returned {}", module.name, return_value);
        Ok(ExecutionResult {
            module_id: module.module_id,
            execution_time_ms: elapsed,
            fuel_consumed: self.fuel.saturating_sub(remaining),
            return_value,
            output: state.output,
            logs: state.logs,
            kv: state.kv,
        })
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
        WasmSandbox::new(1024 * 1024, vec![HybridPublicKey::from_gene(gene)])
    }

    fn wasm(src: &str) -> Vec<u8> {
        wat::parse_str(src).unwrap()
    }

    fn perms(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    const RETURNS_42: &str = r#"(module (func (export "run") (result i32) i32.const 42))"#;

    fn run(src: &str, declared: &[&str], allowed: &[&str], input: ExecutionInput) -> Result<ExecutionResult> {
        let (gene, kp) = author();
        let module = GeneModule::sign("m", "1.0.0", &wasm(src), perms(declared), &gene, &kp).unwrap();
        sandbox_trusting(&gene).allow_permissions(allowed.iter().copied()).execute(&module, input)
    }

    #[test]
    fn test_signed_module_executes() {
        let result = run(RETURNS_42, &[], &[], ExecutionInput::default()).unwrap();
        assert_eq!(result.return_value, 42);
        assert!(result.fuel_consumed > 0);
    }

    #[test]
    fn test_untrusted_author_rejected() {
        let (gene, kp) = author();
        let (other, _) = author();
        let module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), vec![], &gene, &kp).unwrap();
        assert!(sandbox_trusting(&other).execute(&module, ExecutionInput::default()).is_err());
    }

    #[test]
    fn test_tampered_bytes_rejected() {
        let (gene, kp) = author();
        let mut module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), vec![], &gene, &kp).unwrap();
        module.wasm_bytes_b64 = BASE64.encode(wasm(r#"(module (func (export "run") (result i32) i32.const 7))"#));
        assert!(sandbox_trusting(&gene).execute(&module, ExecutionInput::default()).is_err());
    }

    #[test]
    fn test_tampered_name_rejected() {
        let (gene, kp) = author();
        let mut module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), vec![], &gene, &kp).unwrap();
        module.name = "renamed".to_string();
        assert!(sandbox_trusting(&gene).execute(&module, ExecutionInput::default()).is_err());
    }

    #[test]
    fn test_stripped_pq_signature_rejected() {
        let (gene, kp) = author();
        let mut module = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), vec![], &gene, &kp).unwrap();
        assert!(module.author_key.is_post_quantum());
        module.signature.ml_dsa65_b64 = None;
        assert!(sandbox_trusting(&gene).execute(&module, ExecutionInput::default()).is_err());

        let mut downgraded = GeneModule::sign("answer", "1.0.0", &wasm(RETURNS_42), vec![], &gene, &kp).unwrap();
        downgraded.author_key.ml_dsa65_b64 = None;
        assert!(sandbox_trusting(&gene).execute(&downgraded, ExecutionInput::default()).is_err());
    }

    #[test]
    fn test_infinite_loop_runs_out_of_fuel() {
        let (gene, kp) = author();
        let looping = wasm(r#"(module (func (export "run") (result i32) (loop (br 0)) i32.const 0))"#);
        let module = GeneModule::sign("spin", "1.0.0", &looping, vec![], &gene, &kp).unwrap();
        assert!(sandbox_trusting(&gene).with_fuel(100_000).execute(&module, ExecutionInput::default()).is_err());
    }

    #[test]
    fn test_memory_limit_enforced() {
        // 32 pages = 2 MiB, above the 1 MiB sandbox limit.
        let hungry = r#"(module (memory 32) (func (export "run") (result i32) i32.const 0))"#;
        assert!(run(hungry, &[], &[], ExecutionInput::default()).is_err());
    }

    const LOGGER: &str = r#"(module
        (import "netgene" "log" (func $log (param i32 i32)))
        (memory (export "memory") 1)
        (data (i32.const 0) "hello from wasm")
        (func (export "run") (result i32) (call $log (i32.const 0) (i32.const 15)) i32.const 0))"#;

    #[test]
    fn test_declared_and_allowed_permission_links() {
        let result = run(LOGGER, &["log"], &["log"], ExecutionInput::default()).unwrap();
        assert_eq!(result.logs, vec!["hello from wasm".to_string()]);
    }

    #[test]
    fn test_undeclared_import_rejected() {
        let err = run(LOGGER, &[], &["log"], ExecutionInput::default()).unwrap_err().to_string();
        assert!(err.contains("without a declared permission"), "{err}");
    }

    #[test]
    fn test_permission_not_granted_by_sandbox_rejected() {
        let err = run(LOGGER, &["log"], &[], ExecutionInput::default()).unwrap_err().to_string();
        assert!(err.contains("does not grant"), "{err}");
    }

    #[test]
    fn test_unknown_import_rejected() {
        let src = r#"(module (import "env" "open_socket" (func)) (func (export "run") (result i32) i32.const 0))"#;
        assert!(run(src, &["log", "io", "kv", "clock"], &["log", "io", "kv", "clock"], ExecutionInput::default()).is_err());
    }

    #[test]
    fn test_permissions_are_signed() {
        let (gene, kp) = author();
        let mut module = GeneModule::sign("m", "1.0.0", &wasm(LOGGER), perms(&["log"]), &gene, &kp).unwrap();
        module.permissions.push("kv".into());
        let sandbox = sandbox_trusting(&gene).allow_permissions(["log", "kv"]);
        let err = sandbox.execute(&module, ExecutionInput::default()).unwrap_err().to_string();
        assert!(err.contains("invalid module signature"), "{err}");
    }

    #[test]
    fn test_io_echo() {
        let echo = r#"(module
            (import "netgene" "input_len" (func $len (result i32)))
            (import "netgene" "input_read" (func $read (param i32 i32) (result i32)))
            (import "netgene" "output_write" (func $write (param i32 i32) (result i32)))
            (memory (export "memory") 1)
            (func (export "run") (result i32)
                (drop (call $read (i32.const 0) (call $len)))
                (call $write (i32.const 0) (call $len))))"#;
        let input = ExecutionInput { input: b"ping".to_vec(), kv: BTreeMap::new() };
        let result = run(echo, &["io"], &["io"], input).unwrap();
        assert_eq!(result.output, b"ping");
        assert_eq!(result.return_value, 4);
    }

    #[test]
    fn test_kv_roundtrip_and_bounds() {
        let kv = r#"(module
            (import "netgene" "kv_set" (func $set (param i32 i32 i32 i32) (result i32)))
            (import "netgene" "kv_get" (func $get (param i32 i32 i32 i32) (result i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "count")
            (data (i32.const 16) "7")
            (func (export "run") (result i32)
                (drop (call $set (i32.const 0) (i32.const 5) (i32.const 16) (i32.const 1)))
                (call $get (i32.const 0) (i32.const 5) (i32.const 32) (i32.const 8))))"#;
        let result = run(kv, &["kv"], &["kv"], ExecutionInput::default()).unwrap();
        assert_eq!(result.return_value, 1);
        assert_eq!(result.kv.get("count").map(Vec::as_slice), Some(&b"7"[..]));

        // A pointer outside guest memory traps instead of reading host memory.
        let oob = r#"(module
            (import "netgene" "log" (func $log (param i32 i32)))
            (memory (export "memory") 1)
            (func (export "run") (result i32) (call $log (i32.const 65530) (i32.const 100)) i32.const 0))"#;
        assert!(run(oob, &["log"], &["log"], ExecutionInput::default()).is_err());
    }

    /// Regression: a caller on a small stack (like the 1 MiB Windows main
    /// thread) must not overflow when guest code calls into the host.
    #[test]
    fn test_runs_from_small_caller_stack() {
        let (gene, kp) = author();
        let module = GeneModule::sign("m", "1.0.0", &wasm(LOGGER), perms(&["log"]), &gene, &kp).unwrap();
        let sandbox = sandbox_trusting(&gene).allow_permissions(["log"]);
        let result = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(256 * 1024)
                .spawn_scoped(scope, || sandbox.execute(&module, ExecutionInput::default()))
                .unwrap()
                .join()
                .unwrap()
        })
        .unwrap();
        assert_eq!(result.logs.len(), 1);
    }

    #[test]
    fn test_unknown_permission_cannot_be_signed() {
        let (gene, kp) = author();
        assert!(GeneModule::sign("m", "1", &wasm(RETURNS_42), perms(&["network"]), &gene, &kp).is_err());
    }
}
