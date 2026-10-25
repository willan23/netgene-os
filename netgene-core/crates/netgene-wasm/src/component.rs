//! Component Model agents (`wit/agent.wit`, world `netgene:agent/agent`).
//!
//! Same guarantees as the core-module ABI: only the interfaces of granted
//! permissions are linked, any other import is refused before instantiation,
//! and fuel, memory and quotas are enforced.

use std::collections::{BTreeMap, BTreeSet};
use anyhow::Result;
use chrono::Utc;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store, StoreLimits, StoreLimitsBuilder};

use crate::sandbox::{MAX_KV_KEYS, MAX_KV_KEY_BYTES, MAX_KV_VALUE_BYTES, MAX_LOG_BYTES, MAX_LOG_ENTRIES, MAX_OUTPUT_BYTES};

/// WIT package of the agent world.
pub const WIT_PACKAGE: &str = "netgene:agent@0.1.0";
/// The world definition, for guest toolchains.
pub const AGENT_WIT: &str = include_str!("../wit/agent.wit");

/// Interface name → permission.
const INTERFACES: &[(&str, &str)] = &[
    ("netgene:agent/log@0.1.0", "log"),
    ("netgene:agent/io@0.1.0", "io"),
    ("netgene:agent/kv@0.1.0", "kv"),
    ("netgene:agent/clock@0.1.0", "clock"),
];

/// Core instances, tables and memories a component may create (its own
/// module plus the adapter shims a component toolchain generates).
const MAX_INSTANCES: usize = 16;
const MAX_TABLES: usize = 8;
const MAX_MEMORIES: usize = 2;

/// Whether `bytes` is a component rather than a core module (binary layer 1).
pub fn is_component(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[6..8] == [1, 0]
}

pub(crate) struct ComponentState {
    limits: StoreLimits,
    pub input: Vec<u8>,
    pub output: Vec<u8>,
    pub logs: Vec<String>,
    pub kv: BTreeMap<String, Vec<u8>>,
}

pub(crate) struct ComponentRun {
    pub return_value: i32,
    pub fuel_remaining: u64,
    pub state: ComponentState,
}

fn link(linker: &mut Linker<ComponentState>, granted: &BTreeSet<String>) -> Result<()> {
    if granted.contains("log") {
        linker.instance("netgene:agent/log@0.1.0")?.func_wrap("log", |mut store, (message,): (String,)| {
            let state = store.data_mut();
            if state.logs.len() < MAX_LOG_ENTRIES {
                let mut message = message;
                if message.len() > MAX_LOG_BYTES {
                    let mut cut = MAX_LOG_BYTES;
                    while !message.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    message.truncate(cut);
                }
                state.logs.push(message);
            }
            Ok(())
        })?;
    }
    if granted.contains("io") {
        let mut io = linker.instance("netgene:agent/io@0.1.0")?;
        io.func_wrap("input", |store, (): ()| Ok((store.data().input.clone(),)))?;
        io.func_wrap("output", |mut store, (data,): (Vec<u8>,)| {
            let state = store.data_mut();
            if state.output.len() + data.len() > MAX_OUTPUT_BYTES {
                return Ok((false,));
            }
            state.output.extend_from_slice(&data);
            Ok((true,))
        })?;
    }
    if granted.contains("kv") {
        let mut kv = linker.instance("netgene:agent/kv@0.1.0")?;
        kv.func_wrap("get", |store, (key,): (String,)| Ok((store.data().kv.get(&key).cloned(),)))?;
        kv.func_wrap("set", |mut store, (key, value): (String, Vec<u8>)| {
            let kv = &mut store.data_mut().kv;
            let too_big = key.len() > MAX_KV_KEY_BYTES || value.len() > MAX_KV_VALUE_BYTES;
            if too_big || (!kv.contains_key(&key) && kv.len() >= MAX_KV_KEYS) {
                return Ok((false,));
            }
            kv.insert(key, value);
            Ok((true,))
        })?;
    }
    if granted.contains("clock") {
        linker
            .instance("netgene:agent/clock@0.1.0")?
            .func_wrap("now-ms", |_store, (): ()| Ok((Utc::now().timestamp_millis(),)))?;
    }
    Ok(())
}

/// Compile, check imports against `granted`, and call `run`.
pub(crate) fn run(
    engine: &Engine,
    bytes: &[u8],
    granted: &BTreeSet<String>,
    memory_limit_bytes: usize,
    fuel: u64,
    input: Vec<u8>,
    kv: BTreeMap<String, Vec<u8>>,
) -> Result<ComponentRun> {
    let component = Component::new(engine, bytes).map_err(|e| anyhow::anyhow!("Component compilation failed: {}", e))?;

    for (name, _) in component.component_type().imports(engine) {
        let permission = INTERFACES.iter().find(|(i, _)| *i == name).map(|(_, p)| *p);
        match permission {
            Some(p) if granted.contains(p) => {}
            Some(p) => anyhow::bail!("Security Violation: component imports {} without a declared permission (needs '{}')", name, p),
            None => anyhow::bail!("Security Violation: component imports {}, which NetGene does not provide (see {})", name, WIT_PACKAGE),
        }
    }

    let mut linker = Linker::new(engine);
    link(&mut linker, granted)?;

    let limits = StoreLimitsBuilder::new()
        .memory_size(memory_limit_bytes)
        .instances(MAX_INSTANCES)
        .tables(MAX_TABLES)
        .memories(MAX_MEMORIES)
        .build();
    let mut store = Store::new(engine, ComponentState { limits, input, output: vec![], logs: vec![], kv });
    store.limiter(|state| &mut state.limits);
    store.set_fuel(fuel).map_err(|e| anyhow::anyhow!("Failed to set fuel: {}", e))?;

    let instance = linker
        .instantiate(&mut store, &component)
        .map_err(|e| anyhow::anyhow!("Component instantiation failed: {}", e))?;
    let run = instance
        .get_typed_func::<(), (i32,)>(&mut store, "run")
        .map_err(|e| anyhow::anyhow!("Component must export `run: func() -> s32`: {}", e))?;
    let (return_value,) = run.call(&mut store, ()).map_err(|e| anyhow::anyhow!("WASM execution trapped: {}", e))?;
    run.post_return(&mut store)?;

    let fuel_remaining = store.get_fuel().unwrap_or(0);
    Ok(ComponentRun { return_value, fuel_remaining, state: store.into_data() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExecutionInput, GeneModule, WasmSandbox};
    use netgene_gene::{HybridPublicKey, NetGene};

    /// Logs, echoes its input and counts its runs in the KV store, through the
    /// canonical ABI of `wit/agent.wit` (what a guest toolchain would emit).
    const AGENT_CORE: &str = r#"(module
        (import "netgene:agent/log@0.1.0" "log" (func $log (param i32 i32)))
        (import "netgene:agent/io@0.1.0" "input" (func $input (param i32)))
        (import "netgene:agent/io@0.1.0" "output" (func $output (param i32 i32) (result i32)))
        (import "netgene:agent/kv@0.1.0" "get" (func $get (param i32 i32 i32)))
        (import "netgene:agent/kv@0.1.0" "set" (func $set (param i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (global $heap (mut i32) (i32.const 1024))
        (data (i32.const 0) "component says hi")
        (data (i32.const 32) "runs")
        (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
            (local $p i32)
            (local.set $p (i32.and (i32.add (global.get $heap) (i32.sub (local.get 2) (i32.const 1)))
                                   (i32.sub (i32.const 0) (local.get 2))))
            (global.set $heap (i32.add (local.get $p) (local.get 3)))
            (local.get $p))
        (func (export "run") (result i32)
            (local $n i32)
            (call $log (i32.const 0) (i32.const 17))
            (call $input (i32.const 100))
            (drop (call $output (i32.load (i32.const 100)) (i32.load (i32.const 104))))
            (call $get (i32.const 32) (i32.const 4) (i32.const 200))
            (if (i32.load8_u (i32.const 200))
                (then (local.set $n (i32.load (i32.load (i32.const 204))))))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (i32.store (i32.const 300) (local.get $n))
            (drop (call $set (i32.const 32) (i32.const 4) (i32.const 300) (i32.const 4)))
            (local.get $n)))"#;

    fn componentize(core_wat: &str) -> Vec<u8> {
        let mut module = wat::parse_str(core_wat).unwrap();
        let mut resolve = wit_parser::Resolve::default();
        let pkg = resolve.push_str("agent.wit", AGENT_WIT).unwrap();
        let world = resolve.select_world(pkg, Some("agent")).unwrap();
        wit_component::embed_component_metadata(&mut module, &resolve, world, wit_component::StringEncoding::UTF8).unwrap();
        wit_component::ComponentEncoder::default().module(&module).unwrap().validate(true).encode().unwrap()
    }

    fn signed(perms: &[&str]) -> (GeneModule, HybridPublicKey) {
        let (gene, kp) = NetGene::generate_master("component-author").unwrap();
        let bytes = componentize(AGENT_CORE);
        assert!(is_component(&bytes));
        let module = GeneModule::sign("counter", "1.0.0", &bytes, perms.iter().map(|p| p.to_string()).collect(), &gene, &kp).unwrap();
        (module, HybridPublicKey::from_gene(&gene))
    }

    #[test]
    fn test_component_agent_runs_with_typed_interfaces() {
        let (module, key) = signed(&["log", "io", "kv"]);
        let sandbox = WasmSandbox::new(16 << 20, vec![key]).allow_permissions(["log", "io", "kv"]);

        let first = sandbox.execute(&module, ExecutionInput { input: b"hello component".to_vec(), kv: Default::default() }).unwrap();
        assert_eq!(first.return_value, 1);
        assert_eq!(first.output, b"hello component");
        assert_eq!(first.logs, vec!["component says hi".to_string()]);
        assert!(first.fuel_consumed > 0);

        let second = sandbox.execute(&module, ExecutionInput { input: vec![], kv: first.kv }).unwrap();
        assert_eq!(second.return_value, 2, "KV handed back in is seen by the next run");
    }

    #[test]
    fn test_component_needs_every_imported_permission() {
        // Signed without `kv` although it imports the kv interface.
        let (module, key) = signed(&["log", "io"]);
        let sandbox = WasmSandbox::new(16 << 20, vec![key]).allow_permissions(["log", "io"]);
        let err = sandbox.execute(&module, ExecutionInput::default()).unwrap_err().to_string();
        assert!(err.contains("kv"), "{err}");
    }

    #[test]
    fn test_component_fuel_is_enforced() {
        let spin = r#"(module
            (memory (export "memory") 1)
            (func (export "run") (result i32) (loop $l (br $l)) (i32.const 0)))"#;
        let (gene, kp) = NetGene::generate_master("spinner").unwrap();
        let module = GeneModule::sign("spin", "1.0.0", &componentize(spin), vec![], &gene, &kp).unwrap();
        let sandbox = WasmSandbox::new(16 << 20, vec![HybridPublicKey::from_gene(&gene)]).with_fuel(100_000);
        assert!(sandbox.execute(&module, ExecutionInput::default()).is_err(), "infinite loop runs out of fuel");
    }
}
