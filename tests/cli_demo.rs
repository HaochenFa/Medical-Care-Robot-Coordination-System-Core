//! CLI integration tests for the demo mode.

use std::process::Command;

fn run_demo_stdout() -> String {
    let bin = env!("CARGO_BIN_EXE_project_blaze");
    let output = Command::new(bin)
        .output()
        .expect("failed to run demo binary");
    assert!(
        output.status.success(),
        "demo exited with non-zero status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn demo_cli_reports_offline_and_no_zone_violation() {
    let stdout = run_demo_stdout();
    assert!(
        stdout.contains("DEMO SUMMARY"),
        "demo summary missing from output"
    );

    // Ensure the demo reports no zone exclusivity violations.
    let zone_line = stdout
        .lines()
        .find(|line| line.starts_with("zone_violation="))
        .expect("zone_violation line missing");
    assert_eq!(zone_line.trim(), "zone_violation=false");

    let target_line = stdout
        .lines()
        .find(|line| line.starts_with("offline_target="))
        .expect("offline_target line missing");
    assert_eq!(target_line.trim(), "offline_target=1");

    let detected_line = stdout
        .lines()
        .find(|line| line.starts_with("offline_target_detected="))
        .expect("offline_target_detected line missing");
    assert_eq!(detected_line.trim(), "offline_target_detected=true");

    // The demo intentionally marks the fixed target robot as offline.
    let offline_line = stdout
        .lines()
        .find(|line| line.starts_with("offline_robots="))
        .expect("offline_robots line missing");
    assert!(offline_line.contains('1'));
}

#[test]
fn demo_cli_deterministically_marks_fixed_target_offline() {
    // Run the demo a few times to reduce the chance of timing-related false confidence.
    for _ in 0..3 {
        let stdout = run_demo_stdout();
        let detected_line = stdout
            .lines()
            .find(|line| line.starts_with("offline_target_detected="))
            .expect("offline_target_detected line missing");
        assert_eq!(detected_line.trim(), "offline_target_detected=true");
        let offline_line = stdout
            .lines()
            .find(|line| line.starts_with("offline_robots="))
            .expect("offline_robots line missing");
        assert!(offline_line.contains('1'));
    }
}
