use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use crate::sha256::sha256_hex;

/// Allowed foundational Lean 4 kernel axioms in zero-trust comparator mode.
pub const ALLOWED_LEAN_AXIOMS: &[&str] = &["propext", "Classical.choice", "Quot.sound"];

/// Single pinned specification block inside a Rust reference file or Lean Challenge file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengePin {
    pub kind: String,
    pub id: String,
    pub sha256: String,
    pub rel_path: String,
}

/// Deterministic Challenge Lock file (`comparator.lock`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockFile {
    pub pins: Vec<ChallengePin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockError {
    Io(String),
    MalformedLock {
        line: usize,
        detail: String,
    },
    UnclosedMarker {
        path: String,
        id: String,
    },
    DuplicateId {
        id: String,
    },
    MissingPin {
        kind: String,
        id: String,
    },
    ExtraPin {
        kind: String,
        id: String,
    },
    DigestMismatch {
        kind: String,
        id: String,
        expected: String,
        actual: String,
    },
    DisallowedAxiom {
        theorem: String,
        axiom: String,
    },
    MissingAxiomAudit {
        theorem: String,
    },
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::MalformedLock { line, detail } => {
                write!(f, "malformed lockfile at line {line}: {detail}")
            }
            Self::UnclosedMarker { path, id } => {
                write!(f, "unclosed @comparator:begin '{id}' in {path}")
            }
            Self::DuplicateId { id } => write!(f, "duplicate @comparator pin id '{id}'"),
            Self::MissingPin { kind, id } => {
                write!(f, "missing pinned {kind} '{id}' from source")
            }
            Self::ExtraPin { kind, id } => {
                write!(f, "unlocked {kind} '{id}' present in source")
            }
            Self::DigestMismatch {
                kind,
                id,
                expected,
                actual,
            } => write!(
                f,
                "TAMPER DETECTED on {kind} '{id}': locked sha256={expected}, actual sha256={actual}"
            ),
            Self::DisallowedAxiom { theorem, axiom } => {
                write!(f, "forbidden axiom '{axiom}' in theorem '{theorem}'")
            }
            Self::MissingAxiomAudit { theorem } => {
                write!(f, "theorem '{theorem}' missing from axiom audit log")
            }
        }
    }
}

impl std::error::Error for LockError {}

/// Extracts all `@comparator:begin <id>` .. `@comparator:end <id>` blocks from `content`.
/// Supports both Rust (`//`) and Lean (`--`) line comments.
pub fn extract_pins_from_str(
    kind: &str,
    rel_path: &str,
    content: &str,
) -> Result<Vec<ChallengePin>, LockError> {
    let mut pins = Vec::new();
    let mut seen = BTreeSet::new();
    let mut active: Option<(String, String)> = None;

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if let Some(id) = parse_marker(trimmed, "begin") {
            if let Some((open_id, _)) = active {
                return Err(LockError::UnclosedMarker {
                    path: rel_path.to_owned(),
                    id: open_id,
                });
            }
            if !seen.insert(id.to_owned()) {
                return Err(LockError::DuplicateId { id: id.to_owned() });
            }
            active = Some((id.to_owned(), String::new()));
            continue;
        }
        if let Some(id) = parse_marker(trimmed, "end") {
            let Some((open_id, buf)) = active.take() else {
                return Err(LockError::MalformedLock {
                    line: 0,
                    detail: format!("unexpected @comparator:end '{id}' in {rel_path}"),
                });
            };
            if open_id != id {
                return Err(LockError::UnclosedMarker {
                    path: rel_path.to_owned(),
                    id: open_id,
                });
            }
            let digest = sha256_hex(buf.as_bytes());
            pins.push(ChallengePin {
                kind: kind.to_owned(),
                id: open_id,
                sha256: digest,
                rel_path: rel_path.to_owned(),
            });
            continue;
        }

        if let Some((_, ref mut buf)) = active {
            buf.push_str(raw_line.trim_end_matches('\r'));
            buf.push('\n');
        }
    }

    if let Some((open_id, _)) = active {
        return Err(LockError::UnclosedMarker {
            path: rel_path.to_owned(),
            id: open_id,
        });
    }

    Ok(pins)
}

fn parse_marker<'a>(trimmed: &'a str, action: &str) -> Option<&'a str> {
    for prefix in ["//", "--"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let rest = rest.trim();
            if let Some(after) = rest.strip_prefix("@comparator:") {
                if let Some(id_part) = after.strip_prefix(action) {
                    let id = id_part.trim();
                    if !id.is_empty() {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

/// Scans the given `(kind, rel_path)` pairs under `root` and builds a `LockFile`.
pub fn build_lockfile(root: &Path, targets: &[(&str, &str)]) -> Result<LockFile, LockError> {
    let mut pins = Vec::new();
    let mut seen = BTreeSet::new();
    for &(kind, rel) in targets {
        let full = root.join(rel);
        let content = fs::read_to_string(&full)
            .map_err(|e| LockError::Io(format!("{}: {e}", full.display())))?;
        let extracted = extract_pins_from_str(kind, rel, &content)?;
        for p in extracted {
            if !seen.insert((p.kind.clone(), p.id.clone())) {
                return Err(LockError::DuplicateId { id: p.id });
            }
            pins.push(p);
        }
    }
    pins.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.id.cmp(&b.id)));
    Ok(LockFile { pins })
}

impl LockFile {
    pub fn render(&self) -> String {
        let mut out = String::from("# comparator-opt challenge lock (version 1)\nversion = 1\n");
        for p in &self.pins {
            out.push_str(&format!(
                "pin {} {} {} {}\n",
                p.kind, p.id, p.sha256, p.rel_path
            ));
        }
        out
    }

    pub fn parse(src: &str) -> Result<Self, LockError> {
        let mut pins = Vec::new();
        let mut saw_version = false;
        for (idx, raw) in src.lines().enumerate() {
            let ln = idx + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "version = 1" {
                saw_version = true;
                continue;
            }
            let Some(rest) = line.strip_prefix("pin ") else {
                return Err(LockError::MalformedLock {
                    line: ln,
                    detail: format!("unexpected entry '{line}'"),
                });
            };
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 4
                || parts[2].len() != 64
                || !parts[2]
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            {
                return Err(LockError::MalformedLock {
                    line: ln,
                    detail: "expected 'pin <kind> <id> <64-char-lowercase-hex-sha256> <rel_path>'"
                        .to_owned(),
                });
            }
            if pins
                .iter()
                .any(|p: &ChallengePin| p.kind == parts[0] && p.id == parts[1])
            {
                return Err(LockError::DuplicateId {
                    id: parts[1].to_owned(),
                });
            }
            pins.push(ChallengePin {
                kind: parts[0].to_owned(),
                id: parts[1].to_owned(),
                sha256: parts[2].to_owned(),
                rel_path: parts[3].to_owned(),
            });
        }
        if !saw_version {
            return Err(LockError::MalformedLock {
                line: 1,
                detail: "missing 'version = 1' header".to_owned(),
            });
        }
        pins.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.id.cmp(&b.id)));
        Ok(Self { pins })
    }

    /// Verifies that `actual` matches every locked `(kind, id, sha256)` entry without tampering.
    pub fn verify_against(&self, actual: &Self) -> Result<(), LockError> {
        let locked_map: BTreeMap<(&str, &str), &ChallengePin> = self
            .pins
            .iter()
            .map(|p| ((p.kind.as_str(), p.id.as_str()), p))
            .collect();
        let actual_map: BTreeMap<(&str, &str), &ChallengePin> = actual
            .pins
            .iter()
            .map(|p| ((p.kind.as_str(), p.id.as_str()), p))
            .collect();

        for (&(kind, id), lp) in &locked_map {
            let Some(ap) = actual_map.get(&(kind, id)) else {
                return Err(LockError::MissingPin {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                });
            };
            if lp.sha256 != ap.sha256 {
                return Err(LockError::DigestMismatch {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                    expected: lp.sha256.clone(),
                    actual: ap.sha256.clone(),
                });
            }
        }

        for &(kind, id) in actual_map.keys() {
            if !locked_map.contains_key(&(kind, id)) {
                return Err(LockError::ExtraPin {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                });
            }
        }
        Ok(())
    }
}

/// Parses `#print axioms` lines from Lean 4 and rejects `sorryAx`, `Lean.ofReduceBool`
/// (`native_decide`), `Lean.trustCompiler`, and any non-standard user axioms.
pub fn verify_lean_axiom_log(
    axiom_log: &str,
    required_theorems: &[&str],
) -> Result<BTreeMap<String, Vec<String>>, LockError> {
    let mut parsed: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for raw in axiom_log.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix('\'') {
            if let Some((thm, tail)) = rest.split_once('\'') {
                let tail = tail.trim();
                if tail == "does not depend on any axioms" {
                    parsed.insert(thm.to_owned(), Vec::new());
                } else if let Some(list_part) = tail.strip_prefix("depends on axioms:") {
                    let clean = list_part
                        .trim()
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    let axioms: Vec<String> = clean
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    for ax in &axioms {
                        if !ALLOWED_LEAN_AXIOMS.contains(&ax.as_str()) {
                            return Err(LockError::DisallowedAxiom {
                                theorem: thm.to_owned(),
                                axiom: ax.clone(),
                            });
                        }
                    }
                    parsed.insert(thm.to_owned(), axioms);
                }
            }
        }
    }

    for &req in required_theorems {
        if !parsed.contains_key(req) {
            return Err(LockError::MissingAxiomAudit {
                theorem: req.to_owned(),
            });
        }
    }

    Ok(parsed)
}
