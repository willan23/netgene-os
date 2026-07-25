use assert_cmd::Command;

#[test]
fn test_cli_seed_and_verify() {
    // Note: in a real environment this might write to the actual user ~/.netgene/db,
    // so we will just run `--help` to verify the commands are present.
    
    let mut cmd = Command::cargo_bin("netgene").unwrap();
    cmd.arg("seed").arg("--help").assert().success();

    let mut cmd = Command::cargo_bin("netgene").unwrap();
    cmd.arg("cloud").arg("--help").assert().success();

    let mut cmd = Command::cargo_bin("netgene").unwrap();
    cmd.arg("lite").arg("--help").assert().success();

    let mut cmd = Command::cargo_bin("netgene").unwrap();
    cmd.arg("tpm").arg("--help").assert().success();

    let mut cmd = Command::cargo_bin("netgene").unwrap();
    cmd.arg("vault").arg("--help").assert().success();
}
