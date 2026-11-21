use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use comparator_opt::{
    build_lockfile, verify_kernel, verify_lean_axiom_log, AsciiLowerKernel, LockFile,
    Mersenne31BatchKernel, Utf8CountKernel, VerifyConfig, PIN_TARGETS, REQUIRED_THEOREMS,
};

fn usage() -> &'static str {
    "usage:\n  comparator-opt lock [repo-root]\n  comparator-opt verify [repo-root]\n  comparator-opt bench\n"
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprint!("{}", usage());
        return ExitCode::from(2);
    }

    match args[0].as_str() {
        "lock" => {
            let root = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            run_lock(&root)
        }
        "verify" => {
            let root = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            run_verify(&root)
        }
        "bench" => run_bench(),
        "-h" | "--help" | "help" => {
            print!("{}", usage());
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
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let out_path = root.join("comparator.lock");
    if let Err(e) = fs::write(&out_path, lock.render()) {
        eprintln!("error: cannot write {}: {e}", out_path.display());
        return ExitCode::from(1);
    }
    println!(
        "wrote {} ({} pinned specification blocks)",
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
            eprintln!("error: cannot read {}: {e}", lock_path.display());
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
        "[2/3] Lean 4 axiom footprint verified ({} theorems, zero sorryAx / native_decide)",
        audited.len()
    );

    println!("[3/3] running boundary, non-vacuity, and differential equivalence harness:");
    run_suite_and_print(&VerifyConfig::default())
}

fn run_bench() -> ExitCode {
    let cfg = VerifyConfig {
        samples: 2_000,
        bench_iters: 5_000,
        ..VerifyConfig::default()
    };
    run_suite_and_print(&cfg)
}

fn run_suite_and_print(cfg: &VerifyConfig) -> ExitCode {
    let r1 = match verify_kernel(&AsciiLowerKernel, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let r2 = match verify_kernel(&Utf8CountKernel, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let r3 = match verify_kernel(&Mersenne31BatchKernel, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
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
    ExitCode::SUCCESS
}
