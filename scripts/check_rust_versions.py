#!/usr/bin/env python3
"""Check Rust versions named in docs/development/continuous-integration.md.

Read-only. Every "Rust X.Y" or "Rust X.Y.Z" in that document (optionally in
bold or backticks) must equal the Cargo workspace MSRV or a numeric Rust pin
in .github/workflows/*.yml (rustup install, cargo +X, RUST_VERSION).
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

DOC = Path("docs/development/continuous-integration.md")
VERSION = r"(\d+\.\d+(?:\.\d+)?)(?!\.?\d)"
MENTION = re.compile(rf"(?<![A-Za-z0-9_])Rust\s+(?:\*\*|`)?{VERSION}")
PINS = (
    re.compile(rf"\brustup\s+(?:toolchain\s+)?install\s+['\"]?{VERSION}"),
    re.compile(rf"\bcargo\s+\+{VERSION}"),
    re.compile(rf"\bRUST_VERSION\s*:\s*['\"]?{VERSION}"),
)


def allowed_versions(root: Path) -> set[str]:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    versions = {cargo["workspace"]["package"]["rust-version"]}
    for workflow in sorted((root / ".github" / "workflows").glob("*.yml")):
        text = workflow.read_text(encoding="utf-8")
        for pattern in PINS:
            versions.update(match.group(1) for match in pattern.finditer(text))
    return versions


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    root = parser.parse_args(argv).root
    allowed = allowed_versions(root)
    errors = []
    for number, line in enumerate(
        (root / DOC).read_text(encoding="utf-8").splitlines(), start=1
    ):
        for match in MENTION.finditer(line):
            if match.group(1) not in allowed:
                errors.append(f"{DOC.as_posix()}:{number}: Rust {match.group(1)}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(
            "FAIL: not the Cargo MSRV or a workflow pin "
            f"(allowed: {', '.join(sorted(allowed))})",
            file=sys.stderr,
        )
        return 1
    print(f"PASS: {DOC.as_posix()} Rust versions match {', '.join(sorted(allowed))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
