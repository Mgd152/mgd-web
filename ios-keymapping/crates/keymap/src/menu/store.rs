//! Profile files on disk, behind the `Fs` trait (section 6.12, 7.3).
//!
//! Layout under `dir`: `<file>.json`, `<file>.json.bak` (previous saved
//! version), `.draft/<file>.json`, `.trash/<file>.json`. Nothing here touches
//! the real file system: `Fs` is implemented by the app, and by `MemFs` in
//! tests. Paths use `/` as separator.

use super::doc::Kind;
use super::msg::{Lang, Msg, ProblemMsg, RepairField};
use super::templates::{Template, template};
use super::{RADIUS_MAX, RADIUS_MIN, REACH_MAX, REACH_MIN};
use crate::profile::{Control, Orientation, Problem, Profile, ProfileError};
use std::collections::BTreeMap;

const MAX_NAME_CHARS: usize = 100;
const MAX_LABEL_CHARS: usize = 40;
const MAX_RELEASE_MS: u32 = 60_000;
const SENS_MIN: f64 = 0.05;
const SENS_MAX: f64 = 100.0;
const MAX_SLUG: usize = 48;

// ---------------------------------------------------------------- Fs ----

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FsError {
    NotFound,
    Io(String),
}

/// The fallible operations of `Fs` (for failure injection in `MemFs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FsOp {
    List,
    Read,
    WriteAtomic,
    Rename,
    Remove,
}

pub trait Fs {
    /// Names (not paths) of the files directly inside `dir`, sorted. A
    /// missing directory is an empty list. Sub-directories are not listed.
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError>;
    fn read(&self, path: &str) -> Result<String, FsError>;
    /// Temp file + rename; creates missing parent directories. On failure
    /// the old file (if any) is unchanged.
    fn write_atomic(&mut self, path: &str, data: &str) -> Result<(), FsError>;
    /// Replaces `to` if it exists.
    fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError>;
    fn remove(&mut self, path: &str) -> Result<(), FsError>;
    fn exists(&self, path: &str) -> bool;
}

/// In-memory `Fs` for tests. Directories are implicit.
#[derive(Clone, Default, Debug)]
pub struct MemFs {
    files: BTreeMap<String, String>,
    /// (operation, how many calls of it to let through first)
    fail_next: Option<(FsOp, usize)>,
}

impl MemFs {
    pub fn new() -> MemFs {
        MemFs::default()
    }

    /// Make the next call of `op` fail with `FsError::Io` (once).
    pub fn fail_next(&mut self, op: FsOp) {
        self.fail_next = Some((op, 0));
    }

    /// Let `skip` calls of `op` succeed, then fail the one after (once).
    pub fn fail_nth(&mut self, op: FsOp, skip: usize) {
        self.fail_next = Some((op, skip));
    }

    /// Put a file in directly (tests).
    pub fn put(&mut self, path: &str, data: &str) {
        self.files.insert(path.to_string(), data.to_string());
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(|s| s.as_str())
    }

    /// Every path, sorted.
    pub fn paths(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    fn check(&mut self, op: FsOp) -> Result<(), FsError> {
        if let Some((o, skip)) = self.fail_next {
            if o == op {
                if skip == 0 {
                    self.fail_next = None;
                    return Err(FsError::Io(format!("injected {op:?} failure")));
                }
                self.fail_next = Some((o, skip - 1));
            }
        }
        Ok(())
    }
}

impl Fs for MemFs {
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        Ok(self
            .files
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix))
            .filter(|rest| !rest.contains('/'))
            .map(|s| s.to_string())
            .collect())
    }
    fn read(&self, path: &str) -> Result<String, FsError> {
        self.files.get(path).cloned().ok_or(FsError::NotFound)
    }
    fn write_atomic(&mut self, path: &str, data: &str) -> Result<(), FsError> {
        self.check(FsOp::WriteAtomic)?;
        self.files.insert(path.to_string(), data.to_string());
        Ok(())
    }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        self.check(FsOp::Rename)?;
        let data = self.files.remove(from).ok_or(FsError::NotFound)?;
        self.files.insert(to.to_string(), data);
        Ok(())
    }
    fn remove(&mut self, path: &str) -> Result<(), FsError> {
        self.check(FsOp::Remove)?;
        self.files.remove(path).map(|_| ()).ok_or(FsError::NotFound)
    }
    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
}

// ------------------------------------------------------------ types ----

#[derive(Clone, PartialEq, Debug)]
pub enum Health {
    Ok,
    Broken(ProblemMsg),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Entry {
    pub file: String,
    pub name: String,
    pub orientation: Option<Orientation>,
    pub controls: usize,
    pub health: Health,
}

/// Receipt for `delete`; hand it to `undelete`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TrashToken {
    file: String,
    trash_name: String,
}

#[derive(Clone, PartialEq, Debug)]
pub enum StoreError {
    Fs(FsError),
    Invalid(ProblemMsg),
    NotFound,
    NameTaken,
}

impl From<FsError> for StoreError {
    fn from(e: FsError) -> Self {
        match e {
            FsError::NotFound => StoreError::NotFound,
            e => StoreError::Fs(e),
        }
    }
}

// ------------------------------------------------------------ slugs ----

fn ascii_of(c: char) -> Option<char> {
    const MAP: [(char, &str); 7] = [
        ('a', "àáạảãâầấậẩẫăằắặẳẵ"),
        ('e', "èéẹẻẽêềếệểễ"),
        ('i', "ìíịỉĩ"),
        ('o', "òóọỏõôồốộổỗơờớợởỡ"),
        ('u', "ùúụủũưừứựửữ"),
        ('y', "ỳýỵỷỹ"),
        ('d', "đ"),
    ];
    if c.is_ascii_alphanumeric() {
        return Some(c.to_ascii_lowercase());
    }
    MAP.iter().find(|(_, set)| set.contains(c)).map(|(b, _)| *b)
}

fn is_reserved(s: &str) -> bool {
    let up = s.to_ascii_uppercase();
    matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((up.starts_with("COM") || up.starts_with("LPT"))
            && up.len() == 4
            && matches!(up.as_bytes()[3], b'1'..=b'9'))
}

/// File-name stem for a profile name: Vietnamese letters to ASCII, lowercase,
/// everything else `-`, no leading/trailing `-`, at most 48 chars, never a
/// Windows-reserved device name, never empty.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        // combining marks (decomposed Vietnamese) are dropped
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }
        match ascii_of(c) {
            Some(a) => out.push(a),
            None => {
                if !out.ends_with('-') {
                    out.push('-');
                }
            }
        }
    }
    let mut s = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if s.is_empty() {
        return "ho-so".to_string();
    }
    if is_reserved(&s) {
        s = format!("ho-so-{s}");
    }
    if s.len() > MAX_SLUG {
        s.truncate(MAX_SLUG); // ASCII only, so any index is a boundary
        s = s.trim_end_matches('-').to_string();
    }
    s
}

fn safe_file(file: &str) -> bool {
    !file.is_empty()
        && file.len() <= 120
        && file.to_ascii_lowercase().ends_with(".json")
        && !file.starts_with('.')
        && !file.chars().any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
}

// -------------------------------------------------- problem mapping ----

fn control_label(p: &Profile, i: usize) -> String {
    let Some(c) = p.controls.get(i) else {
        return String::new();
    };
    let (label, kind) = match c {
        Control::Joystick { label, .. } => (label, Kind::Joystick),
        Control::Look { label, .. } => (label, Kind::Look),
        Control::Button { label, .. } => (label, Kind::Button),
    };
    if !label.is_empty() {
        return label.clone();
    }
    let same = |c: &Control| {
        matches!(
            (c, kind),
            (Control::Joystick { .. }, Kind::Joystick)
                | (Control::Look { .. }, Kind::Look)
                | (Control::Button { .. }, Kind::Button)
        )
    };
    let total = p.controls.iter().filter(|c| same(c)).count() as u32;
    let pos = p.controls[..=i].iter().filter(|c| same(c)).count() as u32;
    let n = match kind {
        Kind::Button => pos,
        Kind::Joystick if total > 1 => pos,
        _ => 0,
    };
    Msg::ControlName { kind, n }.text(Lang::Vi)
}

fn path_owner(p: Option<&Profile>, path: &str) -> String {
    match path {
        "toggle_key" => Msg::SlotToggle.text(Lang::Vi),
        "panic_key" => Msg::SlotPanic.text(Lang::Vi),
        _ => match (p, control_index(path)) {
            (Some(p), Some(i)) => control_label(p, i),
            _ => path.to_string(),
        },
    }
}

fn control_index(path: &str) -> Option<usize> {
    let rest = path.strip_prefix("controls[")?;
    rest.split(']').next()?.parse().ok()
}

fn quoted(s: &str) -> String {
    s.split('"').nth(1).unwrap_or("").to_string()
}

fn short(s: &str) -> String {
    if s.chars().count() > 30 {
        let t: String = s.chars().take(30).collect();
        format!("{t}…")
    } else {
        s.to_string()
    }
}

/// Plain-language reading of a validation failure. `p` is the leniently
/// parsed profile (for control labels), when there is one.
pub(crate) fn explain_error(e: &ProfileError, p: Option<&Profile>) -> ProblemMsg {
    match e {
        ProfileError::Json(m) => {
            if m.contains("unknown field") || m.contains("unknown variant") {
                ProblemMsg::NotUnderstood
            } else {
                ProblemMsg::NotReadable
            }
        }
        ProfileError::Invalid(Problem { path, message }) => explain_problem(path, message, p),
    }
}

fn explain_problem(path: &str, message: &str, p: Option<&Profile>) -> ProblemMsg {
    let label = || match control_index(path) {
        Some(i) => p.map(|p| control_label(p, i)).unwrap_or_default(),
        None => String::new(),
    };
    if path == "version" {
        let newer = message
            .rsplit("got ")
            .next()
            .and_then(|n| n.trim().parse::<u64>().ok())
            .is_some_and(|n| n > 1);
        return if newer {
            ProblemMsg::NewerVersion
        } else {
            ProblemMsg::BadVersion
        };
    }
    if path == "name" || path == "app" {
        let text = p
            .map(|p| if path == "name" { &p.name } else { &p.app })
            .map(|s| short(s))
            .unwrap_or_default();
        return ProblemMsg::NameTooLong {
            text,
            max: MAX_NAME_CHARS,
        };
    }
    if path == "controls" {
        return ProblemMsg::TooManyControls { max: 64 };
    }
    if path == "toggle_key" || path == "panic_key" {
        if message.starts_with("must be a keyboard key") {
            return ProblemMsg::MouseOnToggle;
        }
        if message.contains("already used by") {
            return ProblemMsg::TogglePanicSame;
        }
        return ProblemMsg::UnknownKey {
            key: quoted(message),
            label: path_owner(p, path),
        };
    }
    if control_index(path).is_some() {
        let field = path.rsplit('.').next().unwrap_or("");
        if !path.contains('.') {
            return ProblemMsg::SecondLook;
        }
        match field {
            "x" | "y" => return ProblemMsg::OutOfScreen { label: label() },
            "radius" => return ProblemMsg::RadiusRange { label: label() },
            "reach" => return ProblemMsg::ReachRange,
            "sensitivity" => return ProblemMsg::SensitivityRange,
            "release_after_ms" => return ProblemMsg::ReleaseRange,
            "label" => {
                return ProblemMsg::NameTooLong {
                    text: p
                        .and_then(|p| control_index(path).and_then(|i| p.controls.get(i)))
                        .map(|c| match c {
                            Control::Joystick { label, .. }
                            | Control::Look { label, .. }
                            | Control::Button { label, .. } => short(label),
                        })
                        .unwrap_or_default(),
                    max: MAX_LABEL_CHARS,
                };
            }
            "up" | "down" | "left" | "right" | "key" => {
                if message.contains("already used by") {
                    let other = message.rsplit("used by ").next().unwrap_or("");
                    return ProblemMsg::DuplicateKey {
                        key: quoted(message),
                        a: path_owner(p, other),
                        b: path_owner(p, path),
                    };
                }
                return ProblemMsg::UnknownKey {
                    key: quoted(message),
                    label: label(),
                };
            }
            _ => {}
        }
    }
    ProblemMsg::NotUnderstood
}

fn lenient(text: &str) -> Option<Profile> {
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).ok()
}

fn explain_text(text: &str, e: &ProfileError) -> ProblemMsg {
    explain_error(e, lenient(text).as_ref())
}

// ------------------------------------------------------------ repair ----

/// If clamping numbers into their allowed ranges alone makes the text a
/// valid profile, return it with one `Msg::Repaired` per changed number.
/// `None` when the file is already valid (nothing to repair), is not a
/// readable profile, or has any problem that is not a number out of range.
pub fn repair_ranges(text: &str) -> Option<(Profile, Vec<Msg>)> {
    let mut p = lenient(text)?;
    if Profile::from_json(text).is_ok() {
        return None;
    }
    let mut report = Vec::new();
    let labels: Vec<String> = (0..p.controls.len()).map(|i| control_label(&p, i)).collect();
    fn fix(
        report: &mut Vec<Msg>,
        label: &str,
        field: RepairField,
        v: &mut f64,
        lo: f64,
        hi: f64,
    ) {
        if v.is_finite() && *v >= lo && *v <= hi {
            return;
        }
        let from = *v;
        *v = if from.is_finite() { from.clamp(lo, hi) } else { lo };
        report.push(Msg::Repaired {
            label: label.to_string(),
            field,
            from,
            to: *v,
        });
    }
    for (i, c) in p.controls.iter_mut().enumerate() {
        let l = labels[i].as_str();
        match c {
            Control::Joystick { x, y, radius, .. } => {
                fix(&mut report, l, RepairField::X, x, 0.0, 1.0);
                fix(&mut report, l, RepairField::Y, y, 0.0, 1.0);
                fix(&mut report, l, RepairField::Radius, radius, RADIUS_MIN, RADIUS_MAX);
            }
            Control::Look {
                x,
                y,
                sensitivity,
                reach,
                release_after_ms,
                ..
            } => {
                fix(&mut report, l, RepairField::X, x, 0.0, 1.0);
                fix(&mut report, l, RepairField::Y, y, 0.0, 1.0);
                fix(&mut report, l, RepairField::Sensitivity, sensitivity, SENS_MIN, SENS_MAX);
                fix(&mut report, l, RepairField::Reach, reach, REACH_MIN, REACH_MAX);
                if *release_after_ms > MAX_RELEASE_MS {
                    report.push(Msg::Repaired {
                        label: l.to_string(),
                        field: RepairField::ReleaseMs,
                        from: f64::from(*release_after_ms),
                        to: f64::from(MAX_RELEASE_MS),
                    });
                    *release_after_ms = MAX_RELEASE_MS;
                }
            }
            Control::Button { x, y, .. } => {
                fix(&mut report, l, RepairField::X, x, 0.0, 1.0);
                fix(&mut report, l, RepairField::Y, y, 0.0, 1.0);
            }
        }
    }
    if report.is_empty() || p.validate().is_err() {
        return None;
    }
    Some((p, report))
}

// ------------------------------------------------------------ store ----

pub struct ProfileStore<F: Fs> {
    fs: F,
    dir: String,
}

impl<F: Fs> ProfileStore<F> {
    pub fn new(fs: F, dir: &str) -> Self {
        ProfileStore {
            fs,
            dir: dir.trim_end_matches('/').to_string(),
        }
    }

    pub fn fs(&self) -> &F {
        &self.fs
    }
    pub fn fs_mut(&mut self) -> &mut F {
        &mut self.fs
    }
    pub fn into_fs(self) -> F {
        self.fs
    }
    pub fn dir(&self) -> &str {
        &self.dir
    }

    fn path(&self, rest: &str) -> String {
        format!("{}/{}", self.dir, rest)
    }
    fn draft_dir(&self) -> String {
        self.path(".draft")
    }
    fn trash_dir(&self) -> String {
        self.path(".trash")
    }

    fn check_file(&self, file: &str) -> Result<(), StoreError> {
        if safe_file(file) {
            Ok(())
        } else {
            Err(StoreError::NotFound)
        }
    }

    /// `*.json` files, broken ones included (never removed or rewritten).
    pub fn list(&self) -> Vec<Entry> {
        let names = self.fs.list(&self.dir).unwrap_or_default();
        names
            .into_iter()
            .filter(|n| safe_file(n))
            .map(|file| {
                let stem = file[..file.len() - 5].to_string();
                match self.fs.read(&self.path(&file)) {
                    Err(_) => Entry {
                        file,
                        name: stem,
                        orientation: None,
                        controls: 0,
                        health: Health::Broken(ProblemMsg::NotReadable),
                    },
                    Ok(text) => {
                        let loose = lenient(&text);
                        let (health, strict) = match Profile::from_json(&text) {
                            Ok(p) => (Health::Ok, Some(p)),
                            Err(e) => (Health::Broken(explain_text(&text, &e)), None),
                        };
                        let shown = strict.as_ref().or(loose.as_ref());
                        Entry {
                            name: shown
                                .map(|p| p.name.trim().to_string())
                                .filter(|n| !n.is_empty())
                                .unwrap_or(stem),
                            orientation: shown.map(|p| p.orientation),
                            controls: shown.map_or(0, |p| p.controls.len()),
                            file,
                            health,
                        }
                    }
                }
            })
            .collect()
    }

    pub fn load(&self, file: &str) -> Result<Profile, StoreError> {
        self.check_file(file)?;
        let text = self.fs.read(&self.path(file))?;
        Profile::from_json(&text).map_err(|e| StoreError::Invalid(explain_text(&text, &e)))
    }

    /// Validate, keep the previous version as `<file>.bak`, write atomically.
    /// If any step fails the existing file is unchanged.
    pub fn save(&mut self, file: &str, p: &Profile) -> Result<(), StoreError> {
        self.check_file(file)?;
        if let Err(e) = p.validate() {
            return Err(StoreError::Invalid(explain_error(&e, Some(p))));
        }
        let path = self.path(file);
        if self.fs.exists(&path) {
            let old = self.fs.read(&path)?;
            self.fs.write_atomic(&format!("{path}.bak"), &old)?;
        }
        self.fs.write_atomic(&path, &p.to_json_pretty())?;
        Ok(())
    }

    fn taken_files(&self) -> Vec<String> {
        self.fs
            .list(&self.dir)
            .unwrap_or_default()
            .into_iter()
            .map(|n| n.to_lowercase())
            .collect()
    }

    /// `<slug>.json`, or `<slug>-2.json`, `-3`, ... if that exists.
    fn unique_file(&self, slug: &str) -> String {
        let taken = self.taken_files();
        let free = |f: &str| !taken.contains(&f.to_lowercase()) && !self.fs.exists(&self.path(f));
        let first = format!("{slug}.json");
        if free(&first) {
            return first;
        }
        (2u32..)
            .map(|k| format!("{slug}-{k}.json"))
            .find(|f| free(f))
            .unwrap_or(first)
    }

    fn names(&self) -> Vec<String> {
        self.list().into_iter().map(|e| e.name.to_lowercase()).collect()
    }

    /// New profile from a template, named `Hồ sơ N` (smallest free N).
    pub fn create(&mut self, tpl: Template) -> Result<String, StoreError> {
        let names = self.names();
        let n = (1u32..)
            .find(|n| !names.contains(&Msg::NewProfileName(*n).text(Lang::Vi).to_lowercase()))
            .unwrap_or(1);
        let name = Msg::NewProfileName(n).text(Lang::Vi);
        let mut p = template(tpl);
        p.name = name.clone();
        let file = self.unique_file(&slugify(&name));
        self.save(&file, &p)?;
        Ok(file)
    }

    /// Change the profile's name and move it to the matching file name.
    /// Returns the (possibly new) file name; the caller updates its prefs.
    pub fn rename(&mut self, file: &str, new_name: &str) -> Result<String, StoreError> {
        let name = new_name.trim();
        if name.is_empty() {
            return Err(StoreError::Invalid(ProblemMsg::NameTooLong {
                text: String::new(),
                max: MAX_NAME_CHARS,
            }));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(StoreError::Invalid(ProblemMsg::NameTooLong {
                text: short(name),
                max: MAX_NAME_CHARS,
            }));
        }
        let mut p = self.load(file)?;
        let lower = name.to_lowercase();
        if self
            .list()
            .iter()
            .any(|e| e.file != file && e.name.to_lowercase() == lower)
        {
            return Err(StoreError::NameTaken);
        }
        p.name = name.to_string();
        let slug = slugify(name);
        let stem = &file[..file.len() - 5];
        let new_file = if stem == slug || stem.strip_prefix(&format!("{slug}-")).is_some_and(|k| k.parse::<u32>().is_ok())
        {
            file.to_string()
        } else {
            self.unique_file(&slug)
        };
        self.save(&new_file, &p)?;
        if new_file != file {
            // the new file is safely written; now move the old one's extras
            let (old, new) = (self.path(file), self.path(&new_file));
            let _ = self.fs.remove(&old);
            if self.fs.exists(&format!("{old}.bak")) {
                let _ = self.fs.rename(&format!("{old}.bak"), &format!("{new}.bak"));
            }
            let (od, nd) = (
                format!("{}/{file}", self.draft_dir()),
                format!("{}/{new_file}", self.draft_dir()),
            );
            if self.fs.exists(&od) {
                let _ = self.fs.rename(&od, &nd);
            }
        }
        Ok(new_file)
    }

    /// Copy named `<name> (bản sao)`, `<name> (bản sao 2)`, ...
    pub fn duplicate(&mut self, file: &str) -> Result<String, StoreError> {
        let mut p = self.load(file)?;
        let names = self.names();
        let base: String = p.name.chars().take(MAX_NAME_CHARS - 16).collect();
        let name = (1u32..)
            .map(|k| {
                if k == 1 {
                    format!("{base} (bản sao)")
                } else {
                    format!("{base} (bản sao {k})")
                }
            })
            .find(|n| !names.contains(&n.to_lowercase()))
            .unwrap_or_default();
        p.name = name.clone();
        let new_file = self.unique_file(&slugify(&name));
        self.save(&new_file, &p)?;
        Ok(new_file)
    }

    /// Move the file (and its `.bak`) to `.trash/`.
    pub fn delete(&mut self, file: &str) -> Result<TrashToken, StoreError> {
        self.check_file(file)?;
        let src = self.path(file);
        if !self.fs.exists(&src) {
            return Err(StoreError::NotFound);
        }
        let trash = self.trash_dir();
        let mut trash_name = file.to_string();
        let mut k = 1;
        while self.fs.exists(&format!("{trash}/{trash_name}")) {
            k += 1;
            trash_name = format!("{k}-{file}");
        }
        let dst = format!("{trash}/{trash_name}");
        self.fs.rename(&src, &dst)?;
        if self.fs.exists(&format!("{src}.bak")) {
            let _ = self.fs.rename(&format!("{src}.bak"), &format!("{dst}.bak"));
        }
        self.drop_draft(file);
        Ok(TrashToken {
            file: file.to_string(),
            trash_name,
        })
    }

    /// Put a deleted profile back (under another file name if the old one
    /// was taken meanwhile). Returns the file name.
    pub fn undelete(&mut self, t: TrashToken) -> Result<String, StoreError> {
        let src = format!("{}/{}", self.trash_dir(), t.trash_name);
        if !self.fs.exists(&src) {
            return Err(StoreError::NotFound);
        }
        let file = if self.fs.exists(&self.path(&t.file)) {
            self.unique_file(&t.file[..t.file.len() - 5])
        } else {
            t.file.clone()
        };
        let dst = self.path(&file);
        self.fs.rename(&src, &dst)?;
        if self.fs.exists(&format!("{src}.bak")) {
            let _ = self.fs.rename(&format!("{src}.bak"), &format!("{dst}.bak"));
        }
        Ok(file)
    }

    /// Empty `.trash/` (called once at start-up); errors are ignored.
    pub fn purge_trash(&mut self) {
        let dir = self.trash_dir();
        for n in self.fs.list(&dir).unwrap_or_default() {
            let _ = self.fs.remove(&format!("{dir}/{n}"));
        }
    }

    /// Add a valid profile from text. Never overwrites; the file name comes
    /// from `hint` (e.g. the dropped file's name), else from the profile's
    /// name. An unnamed profile is named after the hint.
    pub fn import_text(&mut self, text: &str, hint: &str) -> Result<String, StoreError> {
        let mut p = Profile::from_json(text)
            .map_err(|e| StoreError::Invalid(explain_text(text, &e)))?;
        let hint = hint.trim();
        let hint = hint
            .get(hint.len().saturating_sub(5)..)
            .filter(|t| t.eq_ignore_ascii_case(".json"))
            .map_or(hint, |_| &hint[..hint.len() - 5])
            .trim();
        if p.name.trim().is_empty() {
            p.name = if hint.is_empty() {
                Msg::NewProfileName(1).text(Lang::Vi)
            } else {
                hint.chars().take(MAX_NAME_CHARS).collect()
            };
        }
        let slug = slugify(if hint.is_empty() { &p.name } else { hint });
        let file = self.unique_file(&slug);
        self.save(&file, &p)?;
        Ok(file)
    }

    /// Same text as `Profile::to_json_pretty`.
    pub fn export_text(&self, file: &str) -> Result<String, StoreError> {
        Ok(self.load(file)?.to_json_pretty())
    }

    /// Crash-safe copy of an unsaved edit: `.draft/<file>`.
    pub fn write_draft(&mut self, file: &str, p: &Profile) -> Result<(), StoreError> {
        self.check_file(file)?;
        if let Err(e) = p.validate() {
            return Err(StoreError::Invalid(explain_error(&e, Some(p))));
        }
        let path = format!("{}/{file}", self.draft_dir());
        self.fs.write_atomic(&path, &p.to_json_pretty())?;
        Ok(())
    }

    pub fn draft(&self, file: &str) -> Option<Profile> {
        if !safe_file(file) {
            return None;
        }
        let text = self.fs.read(&format!("{}/{file}", self.draft_dir())).ok()?;
        Profile::from_json(&text).ok()
    }

    pub fn drop_draft(&mut self, file: &str) {
        if safe_file(file) {
            let _ = self.fs.remove(&format!("{}/{file}", self.draft_dir()));
        }
    }

    /// File names that have a draft.
    pub fn drafts(&self) -> Vec<String> {
        self.fs
            .list(&self.draft_dir())
            .unwrap_or_default()
            .into_iter()
            .filter(|n| safe_file(n))
            .collect()
    }
}
