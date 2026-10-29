"""Build a verified machine-wide NSIS installer from a packaged Windows payload.

The input must be the ``windows`` directory emitted by package_windows.py. This
tool validates its release manifest before invoking an already-installed NSIS
compiler; it never downloads a compiler or a driver.

Example:
  python scripts/build_windows_installer.py \
    --payload dist/windows --output dist/KohdaLab-Camera-Setup.exe
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_FILES = {
    "KohdaLab Camera.exe",
    "continuous-helper.exe",
    "LICENSE",
    "THIRD_PARTY_NOTICES.md",
    "README.txt",
    "RELEASE-MANIFEST.json",
}
RUNTIME_FILES = {"KohdaLab Camera.exe", "continuous-helper.exe"}
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def nsis_quote(value: str) -> str:
    """Return a strict NSIS string literal for known filesystem paths."""
    if '"' in value or "\r" in value or "\n" in value:
        raise ValueError(f"unsupported character in path: {value!r}")
    return '"' + value.replace("$", "$$") + '"'


def manifest_for(payload: Path) -> dict:
    manifest_path = payload / "RELEASE-MANIFEST.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"invalid release manifest: {error}") from error
    if not isinstance(manifest, dict):
        raise ValueError("release manifest must be an object")
    if manifest.get("product") != "KohdaLab Camera":
        raise ValueError("release manifest product must be 'KohdaLab Camera'")
    version = manifest.get("version")
    if not isinstance(version, str) or not SEMVER.fullmatch(version):
        raise ValueError("release manifest has no supported semantic version")
    if manifest.get("target") != "x86_64-pc-windows-gnu":
        raise ValueError("release manifest target must be x86_64-pc-windows-gnu")
    runtime = manifest.get("runtime")
    if not isinstance(runtime, dict) or set(runtime) != RUNTIME_FILES:
        raise ValueError("release manifest runtime must name exactly the two Camera executables")
    for name, expected_hash in runtime.items():
        if not isinstance(expected_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_hash):
            raise ValueError(f"release manifest has invalid SHA-256 for {name}")
        actual_path = payload / name
        if not actual_path.is_file():
            raise ValueError(f"missing runtime file: {name}")
        if sha256(actual_path) != expected_hash:
            raise ValueError(f"runtime SHA-256 does not match manifest: {name}")
    return manifest


def validate_payload(payload: Path) -> dict:
    # Check the supplied path before resolving it: an installer source symlink
    # can otherwise hide outside the reviewed payload tree.
    if payload.is_symlink():
        raise ValueError("payload directory must not be a symbolic link")
    payload = payload.resolve()
    if not payload.is_dir():
        raise ValueError(f"payload directory does not exist: {payload}")
    missing = sorted(name for name in REQUIRED_FILES if not (payload / name).is_file())
    if missing:
        raise ValueError("payload is missing required files: " + ", ".join(missing))
    licenses = payload / "third-party-licenses"
    if not licenses.is_dir() or not any(path.is_file() for path in licenses.rglob("*")):
        raise ValueError("payload must include a non-empty third-party-licenses directory")
    paths = sorted(payload.rglob("*"), key=lambda path: path.as_posix())
    if any(path.is_symlink() for path in paths):
        raise ValueError("payload must not contain symbolic links")
    if any(not (path.is_file() or path.is_dir()) for path in paths):
        raise ValueError("payload contains an unsupported filesystem entry")
    for path in paths:
        for component in path.relative_to(payload).parts:
            if not component or component in {".", ".."} or any(char in component for char in '$"\\/\r\n'):
                raise ValueError(f"payload contains an unsafe NSIS path component: {component!r}")
    return manifest_for(payload)


def payload_files(payload: Path) -> list[Path]:
    return sorted((path for path in payload.rglob("*") if path.is_file()), key=lambda path: path.relative_to(payload).as_posix())


def render_payload_includes(
    payload: Path,
    work_dir: Path,
) -> tuple[Path, Path, Path, Path, Path]:
    """Render explicit stage/commit/rollback/delete commands.

    Runtime files are first written to NSIS' private plugin directory. The
    commit include backs up only files named by this release to the persistent
    sibling ``$INSTDIR.install-backup``, then copies the staged files. On any
    error the static installer jumps to the rollback include. If Windows will
    not restore a locked file, its backup remains recoverable at that sibling
    path. It never recursively removes the installation directory or unknown
    user files.
    """
    work_dir.mkdir(parents=True, exist_ok=True)
    files = payload_files(payload)
    stage = work_dir / "payload-stage.nsh"
    commit = work_dir / "payload-commit.nsh"
    rollback = work_dir / "payload-rollback.nsh"
    cleanup_backup = work_dir / "payload-cleanup-backup.nsh"
    uninstall = work_dir / "payload-uninstall.nsh"
    stage_lines: list[str] = []
    current_dir: Path | None = None
    for path in files:
        relative = path.relative_to(payload)
        destination_dir = relative.parent
        if destination_dir != current_dir:
            target = "$PLUGINSDIR\\payload" if destination_dir == Path(".") else "$PLUGINSDIR\\payload\\" + "\\".join(destination_dir.parts)
            # This is an NSIS variable expression, not a filesystem string.
            stage_lines.append(f'SetOutPath "{target}"')
            current_dir = destination_dir
        stage_lines.append(f"File {nsis_quote('/oname=' + relative.name)} {nsis_quote(str(path))}")
    commit_lines: list[str] = []
    for index, path in enumerate(files):
        relative = path.relative_to(payload)
        destination = "$INSTDIR\\" + "\\".join(relative.parts)
        staged = "$PLUGINSDIR\\payload\\" + "\\".join(relative.parts)
        backup = "$INSTDIR.install-backup\\" + "\\".join(relative.parts)
        destination_dir = "$INSTDIR" if relative.parent == Path(".") else "$INSTDIR\\" + "\\".join(relative.parent.parts)
        backup_dir = "$INSTDIR.install-backup" if relative.parent == Path(".") else "$INSTDIR.install-backup\\" + "\\".join(relative.parent.parts)
        marker = f"$PLUGINSDIR\\state\\{index}.attempted"
        no_prior = f"payload_no_prior_{index}"
        mark = f"payload_mark_{index}"
        marker_failed = f"payload_marker_failed_{index}"
        next_file = f"payload_commit_{index}_next"
        commit_lines.extend([
            f'CreateDirectory "{destination_dir}"',
            f'IfFileExists "{destination}" 0 {no_prior}',
            f'CreateDirectory "{backup_dir}"',
            "ClearErrors",
            f'Rename "{destination}" "{backup}"',
            "IfErrors payload_transaction_failed",
            f'Goto {mark}',
            f'{no_prior}:',
            f'{mark}:',
            'CreateDirectory "$PLUGINSDIR\\state"',
            "ClearErrors",
            f'FileOpen $0 "{marker}" w',
            f'IfErrors {marker_failed}',
            "FileClose $0",
            "ClearErrors",
            # NSIS CopyFiles uses its second argument as a directory. The
            # source is a single file, so its existing basename is retained.
            f'CopyFiles /SILENT "{staged}" "{destination_dir}"',
            "IfErrors payload_transaction_failed",
            f'Goto {next_file}',
            f'{marker_failed}:',
            # If a previous target was moved but a marker cannot be written,
            # restore it immediately. With no previous target there is
            # nothing to restore and the generic failure path is safe.
            f'IfFileExists "{backup}" 0 payload_transaction_failed',
            "ClearErrors",
            f'Rename "{backup}" "{destination}"',
            "Goto payload_transaction_failed",
            f'{next_file}:',
        ])
    uninstall_lines = [
        'Delete "$INSTDIR\\' + "\\".join(path.relative_to(payload).parts) + '"'
        for path in reversed(files)
    ]
    rollback_lines: list[str] = []
    for index, path in enumerate(reversed(files)):
        relative = path.relative_to(payload)
        destination = "$INSTDIR\\" + "\\".join(relative.parts)
        backup = "$INSTDIR.install-backup\\" + "\\".join(relative.parts)
        destination_dir = "$INSTDIR" if relative.parent == Path(".") else "$INSTDIR\\" + "\\".join(relative.parent.parts)
        original_index = len(files) - index - 1
        marker = f"$PLUGINSDIR\\state\\{original_index}.attempted"
        next_label = f"payload_rollback_{index}_next"
        rollback_lines.extend([
            f'IfFileExists "{marker}" 0 {next_label}',
            f'Delete "{destination}"',
            f'IfFileExists "{backup}" 0 {next_label}',
            f'CreateDirectory "{destination_dir}"',
            f'Rename "{backup}" "{destination}"',
            f'{next_label}:',
        ])
    directories = sorted(
        {path.relative_to(payload) for path in payload.rglob("*") if path.is_dir()},
        key=lambda path: (len(path.parts), path.as_posix()),
        reverse=True,
    )
    uninstall_lines.extend(
        'RMDir "$INSTDIR\\' + "\\".join(directory.parts) + '"'
        for directory in directories
    )
    cleanup_backup_lines = [
        'Delete "$INSTDIR.install-backup\\' + "\\".join(path.relative_to(payload).parts) + '"'
        for path in reversed(files)
    ]
    cleanup_backup_lines.extend(
        'RMDir "$INSTDIR.install-backup\\' + "\\".join(directory.parts) + '"'
        for directory in directories
    )
    cleanup_backup_lines.append('RMDir "$INSTDIR.install-backup"')
    stage.write_text("\n".join(stage_lines) + "\n", encoding="utf-8")
    commit.write_text("\n".join(commit_lines) + "\n", encoding="utf-8")
    rollback.write_text("\n".join(rollback_lines) + "\n", encoding="utf-8")
    cleanup_backup.write_text("\n".join(cleanup_backup_lines) + "\n", encoding="utf-8")
    uninstall.write_text("\n".join(uninstall_lines) + "\n", encoding="utf-8")
    return stage, commit, rollback, cleanup_backup, uninstall


def version_quad(version: str) -> str:
    match = SEMVER.fullmatch(version)
    if not match:
        raise ValueError(f"not a semantic version: {version}")
    return ".".join(match.groups()[:3]) + ".0"


def compiler_path(explicit: str | None) -> str:
    compiler = explicit or shutil.which("makensis")
    if not compiler:
        raise ValueError("NSIS compiler not found; install NSIS or pass --makensis PATH (no download was attempted)")
    return compiler


def build_command(
    compiler: str,
    script: Path,
    output: Path,
    version: str,
    stage: Path,
    commit: Path,
    rollback: Path,
    cleanup_backup: Path,
    uninstall: Path,
) -> list[str]:
    return [
        compiler,
        f"/DOUTPUT_FILE={output}",
        f"/DPRODUCT_VERSION={version}",
        f"/DPRODUCT_VERSION_QUAD={version_quad(version)}",
        f"/DPAYLOAD_STAGE_INCLUDE={stage}",
        f"/DPAYLOAD_COMMIT_INCLUDE={commit}",
        f"/DPAYLOAD_ROLLBACK_INCLUDE={rollback}",
        f"/DPAYLOAD_CLEANUP_BACKUP_INCLUDE={cleanup_backup}",
        f"/DPAYLOAD_UNINSTALL_INCLUDE={uninstall}",
        f"/DICON_FILE={ROOT / 'crates/camera-lab/assets/camera.ico'}",
        str(script),
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--payload", type=Path, required=True, help="packaged windows payload directory")
    parser.add_argument("--output", type=Path, required=True, help="destination setup executable")
    parser.add_argument("--makensis", help="path to an existing makensis executable")
    parser.add_argument("--work-dir", type=Path, help="directory for generated explicit payload includes")
    parser.add_argument("--dry-run", action="store_true", help="validate payload and render includes without invoking NSIS")
    parser.add_argument("--export-uninstaller", type=Path, help="also export the unsigned NSIS uninstaller for signing preparation (Windows compiler)")
    args = parser.parse_args(argv)
    try:
        manifest = validate_payload(args.payload)
        payload = args.payload.resolve()
        output = args.output.resolve()
        if output.suffix.lower() != ".exe":
            raise ValueError("installer output must have an .exe extension")
        if output == payload or payload in output.parents:
            raise ValueError("installer output must be outside the payload directory")
        script = ROOT / "packaging/windows/KohdaLab-Camera.nsi"
        if not script.is_file():
            raise ValueError(f"installer script is missing: {script}")
        context = tempfile.TemporaryDirectory(prefix="kohdalab-camera-nsis-") if args.work_dir is None else None
        work_dir = Path(context.name) if context else args.work_dir.resolve()
        work_dir.mkdir(parents=True, exist_ok=True)
        stage, commit, rollback, cleanup_backup, uninstall = render_payload_includes(payload, work_dir)
        result = {
            "version": manifest["version"],
            "payload": str(payload),
            "output": str(output),
            "stage_include": str(stage),
            "commit_include": str(commit),
            "rollback_include": str(rollback),
            "cleanup_backup_include": str(cleanup_backup),
            "uninstall_include": str(uninstall),
        }
        if args.dry_run:
            print(json.dumps({**result, "dry_run": True}, indent=2))
            return 0
        output.parent.mkdir(parents=True, exist_ok=True)
        if output.exists():
            raise ValueError(f"refusing to overwrite installer output: {output}")
        command = build_command(compiler_path(args.makensis), script, output, manifest["version"], stage, commit, rollback, cleanup_backup, uninstall)
        if args.export_uninstaller:
            exported = args.export_uninstaller.resolve()
            if exported == output or exported.suffix.lower() != '.exe' or exported.exists() or payload == exported or payload in exported.parents:
                raise ValueError("uninstaller export must be a new path outside the payload")
            if any(c in str(exported) for c in '$"%&|<>^\r\n'):
                raise ValueError("unsupported character in uninstaller export path")
            exported.parent.mkdir(parents=True, exist_ok=True)
            command.insert(-1, f"/DEXPORT_UNINSTALLER={exported}")
        subprocess.run(command, check=True)
        if not output.is_file() or output.stat().st_size == 0:
            raise ValueError("NSIS did not produce a non-empty installer")
        if args.export_uninstaller and not exported.is_file():
            raise ValueError("NSIS did not export the uninstaller")
        print(json.dumps({**result, "sha256": sha256(output)}, indent=2))
        return 0
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    finally:
        if 'context' in locals() and context is not None:
            context.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
