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

/// Strip ANSI escape codes from a string for plain-text matching.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // consume until 'm'
            for ch in chars.by_ref() {
                if ch == 'm' { break; }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[test]
fn demo_cli_reports_offline_and_no_zone_violation() {
    let stdout = run_demo_stdout();
    let plain = strip_ansi(&stdout);

    assert!(
        plain.contains("DEMO SUMMARY"),
        "demo summary missing from output"
    );

    // Ensure the demo reports no zone exclusivity violations.
    let zone_line = plain
        .lines()
        .find(|line| line.contains("zone_violation"))
        .expect("zone_violation line missing");
    assert!(zone_line.contains("false"), "expected zone_violation=false, got: {zone_line}");

    let target_line = plain
        .lines()
        .find(|line| line.contains("offline_target") && !line.contains("detected"))
        .expect("offline_target line missing");
    assert!(target_line.contains('1'), "expected offline_target=1, got: {target_line}");

    let detected_line = plain
        .lines()
        .find(|line| line.contains("detected"))
        .expect("offline_target_detected line missing");
    assert!(detected_line.contains("true"), "expected detected=true, got: {detected_line}");

    // The demo intentionally marks the fixed target robot as offline.
    let offline_line = plain
        .lines()
        .find(|line| line.contains("offline_robots"))
        .expect("offline_robots line missing");
    assert!(offline_line.contains('1'), "expected offline_robots to contain 1, got: {offline_line}");
}

#[test]
fn demo_cli_deterministically_marks_fixed_target_offline() {
    // Run the demo a few times to reduce the chance of timing-related false confidence.
    for _ in 0..3 {
        let stdout = run_demo_stdout();
        let plain = strip_ansi(&stdout);

        let detected_line = plain
            .lines()
            .find(|line| line.contains("detected"))
            .expect("offline_target_detected line missing");
        assert!(detected_line.contains("true"), "expected detected=true, got: {detected_line}");

        let offline_line = plain
            .lines()
            .find(|line| line.contains("offline_robots"))
            .expect("offline_robots line missing");
        assert!(offline_line.contains('1'), "expected offline_robots to contain 1, got: {offline_line}");
    }
}
