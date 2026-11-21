# Contributing

Thanks for helping. Issues and pull requests are welcome.

## Before you open a pull request

Run the same checks as CI:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
RUSTFLAGS="-D warnings" cargo test --release
cargo run --release -- verify .
python3 -I scripts/crosscheck.py
```

If you changed anything under `lean/`, also run `./lean/replay.sh` (needs
[elan](https://github.com/leanprover/elan)) and commit the regenerated `lean/axioms.txt`.

## Pinned blocks

Code between `@comparator:begin` and `@comparator:end` markers is the specification. If you
change it on purpose, run `cargo run --release -- lock .`, commit the new `comparator.lock` in
the same pull request, and say in the description which blocks changed and why. A pull request
that changes a fast path should not change the lock.

## Style checks

`tests/hygiene.rs` fails the build on:

- any runtime dependency in `Cargo.toml`, or `unsafe` code;
- typographic characters such as em and en dashes, curly quotes, non-breaking spaces, and
  zero-width characters (use plain ASCII punctuation);
- a short list of banned words and phrases. The list is in the test.

## License

By contributing, you agree that your contributions are licensed under the Apache License 2.0,
the same as the project.
