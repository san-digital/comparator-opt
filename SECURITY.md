# Security

This project is a set of checks that are meant to catch bad changes, so a way to get a bad
change past them counts as a security issue. Examples: a change to a pinned reference that
`verify` does not detect, a proof of the wrong statement that passes the Lean audit, or a
fast path that differs from its reference and still passes the differential tests.

Some routes are already documented in the README and PROOF.md (for example, code outside a
pinned block changing what the compiler builds from it). Those are known limits, not new
reports, but suggestions for closing them are welcome as issues.

## Reporting

Please report privately through GitHub:
[open a security advisory](https://github.com/san-digital/comparator-opt/security/advisories/new).
Do not open a public issue for an unreported problem.

If that link does not work, open an issue that asks a maintainer to get in touch, without any
details of the problem.

## Supported versions

Fixes go into the latest release only.
