use std::process::Command;

#[test]
fn test_vbri_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Vision Bridge"));
}

#[test]
fn test_vbri_init_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "init", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
}
