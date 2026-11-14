//! End-to-end tests of the `fixloop` binary.

#![allow(clippy::unwrap_used)] // test-only crate

use assert_cmd::Command;
use predicates::prelude::*;

fn fixloop() -> Command {
    let mut cmd = Command::cargo_bin("fixloop").unwrap();
    cmd.env_remove("FIXLOOP_CONFIG");
    cmd
}

#[test]
fn help_works() {
    fixloop()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "review, find, file, fix, check, close",
        ))
        .stdout(predicate::str::contains("init"));
}

#[test]
fn init_creates_valid_config_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    fixloop().arg("init").arg(dir.path()).assert().success();
    assert!(dir.path().join(".fixloop.yaml").is_file());
    assert!(
        dir.path()
            .join(".fixloop/rules/example-no-unwrap.md")
            .is_file()
    );

    fixloop()
        .args(["config", "validate-repo"])
        .arg(dir.path())
        .assert()
        .success();

    fixloop()
        .arg("init")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("hint: pass --force"));
}

#[test]
fn invalid_server_config_shows_hint() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("fixloop.yaml");
    std::fs::write(
        &file,
        "llm: { offline: true, providers: { c: { kind: openai } } }",
    )
    .unwrap();
    fixloop()
        .arg("--config")
        .arg(&file)
        .args(["config", "validate"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("llm.providers.c"))
        .stderr(predicate::str::contains("hint:"));
}

#[test]
fn unimplemented_command_exits_2_with_phase() {
    fixloop()
        .arg("serve")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Phase 2"));
}
