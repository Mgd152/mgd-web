//! Offline simulator for keymap profiles: validate, list keys, replay scripts.

use keymap::geom::{Rotation, View};
use keymap::keys::{KEY_NAMES, MOUSE_NAMES, is_known_key};
use keymap::{Engine, EngineEvent, Frame, Input, Orientation, Profile, ProfileError};
use std::process::ExitCode;

const USAGE: &str = "usage:
  keymap-sim validate <profile.json>
  keymap-sim keys
  keymap-sim simulate <profile.json> <script.txt> [--rotation R0|R90|R180|R270] [--panel WxH]";

enum Failure {
    Usage,
    Error(String),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Error(msg)) => {
            for line in msg.lines() {
                eprintln!("error: {line}");
            }
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    match args.first().map(String::as_str) {
        Some("validate") if args.len() == 2 => validate(&args[1]),
        Some("keys") if args.len() == 1 => {
            for k in KEY_NAMES.iter().chain(MOUSE_NAMES) {
                println!("{k}");
            }
            Ok(())
        }
        Some("simulate") if args.len() >= 3 => simulate(&args[1..]),
        _ => Err(Failure::Usage),
    }
}

fn read(path: &str) -> Result<String, Failure> {
    std::fs::read_to_string(path).map_err(|e| Failure::Error(format!("cannot read {path}: {e}")))
}

/// Parse a profile; on failure the message lists every problem as `path: message`.
fn load_profile(path: &str) -> Result<Profile, Failure> {
    let text = read(path)?;
    match Profile::from_json(&text) {
        Ok(p) => Ok(p),
        Err(ProfileError::Json(m)) => Err(Failure::Error(format!("{path}: invalid JSON: {m}"))),
        Err(ProfileError::Invalid(_)) => {
            // Re-parse leniently to list all problems.
            let raw: Profile = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text))
                .map_err(|e| Failure::Error(e.to_string()))?;
            let all: Vec<String> = raw
                .problems()
                .iter()
                .map(|p| format!("{}: {}", p.path, p.message))
                .collect();
            Err(Failure::Error(all.join("\n")))
        }
    }
}

fn validate(path: &str) -> Result<(), Failure> {
    let p = load_profile(path)?;
    println!(
        "OK: {} ({}, {} controls)",
        p.name,
        p.orientation.as_str(),
        p.controls.len()
    );
    Ok(())
}

fn parse_rotation(s: &str) -> Option<Rotation> {
    match s {
        "R0" => Some(Rotation::R0),
        "R90" => Some(Rotation::R90),
        "R180" => Some(Rotation::R180),
        "R270" => Some(Rotation::R270),
        _ => None,
    }
}

fn parse_panel(s: &str) -> Option<(f64, f64)> {
    let (w, h) = s.split_once('x')?;
    let (w, h): (f64, f64) = (w.parse().ok()?, h.parse().ok()?);
    (w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()).then_some((w, h))
}

fn print_frame(t: u64, f: &Frame) {
    let parts: Vec<String> = f
        .contacts()
        .iter()
        .map(|c| {
            format!(
                "{}:{}@{},{}",
                c.id,
                if c.touching { 'D' } else { 'U' },
                c.x,
                c.y
            )
        })
        .collect();
    println!(
        "{t:>8} {} {}",
        if f.has_transition() { 'T' } else { 'M' },
        parts.join(" ")
    );
}

fn print_event(t: u64, ev: &EngineEvent) {
    match ev {
        EngineEvent::Activated => println!("{t:>8} # activated"),
        EngineEvent::Deactivated => println!("{t:>8} # deactivated"),
        EngineEvent::Panic => println!("{t:>8} # panic"),
        EngineEvent::ActivateFailed(r) => println!("{t:>8} # activate-failed: {r}"),
    }
}

fn simulate(args: &[String]) -> Result<(), Failure> {
    let mut positional = Vec::new();
    let mut rotation: Option<Rotation> = None;
    let mut panel = (828.0, 1792.0);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rotation" => {
                let v = it.next().ok_or(Failure::Usage)?;
                rotation = Some(
                    parse_rotation(v)
                        .ok_or_else(|| Failure::Error(format!("bad --rotation {v}")))?,
                );
            }
            "--panel" => {
                let v = it.next().ok_or(Failure::Usage)?;
                panel = parse_panel(v)
                    .ok_or_else(|| Failure::Error(format!("bad --panel {v} (want WxH)")))?;
            }
            s if s.starts_with("--") => return Err(Failure::Usage),
            _ => positional.push(a.clone()),
        }
    }
    if positional.len() != 2 {
        return Err(Failure::Usage);
    }
    let profile = load_profile(&positional[0])?;
    let script = read(&positional[1])?;
    let rotation = rotation.unwrap_or(match profile.orientation {
        Orientation::Portrait => Rotation::R0,
        Orientation::Landscape => Rotation::R90,
    });
    let mut engine = Engine::new(profile).map_err(|e| Failure::Error(e.to_string()))?;
    let mut t: u64 = 0;
    engine.set_view(View::from_panel(panel.0, panel.1, rotation));

    for (idx, raw) in script.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let err = |m: String| Failure::Error(format!("script line {line_no}: {m}"));
        let toks: Vec<&str> = line.split_whitespace().collect();
        match (toks[0], toks.len()) {
            ("press" | "release", 2) => {
                if !is_known_key(toks[1]) {
                    return Err(err(format!("unknown key \"{}\"", toks[1])));
                }
                let pressed = toks[0] == "press";
                let out = engine.handle(
                    if pressed {
                        Input::Press(toks[1])
                    } else {
                        Input::Release(toks[1])
                    },
                    t,
                );
                emit(t, &out.frames);
                if let Some(ev) = &out.event {
                    print_event(t, ev);
                }
                if !out.consumed {
                    println!("{t:>8} # not-consumed: {} {}", toks[0], toks[1]);
                }
            }
            ("move", 3) => {
                let dx: f64 = toks[1]
                    .parse()
                    .map_err(|_| err(format!("bad number \"{}\"", toks[1])))?;
                let dy: f64 = toks[2]
                    .parse()
                    .map_err(|_| err(format!("bad number \"{}\"", toks[2])))?;
                let out = engine.handle(Input::MouseMotion { dx, dy }, t);
                emit(t, &out.frames);
            }
            ("wait", 2) => {
                let ms: u64 = toks[1]
                    .parse()
                    .map_err(|_| err(format!("bad duration \"{}\"", toks[1])))?;
                let end = t.saturating_add(ms);
                while let Some(d) = engine.next_deadline() {
                    if d > end {
                        break;
                    }
                    t = t.max(d);
                    emit(t, &engine.tick(d));
                }
                t = end;
            }
            ("focus-lost", 1) => {
                let was = engine.is_active();
                if let Some(f) = engine.deactivate() {
                    print_frame(t, &f);
                }
                if was {
                    println!("{t:>8} # deactivated");
                }
            }
            ("disconnect", 1) => {
                let n = engine.deactivate().map_or(0, |f| f.contacts().len());
                println!("{t:>8} # disconnected: {n} contacts dropped without a lift frame");
            }
            ("rotate", 2) => {
                let r = parse_rotation(toks[1])
                    .ok_or_else(|| err(format!("bad rotation \"{}\"", toks[1])))?;
                let was = engine.is_active();
                emit(t, &engine.set_view(View::from_panel(panel.0, panel.1, r)));
                if was && !engine.is_active() {
                    println!("{t:>8} # deactivated");
                }
            }
            _ => return Err(err(format!("cannot parse \"{line}\""))),
        }
    }
    let s = engine.stats();
    println!(
        "# frames={} dropped_presses={} look_recenters={} active={} contacts_down={}",
        s.frames,
        s.dropped_presses,
        s.look_recenters,
        engine.is_active(),
        engine.contacts_down()
    );
    Ok(())
}

fn emit(t: u64, frames: &[Frame]) {
    for f in frames {
        print_frame(t, f);
    }
}
