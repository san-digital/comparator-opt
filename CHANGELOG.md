# Changelog

All notable changes to this project are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

### Changed

- The CLI prints a final `verify: ok` line, accepts `--version`, rejects extra arguments, and
  explains that it only checks a checkout of this repository when it cannot find its files.
- The `[2/3]` line of `verify` now says it read `lean/axioms.txt`, since it does not run Lean.
- README rewritten: what the crate is for, a quick start, how to use the library on your own
  code, what the lock does and does not protect, and the limits of the speedup figures.
- PROOF.md now says plainly what the Lean proofs cover and what they do not.
- CI pins actions to commit SHAs, runs the Lean replay in its own job, and tests the minimum
  supported Rust version (1.81).

### Added

- Tests for SHA-256 padding boundaries and for the block parser (CRLF, nesting, duplicates,
  stray and mismatched markers).
- CHANGELOG.md, CONTRIBUTING.md, SECURITY.md, CODEOWNERS, and Dependabot for GitHub Actions.

## 0.1.1 - 2026-11-21

### Fixed

- CI now regenerates `lean/axioms.txt` with `lean/replay.sh` and fails if it differs from the
  committed copy. Before, `verify` only read the committed file, so a `sorry` in a proof went
  unnoticed.
- The pinned `Check.lean` binds every proof to its pinned statement, so a proof of a weaker
  statement no longer passes.
- Each `KernelSpec` impl now sits inside its pinned block, so `eval_ref` can no longer be
  pointed at the fast path without changing the lock.
- The axiom log parser reads lists that Lean wraps across several lines.

## 0.1.0 - 2026-11-21

- First release: three example kernels, the differential test harness, the SHA-256 lock, and
  Lean 4 proofs.
