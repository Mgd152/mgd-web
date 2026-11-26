//! Sample profiles shipped in `../../profiles` (skipped when the crate is
//! copied into Mirrorly and that directory does not exist).

use keymap::{Orientation, Profile};
use std::path::Path;

#[test]
fn sample_profiles_validate_and_match_their_names() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("profiles directory not present; skipping");
        return;
    };
    for e in rd {
        let path = e.unwrap().path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let p = Profile::from_json(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        if name.contains("portrait") {
            assert_eq!(p.orientation, Orientation::Portrait, "{name}");
        }
        if name.contains("landscape") {
            assert_eq!(p.orientation, Orientation::Landscape, "{name}");
        }
    }
}
