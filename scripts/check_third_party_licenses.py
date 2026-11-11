"""Check the shipped third-party inventory against locked Cargo metadata."""

import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
TOOL_LICENSES = {"gcc-runtime", "mingw-w64", "nsis-3.12", "rust-standard-library"}
TABLE_HEADER = "| Crate | Version | License | Source |"


def notice_packages():
    lines = (ROOT / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8").splitlines()
    if lines.count(TABLE_HEADER) != 1:
        raise ValueError("expected one crate inventory table in THIRD_PARTY_NOTICES.md")

    packages = {}
    for line in lines[lines.index(TABLE_HEADER) + 2:]:
        if not line.startswith("|"):
            break
        fields = [field.strip() for field in line.strip("|").split("|")]
        if len(fields) != 4 or not all(fields):
            raise ValueError(f"malformed notice row: {line}")
        name, version, license_expression, source = fields
        key = (name, version)
        if key in packages:
            raise ValueError(f"duplicate notice row: {name} {version}")
        packages[key] = (license_expression, source)
    return packages


def report(label, entries):
    print(f"{label}: {len(entries)}", flush=True)
    for entry in sorted(entries):
        print(f"  {entry}", flush=True)


def main():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT,
    ))
    expected = {
        (package["name"], package["version"]): (package["license"], package["source"])
        for package in metadata["packages"] if package["source"] is not None
    }
    expected_dirs = {f"{name}-{version}" for name, version in expected} | TOOL_LICENSES
    actual_dirs = {
        path.name for path in (ROOT / "third-party-licenses").iterdir() if path.is_dir()
    }
    notices = notice_packages()
    missing_dirs = expected_dirs - actual_dirs
    extra_dirs = actual_dirs - expected_dirs
    empty_crate_dirs = {
        f"{name}-{version}"
        for name, version in expected
        if f"{name}-{version}" in actual_dirs
        and not any(
            path.is_file() and path.name != "RECOVERY.tsv"
            for path in (ROOT / "third-party-licenses" / f"{name}-{version}").rglob("*")
        )
    }
    missing_notices = expected.keys() - notices.keys()
    extra_notices = notices.keys() - expected.keys()
    version_mismatches = {name for name, _ in missing_notices} & {
        name for name, _ in extra_notices
    }
    metadata_mismatches = {
        f"{name} {version}"
        for name, version in expected.keys() & notices.keys()
        if expected[name, version] != notices[name, version]
    }

    print(f"Checking {len(expected)} locked external crates and {len(TOOL_LICENSES)} tool licenses")
    report("Missing license directories", missing_dirs)
    report("Extra license directories", extra_dirs)
    report("Crate directories without license files", empty_crate_dirs)
    report("Missing notice rows", {f"{name} {version}" for name, version in missing_notices})
    report("Extra notice rows", {f"{name} {version}" for name, version in extra_notices})
    report("Notice version mismatches", version_mismatches)
    report("Notice license or source mismatches", metadata_mismatches)
    return int(any((missing_dirs, extra_dirs, empty_crate_dirs, missing_notices, extra_notices, metadata_mismatches)))


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Third-party license check failed: {error}") from error
