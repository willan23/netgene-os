use assert_cmd::Command;

/// Every top-level subcommand must be wired and answer `--help`.
#[test]
fn test_cli_subcommands_are_wired() {
    for sub in ["gene", "agent", "store", "llm", "p2p", "wasm", "vault", "ebpf"] {
        Command::cargo_bin("netgene")
            .unwrap()
            .arg(sub)
            .arg("--help")
            .assert()
            .success();
    }
}
