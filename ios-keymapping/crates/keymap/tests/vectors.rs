//! Shared profile vectors (also consumed by the editor test).

use keymap::{Profile, ProfileError};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn json_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "json") && p.file_name().unwrap() != "expected.json"
        })
        .collect();
    v.sort();
    v
}

#[test]
fn valid_vectors_parse() {
    let files = json_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/valid"));
    assert!(files.len() >= 6, "need at least 6 valid vectors");
    for f in files {
        let text = fs::read_to_string(&f).unwrap();
        let p = Profile::from_json(&text).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        // Re-serialising must give an equal, valid profile.
        assert_eq!(
            Profile::from_json(&p.to_json_pretty()).unwrap(),
            p,
            "{}",
            f.display()
        );
    }
}

#[test]
fn invalid_vectors_fail_with_expected_path() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/invalid");
    let expected: BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    let files = json_files(&dir);
    assert!(files.len() >= 15, "need at least 15 invalid vectors");
    assert_eq!(
        files.len(),
        expected.len(),
        "every invalid vector needs an expected entry"
    );
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let want = expected
            .get(&name)
            .unwrap_or_else(|| panic!("{name} missing in expected.json"));
        let got = match Profile::from_json(&fs::read_to_string(&f).unwrap()) {
            Ok(_) => panic!("{name} unexpectedly valid"),
            Err(ProfileError::Json(_)) => "<json>".to_string(),
            Err(ProfileError::Invalid(p)) => p.path,
        };
        assert_eq!(&got, want, "{name}");
    }
}
