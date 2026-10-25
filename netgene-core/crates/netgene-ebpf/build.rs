//! Compiles the XDP program with clang when building for Linux.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=bpf/netgene_xdp.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("netgene_xdp.o");
    let clang = std::env::var("NETGENE_BPF_CLANG").unwrap_or_else(|_| "clang".into());
    let status = Command::new(&clang)
        .args(["-O2", "-g", "-target", "bpf", "-D__TARGET_ARCH_x86", "-I/usr/include/x86_64-linux-gnu", "-c", "bpf/netgene_xdp.c", "-o"])
        .arg(&out)
        .status()
        .unwrap_or_else(|e| panic!("netgene-ebpf needs clang to build the XDP program on Linux ({}): {}", clang, e));
    assert!(status.success(), "clang failed to compile bpf/netgene_xdp.c (install clang and libbpf-dev)");
}
