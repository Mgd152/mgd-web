//! Black-box tests of the `keymap-sim` binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

fn sim(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_keymap-sim"))
        .args(args)
        .output()
        .unwrap()
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn tmp(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("keymap-sim-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    fs::write(&p, content).unwrap();
    p
}

#[test]
fn validate_ok() {
    let f = data("valid/full-portrait.json");
    let o = sim(&["validate", f.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(s(&o.stdout), "OK: Full (portrait, 4 controls)\n");
}

#[test]
fn validate_fails_with_paths_on_stderr() {
    let f = data("invalid/duplicate-keys.json");
    let o = sim(&["validate", f.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    let err = s(&o.stderr);
    assert!(
        err.contains("error: controls[1].key: key \"Space\" is already used by controls[0].key"),
        "{err}"
    );
    let f = data("invalid/not-json.json");
    let o = sim(&["validate", f.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(s(&o.stderr).contains("invalid JSON"));
    let o = sim(&["validate", "/nonexistent/profile.json"]);
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn keys_lists_everything() {
    let o = sim(&["keys"]);
    assert_eq!(o.status.code(), Some(0));
    let out = s(&o.stdout);
    assert_eq!(out.lines().count(), 116 + 5);
    assert!(out.lines().any(|l| l == "KeyA"));
    assert!(out.lines().last() == Some("MouseForward"));
}

#[test]
fn simulate_matches_golden() {
    let o = sim(&[
        "simulate",
        data("sim/profile.json").to_str().unwrap(),
        data("sim/script.txt").to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", s(&o.stderr));
    let got = s(&o.stdout);
    let want = fs::read_to_string(data("sim/golden.txt")).unwrap();
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {}", i + 1);
    }
    assert_eq!(got.lines().count(), want.lines().count());
}

#[test]
fn simulate_rotation_and_panel_flags() {
    let script = tmp(
        "rot.txt",
        "press Backquote\nrelease Backquote\npress Space\n",
    );
    let prof = data("valid/landscape.json");
    // Landscape profile defaults to R90 and activates.
    let o = sim(&[
        "simulate",
        prof.to_str().unwrap(),
        script.to_str().unwrap(),
        "--panel",
        "750x1334",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", s(&o.stderr));
    assert!(s(&o.stdout).contains("# activated"));
    // Forcing R0 on a landscape profile fails to activate.
    let o = sim(&[
        "simulate",
        prof.to_str().unwrap(),
        script.to_str().unwrap(),
        "--rotation",
        "R0",
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert!(s(&o.stdout).contains("# activate-failed:"));
}

#[test]
fn simulate_disconnect_and_rotate() {
    let script = tmp(
        "dc.txt",
        "press Backquote\nrelease Backquote\npress Space\nrotate R180\nrelease Space\npress Space\nrotate R90\ndisconnect\n",
    );
    let o = sim(&[
        "simulate",
        data("valid/full-portrait.json").to_str().unwrap(),
        script.to_str().unwrap(),
    ]);
    // full-portrait uses F2 as toggle, so Backquote is not the toggle there.
    assert_eq!(o.status.code(), Some(0));
    assert!(!s(&o.stdout).contains("# activated"));
    let script = tmp(
        "dc2.txt",
        "press F2\nrelease F2\npress Space\nrotate R180\nrelease Space\npress Space\nrotate R90\ndisconnect\n",
    );
    let o = sim(&[
        "simulate",
        data("valid/full-portrait.json").to_str().unwrap(),
        script.to_str().unwrap(),
    ]);
    let out = s(&o.stdout);
    assert!(out.contains("# deactivated"), "{out}");
    assert!(out.contains("# disconnected: 0 contacts dropped"), "{out}");
    assert!(out.contains("active=false contacts_down=0"));
}

#[test]
fn simulate_bad_script_line_reports_number() {
    for (script, needle) in [
        ("press KeyW\nbogus line\n", "script line 2"),
        ("wait abc\n", "script line 1"),
        ("# c\n\npress NotAKey\n", "script line 3"),
        ("rotate R45\n", "script line 1"),
        ("move 1\n", "script line 1"),
    ] {
        let sc = tmp("bad.txt", script);
        let o = sim(&[
            "simulate",
            data("sim/profile.json").to_str().unwrap(),
            sc.to_str().unwrap(),
        ]);
        assert_eq!(o.status.code(), Some(1), "{script}");
        let err = s(&o.stderr);
        assert!(err.starts_with("error: ") && err.contains(needle), "{err}");
    }
}

#[test]
fn usage_errors_exit_2() {
    assert_eq!(sim(&[]).status.code(), Some(2));
    assert_eq!(sim(&["bogus"]).status.code(), Some(2));
    assert_eq!(sim(&["validate"]).status.code(), Some(2));
    assert_eq!(sim(&["simulate", "a.json"]).status.code(), Some(2));
    let o = sim(&["simulate", "a", "b", "--nope"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(s(&o.stderr).contains("usage:"));
}
