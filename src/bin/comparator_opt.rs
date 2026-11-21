//! Command-line checker for the example kernels in this repository.
//!
//! The kernels and the list of pinned files are compiled into the binary, so the commands only
//! make sense inside a checkout of the comparator-opt repository. Exit codes: `0` when every
//! check passes, `1` when a check fails or a file cannot be read or written, `2` for a usage
//! error.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use comparator_opt::{
    build_lockfile, verify_kernel, verify_lean_axiom_log, AsciiLowerKernel, LockFile,
    Mersenne31BatchKernel, Utf8CountKernel, VerifyConfig, PIN_TARGETS, REQUIRED_THEOREMS,
};

const REPO_HINT: &str =
    "comparator-opt only checks a checkout of https://github.com/san-digital/comparator-opt";

fn usage() -> &'static str {
    concat!(
        "comparator-opt ",
        env!("CARGO_PKG_VERSION"),
        ": checks the example kernels in a checkout of the comparator-opt repository.\n",
        "\n",
        "usage:\n",
        "  comparator-opt verify [repo-root]  check comparator.lock and lean/axioms.txt, then run the\n",
        "                                     differential tests and benchmark\n",
        "  comparator-opt lock [repo-root]    rewrite comparator.lock from the current pinned blocks\n",
        "  comparator-opt bench               run the differential tests with more benchmark iterations\n",
        "  comparator-opt --version\n",
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprint!("{}", usage());
        return ExitCode::from(2);
    };
    let max_args = match cmd.as_str() {
        "lock" | "verify" => 2,
        _ => 1,
    };
    if args.len() > max_args {
        eprintln!("error: too many arguments for '{cmd}'\n{}", usage());
        return ExitCode::from(2);
    }
    let root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    match cmd.as_str() {
        "lock" => run_lock(&root),
        "verify" => run_verify(&root),
        "bench" => run_bench(),
        "-h" | "--help" | "help" => {
            print!("{}", usage());
            ExitCode::SUCCESS
        }
        "-V" | "--version" => {
            println!("comparator-opt {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown subcommand '{other}'\n{}", usage());
            ExitCode::from(2)
        }
    }
}

fn run_lock(root: &Path) -> ExitCode {
    let lock = match build_lockfile(root, PIN_TARGETS) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}\n({REPO_HINT})");
            return ExitCode::from(1);
        }
    };
    let out_path = root.join("comparator.lock");
    if let Err(e) = fs::write(&out_path, lock.render()) {
        eprintln!("error: cannot write {}: {e}", out_path.display());
        return ExitCode::from(1);
    }
    println!(
        "wrote {} ({} pinned blocks); review the pinned changes before committing it",
        out_path.display(),
        lock.pins.len()
    );
    ExitCode::SUCCESS
}

fn run_verify(root: &Path) -> ExitCode {
    let lock_path = root.join("comparator.lock");
    let lock_src = match fs::read_to_string(&lock_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: cannot read {}: {e}\n({REPO_HINT})",
                lock_path.display()
            );
            return ExitCode::from(1);
        }
    };
    let locked = match LockFile::parse(&lock_src) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let actual = match build_lockfile(root, PIN_TARGETS) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = locked.verify_against(&actual) {
        eprintln!("challenge lock failed: {e}");
        return ExitCode::from(1);
    }
    println!(
        "[1/3] challenge lock verified ({} pinned blocks match SHA-256)",
        locked.pins.len()
    );

    let axiom_path = root.join("lean/axioms.txt");
    let axiom_src = match fs::read_to_string(&axiom_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", axiom_path.display());
            return ExitCode::from(1);
        }
    };
    let audited = match verify_lean_axiom_log(&axiom_src, REQUIRED_THEOREMS) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("axiom audit failed: {e}");
            return ExitCode::from(1);
        }
    };
    println!(
        "[2/3] lean/axioms.txt lists {} theorems using only standard axioms (lean/replay.sh regenerates it)",
        audited.len()
    );

    println!("[3/3] running boundary, non-vacuity, and differential tests:");
    if !run_suite_and_print(&VerifyConfig::default()) {
        return ExitCode::from(1);
    }
    println!("verify: ok");
    ExitCode::SUCCESS
}

fn run_bench() -> ExitCode {
    let cfg = VerifyConfig {
        samples: 2_000,
        bench_iters: 5_000,
        ..VerifyConfig::default()
    };
    if run_suite_and_print(&cfg) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Runs every example kernel through `verify_kernel` and prints one line each.
/// Returns `false` (after printing the error) on the first failure.
fn run_suite_and_print(cfg: &VerifyConfig) -> bool {
    let r1 = match verify_kernel(&AsciiLowerKernel, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return false;
        }
    };
    let r2 = match verify_kernel(&Utf8CountKernel, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return false;
        }
    };
    let r3 = match verify_kernel(&Mersenne31BatchKernel, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return false;
        }
    };

    for r in [r1, r2, r3] {
        println!(
            "  kernel={:<22} valid={}/{} ({} bps) ref={:>8.1} ns  fast={:>8.1} ns  speedup={:>5.2}x",
            r.kernel,
            r.sampled_valid,
            r.sampled_total,
            r.valid_bps,
            r.ref_ns_per_call,
            r.fast_ns_per_call,
            r.speedup_x
        );
    }
    true
}
