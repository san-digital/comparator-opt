//! The challenge lock: SHA-256 digests of the pinned `@comparator` blocks, and the check of
//! the Lean axiom log.
//!
//! The lock shows that the text between the markers has not changed. It does not show that
//! the compiler builds that text (the rest of the file can, for example, wrap a block in a
//! string literal), and anyone who can edit the repository can regenerate it. It is a review
//! aid: see "What the lock protects" in the README.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use crate::sha256::sha256_hex;

/// The standard Lean 4 axioms a proof may depend on. Anything else (`sorryAx`,
/// `Lean.ofReduceBool` from `native_decide`, `Lean.trustCompiler`, or a custom `axiom`) is
/// rejected.
pub const ALLOWED_LEAN_AXIOMS: &[&str] = &["propext", "Classical.choice", "Quot.sound"];

/// Files the `comparator-opt` binary scans for `@comparator` blocks in this repository, as
/// `(kind, repo-relative path)`. Pass your own list to [`build_lockfile`] for other projects.
pub const PIN_TARGETS: &[(&str, &str)] = &[
    ("rust-ref", "src/kernels.rs"),
    ("lean-challenge", "lean/ComparatorOpt/Challenge.lean"),
    ("lean-challenge", "lean/ComparatorOpt/Check.lean"),
];

/// Theorems from this repository's Lean proofs that must appear in `lean/axioms.txt`.
///
/// The `ComparatorOpt.Audit.*` theorems live in the pinned block of `Check.lean` and restate
/// each proof at its pinned `Challenge.lean` type, so their footprint covers whatever term the
/// optimizer-editable `Proofs.lean` supplies.
pub const REQUIRED_THEOREMS: &[&str] = &[
    "ComparatorOpt.ascii_lower_byte_sound",
    "ComparatorOpt.swar_no_carry_add_sound",
    "ComparatorOpt.utf8_lead_bit_sound",
    "ComparatorOpt.horizontal_byte_sum_sound",
    "ComparatorOpt.mersenne31_fast_sound",
    "ComparatorOpt.Audit.ascii_lower_byte_sound",
    "ComparatorOpt.Audit.swar_no_carry_add_sound",
    "ComparatorOpt.Audit.utf8_lead_bit_sound",
    "ComparatorOpt.Audit.horizontal_byte_sum_sound",
    "ComparatorOpt.Audit.mersenne31_fast_sound",
];

/// One pinned block: its kind, id, SHA-256 digest, and the file it was found in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengePin {
    pub kind: String,
    pub id: String,
    pub sha256: String,
    pub rel_path: String,
}

/// The contents of a lock file such as `comparator.lock`. Pins are kept sorted by kind and id.
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
///
/// Markers are line comments in Rust (`//`) or Lean (`--`) style. The digest covers the lines
/// between the markers, with line endings normalized to `\n` so CRLF and LF checkouts agree.
/// Nested, unclosed, mismatched, or duplicate blocks are errors. The scan is line-based and does
/// not parse Rust or Lean, so a marker inside a string literal or block comment still counts.
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

/// Scans the given `(kind, rel_path)` pairs under `root` and builds a [`LockFile`].
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
    /// Renders the lock file text, one `pin <kind> <id> <sha256> <path>` line per block.
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

    /// Parses lock file text produced by [`LockFile::render`].
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

    /// Checks that `actual` has exactly the same `(kind, id)` pins as `self`, with the same
    /// digests. The file path column is informational and is not compared.
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

/// Reads Lean 4 `#print axioms` output and fails if any theorem depends on an axiom outside
/// [`ALLOWED_LEAN_AXIOMS`], or if a theorem in `required_theorems` is missing.
///
/// This only reads text. It is meaningful when the log was just produced by Lean from the
/// current proofs, as `lean/replay.sh` does in CI.
///
/// Lean wraps long axiom lists across several lines (`[propext,` / ` sorryAx,` / ...), so a
/// list is read until its closing `]`.
pub fn verify_lean_axiom_log(
    axiom_log: &str,
    required_theorems: &[&str],
) -> Result<BTreeMap<String, Vec<String>>, LockError> {
    let mut parsed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut lines = axiom_log.lines().enumerate();

    while let Some((idx, raw)) = lines.next() {
        let line = raw.trim();
        let Some(rest) = line.strip_prefix('\'') else {
            continue;
        };
        let Some((thm, tail)) = rest.split_once('\'') else {
            continue;
        };
        let tail = tail.trim();
        if tail == "does not depend on any axioms" {
            parsed.insert(thm.to_owned(), Vec::new());
            continue;
        }
        let Some(list_part) = tail.strip_prefix("depends on axioms:") else {
            continue;
        };

        let mut list = list_part.trim().to_owned();
        while !list.ends_with(']') {
            let Some((_, cont)) = lines.next() else {
                return Err(LockError::MalformedLock {
                    line: idx + 1,
                    detail: format!("unterminated axiom list for theorem '{thm}'"),
                });
            };
            list.push(' ');
            list.push_str(cont.trim());
        }
        let Some(inner) = list.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
            return Err(LockError::MalformedLock {
                line: idx + 1,
                detail: format!("malformed axiom list for theorem '{thm}'"),
            });
        };

        let axioms: Vec<String> = inner
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

    for &req in required_theorems {
        if !parsed.contains_key(req) {
            return Err(LockError::MissingAxiomAudit {
                theorem: req.to_owned(),
            });
        }
    }

    Ok(parsed)
}
