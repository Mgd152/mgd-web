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
    fail_next: std::cell::Cell<Option<(FsOp, usize)>>,
}

impl MemFs {
    pub fn new() -> MemFs {
        MemFs::default()
    }

    /// Make the next call of `op` fail with `FsError::Io` (once).
    pub fn fail_next(&mut self, op: FsOp) {
        self.fail_next.set(Some((op, 0)));
    }

    /// Let `skip` calls of `op` succeed, then fail the one after (once).
    pub fn fail_nth(&mut self, op: FsOp, skip: usize) {
        self.fail_next.set(Some((op, skip)));
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

    fn check(&self, op: FsOp) -> Result<(), FsError> {
        match self.fail_next.get() {
            Some((o, 0)) if o == op => {
                self.fail_next.set(None);
                Err(FsError::Io(format!("injected {op:?} failure")))
            }
            Some((o, skip)) if o == op => {
                self.fail_next.set(Some((o, skip - 1)));
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl Fs for MemFs {
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        self.check(FsOp::List)?;
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
        self.check(FsOp::Read)?;
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
        && !file
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
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
    let labels: Vec<String> = (0..p.controls.len())
        .map(|i| control_label(&p, i))
        .collect();
    fn fix(report: &mut Vec<Msg>, label: &str, field: RepairField, v: &mut f64, lo: f64, hi: f64) {
        if v.is_finite() && *v >= lo && *v <= hi {
            return;
        }
        let from = *v;
        *v = if from.is_finite() {
            from.clamp(lo, hi)
        } else {
            lo
        };
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
                fix(
                    &mut report,
                    l,
                    RepairField::Radius,
                    radius,
                    RADIUS_MIN,
                    RADIUS_MAX,
                );
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
                fix(
                    &mut report,
                    l,
                    RepairField::Sensitivity,
                    sensitivity,
                    SENS_MIN,
                    SENS_MAX,
                );
                fix(
                    &mut report,
                    l,
                    RepairField::Reach,
                    reach,
                    REACH_MIN,
                    REACH_MAX,
                );
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
        self.list()
            .into_iter()
            .map(|e| e.name.to_lowercase())
            .collect()
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
        let new_file = if stem == slug
            || stem
                .strip_prefix(&format!("{slug}-"))
                .is_some_and(|k| k.parse::<u32>().is_ok())
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
        let mut p =
            Profile::from_json(text).map_err(|e| StoreError::Invalid(explain_text(text, &e)))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::templates::Template::*;

    const DIR: &str = "keymaps";

    fn store() -> ProfileStore<MemFs> {
        ProfileStore::new(MemFs::new(), DIR)
    }

    fn data(sub: &str) -> Vec<(String, String)> {
        let dir = format!("{}/tests/data/{sub}", env!("CARGO_MANIFEST_DIR"));
        let mut v: Vec<(String, String)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .filter(|p| p.file_name().unwrap() != "expected.json")
            .map(|p| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    String::from_utf8(std::fs::read(&p).unwrap()).unwrap(),
                )
            })
            .collect();
        v.sort();
        assert!(!v.is_empty());
        v
    }

    fn files(s: &ProfileStore<MemFs>) -> Vec<String> {
        s.fs().paths()
    }

    // test 59
    #[test]
    fn create_numbers_profiles_and_names_files() {
        let mut s = store();
        assert_eq!(s.create(BasicLandscape).unwrap(), "ho-so-1.json");
        assert_eq!(s.create(ShooterLandscape).unwrap(), "ho-so-2.json");
        assert_eq!(
            s.create(Empty(Orientation::Portrait)).unwrap(),
            "ho-so-3.json"
        );
        let names: Vec<String> = s.list().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["Hồ sơ 1", "Hồ sơ 2", "Hồ sơ 3"]);
        assert_eq!(
            s.load("ho-so-3.json").unwrap().orientation,
            Orientation::Portrait
        );
        // a hole is refilled
        s.delete("ho-so-2.json").unwrap();
        assert_eq!(s.create(BasicPortrait).unwrap(), "ho-so-2.json");
    }

    #[test]
    fn slug_collisions_get_a_number() {
        let mut s = store();
        // an imported profile that squats on the slug of "Hồ sơ 1"
        let other = r#"{"version":1,"name":"Của tôi","orientation":"landscape"}"#;
        assert_eq!(
            s.import_text(other, "Hồ sơ 1.json").unwrap(),
            "ho-so-1.json"
        );
        assert_eq!(s.create(BasicLandscape).unwrap(), "ho-so-1-2.json");
        assert_eq!(s.create(BasicLandscape).unwrap(), "ho-so-2.json");
        assert_eq!(s.load("ho-so-1-2.json").unwrap().name, "Hồ sơ 1");
        // and again: -3
        let third = s.import_text(other, "Hồ sơ 1").unwrap();
        assert_eq!(third, "ho-so-1-3.json");
    }

    // test 60
    #[test]
    fn slugify_cases() {
        assert_eq!(slugify("Hồ sơ Ngang"), "ho-so-ngang");
        assert_eq!(slugify("Đấu trường"), "dau-truong");
        assert_eq!(slugify("ĐẤU TRƯỜNG"), "dau-truong");
        assert_eq!(slugify("Tiếng Việt: ươ ữ ỹ ỵ"), "tieng-viet-uo-u-y-y");
        // decomposed: o + circumflex + dot below
        assert_eq!(slugify("Ho\u{0302}\u{0300} so\u{031b}"), "ho-so");
        assert_eq!(slugify("  A   B__C  "), "a-b-c");
        for r in ["CON", "con", "Prn", "AUX", "nul", "COM1", "lpt9", "Com5"] {
            assert_eq!(slugify(r), format!("ho-so-{}", r.to_lowercase()), "{r}");
        }
        assert_eq!(slugify("COM0"), "com0");
        assert_eq!(slugify("CONSOLE"), "console");
        assert_eq!(slugify("name..."), "name");
        assert_eq!(slugify("...hidden"), "hidden");
        assert_eq!(slugify(".-."), "ho-so");
        assert_eq!(slugify(""), "ho-so");
        assert_eq!(slugify("***"), "ho-so");
        assert_eq!(slugify("日本語"), "ho-so");
        let long = slugify(&"ab ".repeat(100));
        assert!(long.len() <= 48 && !long.ends_with('-') && !long.starts_with('-'));
        let long = slugify(&"x".repeat(100));
        assert_eq!(long.len(), 48);
        // the truncation point may land on a '-'
        let s = slugify(&format!("{} y", "x".repeat(47)));
        assert_eq!(s, "x".repeat(47));
        for n in ["a/b", "a\\b", "a:b", "a*b?\"<>|", "tab\there"] {
            let s = slugify(n);
            assert!(
                s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "{s}"
            );
        }
    }

    fn some_profile(name: &str) -> Profile {
        let mut p = template(BasicLandscape);
        p.name = name.to_string();
        p
    }

    // test 61
    #[test]
    fn save_writes_bak_and_validates() {
        let mut s = store();
        let a = some_profile("A");
        s.save("a.json", &a).unwrap();
        assert!(!s.fs().exists("keymaps/a.json.bak"));
        let mut b = some_profile("B");
        b.name = "B".into();
        s.save("a.json", &b).unwrap();
        assert_eq!(
            s.fs().get("keymaps/a.json.bak").unwrap(),
            a.to_json_pretty()
        );
        assert_eq!(s.fs().get("keymaps/a.json").unwrap(), b.to_json_pretty());
        let c = some_profile("C");
        s.save("a.json", &c).unwrap();
        assert_eq!(
            s.fs().get("keymaps/a.json.bak").unwrap(),
            b.to_json_pretty()
        );
        // invalid profile: refused, nothing changes
        let before = files(&s);
        let mut bad = some_profile("D");
        bad.toggle_key = "F8".into(); // equals the panic key
        assert!(matches!(
            s.save("a.json", &bad),
            Err(StoreError::Invalid(ProblemMsg::TogglePanicSame))
        ));
        assert_eq!(files(&s), before);
        assert_eq!(s.fs().get("keymaps/a.json").unwrap(), c.to_json_pretty());
        // unsafe file names
        for f in [
            "../x.json",
            "a/b.json",
            "x.txt",
            "",
            ".json",
            ".hidden.json",
        ] {
            assert_eq!(s.save(f, &c), Err(StoreError::NotFound), "{f}");
        }
    }

    #[test]
    fn failed_write_leaves_the_old_file_for_every_fs_op() {
        let old = some_profile("Old");
        let new = some_profile("New");
        for op in [
            FsOp::List,
            FsOp::Read,
            FsOp::WriteAtomic,
            FsOp::Rename,
            FsOp::Remove,
        ] {
            for skip in 0..3 {
                let mut s = store();
                s.save("a.json", &old).unwrap();
                s.fs_mut().fail_nth(op, skip);
                let r = s.save("a.json", &new);
                let now = s.fs().get("keymaps/a.json").unwrap().to_string();
                match r {
                    Err(StoreError::Fs(_)) => {
                        assert_eq!(now, old.to_json_pretty(), "{op:?} {skip}")
                    }
                    Ok(()) => assert_eq!(now, new.to_json_pretty(), "{op:?} {skip}"),
                    Err(e) => panic!("{op:?} {skip}: {e:?}"),
                }
                // whatever happened, the file is a complete valid profile
                assert!(Profile::from_json(&now).is_ok());
            }
        }
        // the main write failing (second write_atomic) keeps the old bytes
        let mut s = store();
        s.save("a.json", &old).unwrap();
        s.fs_mut().fail_nth(FsOp::WriteAtomic, 1);
        assert!(matches!(
            s.save("a.json", &new),
            Err(StoreError::Fs(FsError::Io(_)))
        ));
        assert_eq!(s.fs().get("keymaps/a.json").unwrap(), old.to_json_pretty());
        // failure is one-shot
        s.save("a.json", &new).unwrap();
    }

    #[test]
    fn failures_in_other_operations_change_nothing() {
        let mut s = store();
        let f = s.create(BasicLandscape).unwrap();
        let before = s.fs().clone().paths();
        s.fs_mut().fail_next(FsOp::Rename);
        assert!(matches!(s.delete(&f), Err(StoreError::Fs(_))));
        assert_eq!(s.fs().paths(), before);
        s.fs_mut().fail_next(FsOp::WriteAtomic);
        assert!(matches!(s.rename(&f, "Khác"), Err(StoreError::Fs(_))));
        assert_eq!(s.fs().paths(), before);
        s.fs_mut().fail_next(FsOp::WriteAtomic);
        assert!(matches!(s.duplicate(&f), Err(StoreError::Fs(_))));
        assert_eq!(s.fs().paths(), before);
        s.fs_mut().fail_next(FsOp::Read);
        assert!(matches!(s.load(&f), Err(StoreError::Fs(_))));
        s.fs_mut().fail_next(FsOp::List);
        assert!(s.list().is_empty()); // list failure: empty, not a panic
        assert_eq!(s.list().len(), 1);
    }

    // test 62
    #[test]
    fn rename_updates_name_and_file() {
        let mut s = store();
        let f = s.create(BasicLandscape).unwrap();
        s.save(&f, &some_profile("Hồ sơ 1")).unwrap(); // makes a .bak
        s.write_draft(&f, &some_profile("Nháp")).unwrap();
        let g = s.rename(&f, "  Đấu trường  ").unwrap();
        assert_eq!(g, "dau-truong.json");
        assert_eq!(s.load(&g).unwrap().name, "Đấu trường");
        assert!(!s.fs().exists("keymaps/ho-so-1.json"));
        assert!(s.fs().exists("keymaps/dau-truong.json.bak"));
        assert!(!s.fs().exists("keymaps/ho-so-1.json.bak"));
        assert_eq!(s.drafts(), std::slice::from_ref(&g));
        // same slug: file kept, name changes
        assert_eq!(s.rename(&g, "ĐẤU TRƯỜNG").unwrap(), g);
        assert_eq!(s.load(&g).unwrap().name, "ĐẤU TRƯỜNG");
        // taken name (case-insensitive) and bad names
        let h = s.create(BasicLandscape).unwrap();
        assert_eq!(s.rename(&h, "đấu trường"), Err(StoreError::NameTaken));
        assert!(matches!(
            s.rename(&h, &"x".repeat(101)),
            Err(StoreError::Invalid(ProblemMsg::NameTooLong {
                max: 100,
                ..
            }))
        ));
        assert!(matches!(s.rename(&h, "   "), Err(StoreError::Invalid(_))));
        assert_eq!(s.rename("nope.json", "Z"), Err(StoreError::NotFound));
        // another profile's slug: gets -2
        let k = s.rename(&h, "Đấu-trường").unwrap();
        assert_eq!(k, "dau-truong-2.json");
        // keeping the "-2" file when renaming back to the same slug
        assert_eq!(s.rename(&k, "Đấu trường!").unwrap(), k);
    }

    #[test]
    fn duplicate_names_are_unique() {
        let mut s = store();
        let f = s.create(BasicLandscape).unwrap();
        let d1 = s.duplicate(&f).unwrap();
        let d2 = s.duplicate(&f).unwrap();
        let d3 = s.duplicate(&d1).unwrap();
        let names: Vec<String> = [&d1, &d2, &d3]
            .iter()
            .map(|x| s.load(x).unwrap().name)
            .collect();
        assert_eq!(
            names,
            [
                "Hồ sơ 1 (bản sao)",
                "Hồ sơ 1 (bản sao 2)",
                "Hồ sơ 1 (bản sao) (bản sao)"
            ]
        );
        assert_eq!(s.load(&f).unwrap().controls, s.load(&d1).unwrap().controls);
        let mut long = some_profile(&"é".repeat(100));
        long.name = "é".repeat(100);
        s.save("long.json", &long).unwrap();
        let d = s.duplicate("long.json").unwrap();
        assert!(s.load(&d).unwrap().name.chars().count() <= 100);
    }

    // test 63
    #[test]
    fn delete_undelete_restores_bytes_and_purge_empties_trash() {
        let mut s = store();
        let f = s.create(ShooterLandscape).unwrap();
        s.save(&f, &some_profile("Hồ sơ 1")).unwrap();
        let bytes = s.fs().get("keymaps/ho-so-1.json").unwrap().to_string();
        let bak = s.fs().get("keymaps/ho-so-1.json.bak").unwrap().to_string();
        let t = s.delete(&f).unwrap();
        assert!(s.list().is_empty());
        assert!(!s.fs().exists("keymaps/ho-so-1.json"));
        assert!(s.fs().exists("keymaps/.trash/ho-so-1.json"));
        let back = s.undelete(t.clone()).unwrap();
        assert_eq!(back, "ho-so-1.json");
        assert_eq!(s.fs().get("keymaps/ho-so-1.json").unwrap(), bytes);
        assert_eq!(s.fs().get("keymaps/ho-so-1.json.bak").unwrap(), bak);
        assert_eq!(s.undelete(t), Err(StoreError::NotFound)); // used up

        // the name was reused meanwhile: restored under another file name
        let t = s.delete("ho-so-1.json").unwrap();
        s.create(BasicLandscape).unwrap();
        assert_eq!(s.undelete(t).unwrap(), "ho-so-1-2.json");
        assert_eq!(s.fs().get("keymaps/ho-so-1-2.json").unwrap(), bytes);

        // two deletes of the same file name do not clobber each other
        let t1 = s.delete("ho-so-1.json").unwrap();
        let named = r#"{"version":1,"name":"Khác","orientation":"landscape"}"#;
        assert_eq!(s.import_text(named, "ho-so-1").unwrap(), "ho-so-1.json");
        let t2 = s.delete("ho-so-1.json").unwrap();
        assert_ne!(t1, t2);
        assert_eq!(
            s.fs()
                .paths()
                .iter()
                .filter(|p| p.contains(".trash"))
                .count(),
            2
        );

        s.purge_trash();
        assert!(!s.fs().paths().iter().any(|p| p.contains(".trash")));
        assert_eq!(s.undelete(t1), Err(StoreError::NotFound));
        assert_eq!(s.delete("ghost.json"), Err(StoreError::NotFound));
        // deleting also drops a draft
        s.write_draft("ho-so-1-2.json", &some_profile("x")).unwrap();
        s.delete("ho-so-1-2.json").unwrap();
        assert!(s.drafts().is_empty());
    }

    // test 64
    #[test]
    fn broken_files_are_listed_and_never_removed() {
        let mut s = store();
        s.create(BasicLandscape).unwrap();
        let broken = [
            ("b-garbage.json", "not json at all", ProblemMsg::NotReadable),
            (
                "b-range.json",
                r#"{"version":1,"orientation":"landscape","controls":[{"type":"button","label":"Bắn","x":1.5,"y":0.5,"key":"Space"}]}"#,
                ProblemMsg::OutOfScreen {
                    label: "Bắn".into(),
                },
            ),
            (
                "b-newer.json",
                r#"{"version":2,"name":"Mới","orientation":"portrait"}"#,
                ProblemMsg::NewerVersion,
            ),
        ];
        for (f, t, _) in &broken {
            s.fs_mut().put(&format!("keymaps/{f}"), t);
        }
        s.fs_mut().put("keymaps/readme.txt", "ignore me");
        s.fs_mut().put("keymaps/c.json.bak", "{}");
        let before = files(&s);
        let list = s.list();
        assert_eq!(files(&s), before, "list must not touch anything");
        assert_eq!(list.len(), 4);
        assert_eq!(list[0].file, "b-garbage.json");
        for (f, _, msg) in &broken {
            let e = list.iter().find(|e| e.file == *f).unwrap();
            assert_eq!(e.health, Health::Broken(msg.clone()), "{f}");
        }
        let range = list.iter().find(|e| e.file == "b-range.json").unwrap();
        assert_eq!(
            (range.controls, range.orientation),
            (1, Some(Orientation::Landscape))
        );
        assert_eq!(
            list.iter()
                .find(|e| e.file == "b-garbage.json")
                .unwrap()
                .name,
            "b-garbage"
        );
        assert_eq!(
            list.iter().find(|e| e.file == "b-newer.json").unwrap().name,
            "Mới"
        );
        assert_eq!(
            list.iter()
                .find(|e| e.file == "ho-so-1.json")
                .unwrap()
                .health,
            Health::Ok
        );
        // loading one says why; a broken file can be deleted and restored
        assert!(matches!(
            s.load("b-newer.json"),
            Err(StoreError::Invalid(ProblemMsg::NewerVersion))
        ));
        let t = s.delete("b-garbage.json").unwrap();
        s.undelete(t).unwrap();
        assert_eq!(
            s.fs().get("keymaps/b-garbage.json").unwrap(),
            "not json at all"
        );
        // creating more profiles does not disturb them
        s.create(BasicPortrait).unwrap();
        assert_eq!(
            s.fs().get("keymaps/b-garbage.json").unwrap(),
            "not json at all"
        );
        // an unreadable file (read failure) is listed as broken, too
        s.fs_mut().fail_nth(FsOp::Read, 0);
        let list = s.list();
        assert_eq!(list.len(), 5);
        assert!(
            list.iter()
                .any(|e| e.health == Health::Broken(ProblemMsg::NotReadable))
        );
    }

    // test 65
    #[test]
    fn import_accepts_every_valid_sample() {
        for (name, text) in data("valid") {
            let mut s = store();
            let file = s
                .import_text(&text, &name)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let p = Profile::from_json(&text).unwrap();
            let got = s.load(&file).unwrap();
            assert_eq!(got.controls, p.controls, "{name}");
            assert_eq!(got.orientation, p.orientation);
            assert!(safe_file(&file));
            // second import never overwrites: another file name
            let again = s.import_text(&text, &name).unwrap();
            assert_ne!(again, file);
            assert_eq!(s.list().len(), 2);
        }
    }

    #[test]
    fn import_rejects_every_invalid_sample_and_writes_nothing() {
        let mut s = store();
        s.create(BasicLandscape).unwrap();
        let before = s.fs().clone().paths();
        let bytes = s.fs().get("keymaps/ho-so-1.json").unwrap().to_string();
        for (name, text) in data("invalid") {
            match s.import_text(&text, &name) {
                Err(StoreError::Invalid(m)) => {
                    for lang in [Lang::Vi, Lang::En] {
                        assert!(
                            !Msg::Problem(m.clone()).text(lang).trim().is_empty(),
                            "{name}"
                        );
                    }
                }
                other => panic!("{name}: {other:?}"),
            }
            assert_eq!(s.fs().paths(), before, "{name} wrote something");
        }
        assert_eq!(s.fs().get("keymaps/ho-so-1.json").unwrap(), bytes);
    }

    #[test]
    fn invalid_samples_map_to_the_expected_messages() {
        let want = |n: &str| -> ProblemMsg {
            use ProblemMsg::*;
            match n {
                "array-root.json"
                | "bad-orientation.json"
                | "missing-orientation.json"
                | "not-json.json" => {
                    if n == "bad-orientation.json" {
                        NotUnderstood
                    } else {
                        NotReadable
                    }
                }
                "unknown-field.json" | "unknown-type.json" => NotUnderstood,
                "bad-version.json" => NewerVersion,
                "name-too-long.json" => NameTooLong {
                    text: String::new(),
                    max: 100,
                },
                "mouse-toggle.json" => MouseOnToggle,
                "toggle-equals-panic.json" => TogglePanicSame,
                "too-many-controls.json" => TooManyControls { max: 64 },
                "second-look.json" => SecondLook,
                "x-out-of-range.json" | "y-out-of-range.json" | "second-look-then-bad-x.json" => {
                    OutOfScreen {
                        label: String::new(),
                    }
                }
                "radius-too-big.json" | "radius-zero.json" => RadiusRange {
                    label: String::new(),
                },
                "reach-too-big.json" | "reach-zero.json" => ReachRange,
                "sensitivity-zero.json" => SensitivityRange,
                "release-too-long.json" => ReleaseRange,
                "unknown-key.json" | "unknown-joystick-key.json" => UnknownKey {
                    key: String::new(),
                    label: String::new(),
                },
                _ => DuplicateKey {
                    key: String::new(),
                    a: String::new(),
                    b: String::new(),
                },
            }
        };
        for (name, text) in data("invalid") {
            let got = match Profile::from_json(&text) {
                Err(e) => explain_text(&text, &e),
                Ok(_) => panic!("{name} is valid"),
            };
            assert_eq!(
                std::mem::discriminant(&got),
                std::mem::discriminant(&want(&name)),
                "{name}: {got:?}"
            );
            if let ProblemMsg::DuplicateKey { key, a, b } = &got {
                assert!(!key.is_empty() && !a.is_empty() && !b.is_empty(), "{got:?}");
            }
            if let ProblemMsg::UnknownKey { key, label } = &got {
                assert!(!key.is_empty() && !label.is_empty(), "{got:?}");
            }
        }
    }

    #[test]
    fn import_names_and_hints() {
        let mut s = store();
        let unnamed = r#"{"version":1,"orientation":"landscape"}"#;
        let f = s.import_text(unnamed, "Cấu hình của Nam.json").unwrap();
        assert_eq!(f, "cau-hinh-cua-nam.json");
        assert_eq!(s.load(&f).unwrap().name, "Cấu hình của Nam");
        let f = s.import_text(unnamed, "").unwrap();
        assert_eq!(f, "ho-so-1.json");
        assert_eq!(s.load(&f).unwrap().name, "Hồ sơ 1");
        let named = r#"{"version":1,"name":"Súng","orientation":"landscape"}"#;
        assert_eq!(s.import_text(named, "").unwrap(), "sung.json");
        // BOM is accepted
        assert!(s.import_text(&format!("\u{feff}{named}"), "").is_ok());
        // pre-existing file with different case is not overwritten
        s.fs_mut().put("keymaps/SUNG-2.json", "x");
        assert_eq!(s.import_text(named, "").unwrap(), "sung-3.json");
    }

    // test 66
    #[test]
    fn export_equals_pretty_json_and_reimports() {
        let mut s = store();
        let f = s.create(ShooterLandscape).unwrap();
        let text = s.export_text(&f).unwrap();
        assert_eq!(text, s.load(&f).unwrap().to_json_pretty());
        let g = s.import_text(&text, "copy").unwrap();
        assert_eq!(s.load(&g).unwrap().controls, s.load(&f).unwrap().controls);
        assert_eq!(s.export_text("none.json"), Err(StoreError::NotFound));
    }

    // test 67
    #[test]
    fn drafts_write_read_drop() {
        let mut s = store();
        let f = s.create(BasicLandscape).unwrap();
        assert_eq!(s.draft(&f), None);
        assert!(s.drafts().is_empty());
        let mut p = s.load(&f).unwrap();
        p.name = "Chưa lưu".into();
        s.write_draft(&f, &p).unwrap();
        s.write_draft("moi.json", &p).unwrap(); // draft of a never-saved profile
        assert_eq!(s.draft(&f), Some(p.clone()));
        assert_eq!(s.drafts(), [f.clone(), "moi.json".to_string()]);
        assert_eq!(
            s.load(&f).unwrap().name,
            "Hồ sơ 1",
            "draft is not the profile"
        );
        assert_eq!(s.list().len(), 1, "drafts are not listed as profiles");
        s.drop_draft(&f);
        assert_eq!(s.draft(&f), None);
        s.drop_draft(&f); // idempotent
        assert_eq!(s.drafts(), ["moi.json"]);
        // a damaged draft reads as none
        s.fs_mut().put("keymaps/.draft/moi.json", "{");
        assert_eq!(s.draft("moi.json"), None);
        // failed write: nothing changes
        s.fs_mut().fail_next(FsOp::WriteAtomic);
        assert!(s.write_draft(&f, &p).is_err());
        assert_eq!(s.draft(&f), None);
        // invalid draft refused
        let mut bad = p.clone();
        bad.panic_key = bad.toggle_key.clone();
        assert!(matches!(
            s.write_draft(&f, &bad),
            Err(StoreError::Invalid(_))
        ));
    }

    // test 68
    #[test]
    fn repair_ranges_only_for_pure_range_problems() {
        let wrap = |c: &str| {
            format!(r#"{{"version":1,"name":"R","orientation":"landscape","controls":[{c}]}}"#)
        };
        let btn =
            |x: &str| format!(r#"{{"type":"button","label":"Bắn","x":{x},"y":0.5,"key":"Space"}}"#);
        // x out of range
        let (p, rep) = repair_ranges(&wrap(&btn("1.4"))).unwrap();
        assert!(p.validate().is_ok());
        assert_eq!(rep.len(), 1);
        assert_eq!(
            rep[0],
            Msg::Repaired {
                label: "Bắn".into(),
                field: RepairField::X,
                from: 1.4,
                to: 1.0
            }
        );
        assert_eq!(
            rep[0].text(Lang::Vi),
            "Đã chỉnh vị trí ngang của “Bắn”: 1,40 → 1,00."
        );
        assert_eq!(
            rep[0].text(Lang::En),
            "Adjusted horizontal position of “Bắn”: 1.40 → 1.00."
        );
        // several numbers in several controls: one line each
        let text = wrap(&format!(
            "{},{},{}",
            r#"{"type":"joystick","x":-0.2,"y":0.7,"radius":0.9,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}"#,
            r#"{"type":"look","x":0.7,"y":0.4,"sensitivity":0,"reach":0,"release_after_ms":90000}"#,
            btn("0.5"),
        ));
        let (p, rep) = repair_ranges(&text).unwrap();
        assert!(p.validate().is_ok());
        assert_eq!(rep.len(), 5);
        match (&p.controls[0], &p.controls[1]) {
            (
                Control::Joystick { x, radius, .. },
                Control::Look {
                    sensitivity,
                    reach,
                    release_after_ms,
                    ..
                },
            ) => {
                assert_eq!((*x, *radius), (0.0, RADIUS_MAX));
                assert_eq!(
                    (*sensitivity, *reach, *release_after_ms),
                    (SENS_MIN, REACH_MIN, 60_000)
                );
            }
            c => panic!("{c:?}"),
        }
        // unlabeled controls are named like in the editor
        let (_, rep) = repair_ranges(&wrap(
            r#"{"type":"button","x":2,"y":0.5,"key":"Space"},{"type":"button","x":0.5,"y":2,"key":"KeyE"}"#,
        ))
        .unwrap();
        assert!(matches!(&rep[0], Msg::Repaired { label, .. } if label == "Nút 1"));
        assert!(matches!(&rep[1], Msg::Repaired { label, .. } if label == "Nút 2"));
        // range problem plus another problem: refused
        for bad in [
            wrap(&format!("{},{}", btn("1.4"), btn("0.4"))), // duplicate key
            wrap(r#"{"type":"button","x":1.4,"y":0.5,"key":"Nope"}"#),
            wrap(r#"{"type":"look","x":0.5,"y":0.5},{"type":"look","x":1.5,"y":0.5}"#),
            r#"{"version":2,"orientation":"landscape","controls":[{"type":"button","x":1.4,"y":0.5,"key":"Space"}]}"#.to_string(),
            r#"{"version":1,"orientation":"landscape","extra":1,"controls":[{"type":"button","x":1.4,"y":0.5,"key":"Space"}]}"#.to_string(),
            "not json".to_string(),
            String::new(),
        ] {
            assert!(repair_ranges(&bad).is_none(), "{bad}");
        }
        // already valid: nothing to repair
        assert!(repair_ranges(&wrap(&btn("0.5"))).is_none());
        // every invalid sample: Some only when the result validates
        let mut some = 0;
        for (name, text) in data("invalid") {
            if let Some((p, rep)) = repair_ranges(&text) {
                some += 1;
                assert!(p.validate().is_ok() && !rep.is_empty(), "{name}");
                assert!(Profile::from_json(&p.to_json_pretty()).is_ok());
            }
        }
        assert!(some >= 6, "{some}");
    }

    #[test]
    fn repaired_text_can_be_imported() {
        let mut s = store();
        let text = r#"{"version":1,"orientation":"landscape","controls":[{"type":"button","x":1.4,"y":0.5,"key":"Space"}]}"#;
        assert!(s.import_text(text, "x").is_err());
        let (p, _) = repair_ranges(text).unwrap();
        assert!(s.import_text(&p.to_json_pretty(), "x").is_ok());
    }

    #[test]
    fn memfs_behaves() {
        let mut fs = MemFs::new();
        assert_eq!(fs.list("d"), Ok(vec![]));
        fs.write_atomic("d/b", "1").unwrap();
        fs.write_atomic("d/a", "2").unwrap();
        fs.write_atomic("d/sub/c", "3").unwrap();
        fs.write_atomic("dd/x", "4").unwrap();
        assert_eq!(fs.list("d").unwrap(), ["a", "b"]);
        assert_eq!(fs.list("d/").unwrap(), ["a", "b"]);
        fs.rename("d/a", "d/b").unwrap();
        assert_eq!(fs.read("d/b").unwrap(), "2");
        assert_eq!(fs.rename("d/zz", "d/y"), Err(FsError::NotFound));
        assert_eq!(fs.remove("d/zz"), Err(FsError::NotFound));
        assert_eq!(fs.read("d/zz"), Err(FsError::NotFound));
        fs.remove("d/b").unwrap();
        assert!(!fs.exists("d/b"));
    }
}
