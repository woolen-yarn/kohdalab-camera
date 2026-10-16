"""Focused tests for build_windows_installer.py; no NSIS compiler is needed."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("build_windows_installer.py")
SPEC = importlib.util.spec_from_file_location("build_windows_installer", SCRIPT)
assert SPEC and SPEC.loader
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make_payload(root: Path) -> Path:
    payload = root / "windows"
    payload.mkdir()
    runtime = {}
    for name, content in {
        "KohdaLab Camera.exe": b"camera runtime",
        "continuous-helper.exe": b"helper runtime",
    }.items():
        path = payload / name
        path.write_bytes(content)
        runtime[name] = digest(path)
    for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "README.txt"):
        (payload / name).write_text(name, encoding="utf-8")
    licenses = payload / "third-party-licenses" / "dependency"
    licenses.mkdir(parents=True)
    (licenses / "LICENSE").write_text("license", encoding="utf-8")
    (payload / "RELEASE-MANIFEST.json").write_text(json.dumps({
        "product": "KohdaLab Camera",
        "version": "1.2.3",
        "target": "x86_64-pc-windows-gnu",
        "runtime": runtime,
    }), encoding="utf-8")
    return payload


class InstallerPayloadTests(unittest.TestCase):
    def test_cli_rejects_symlink_payload_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = make_payload(root)
            alias = root / "alias"
            alias.symlink_to(payload, target_is_directory=True)
            self.assertEqual(installer.main([
                "--payload", str(alias), "--output", str(root / "setup.exe"),
                "--dry-run",
            ]), 2)

    def test_rejects_tampered_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            payload = make_payload(Path(temporary))
            (payload / "KohdaLab Camera.exe").write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                installer.validate_payload(payload)

    def test_rejects_unsafe_nsis_payload_component(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            payload = make_payload(Path(temporary))
            unsafe = payload / "third-party-licenses" / "dollar$component"
            unsafe.mkdir()
            (unsafe / "LICENSE").write_text("license", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unsafe NSIS"):
                installer.validate_payload(payload)

    def test_generated_uninstall_is_explicit_and_non_recursive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = make_payload(root)
            installer.validate_payload(payload)
            stage, commit, rollback, cleanup_backup, uninstall = installer.render_payload_includes(payload, root / "includes")
            self.assertIn('File "/oname=KohdaLab Camera.exe" ', stage.read_text(encoding="utf-8"))
            commands = uninstall.read_text(encoding="utf-8")
            self.assertIn('Delete "$INSTDIR\\KohdaLab Camera.exe"', commands)
            self.assertIn('RMDir "$INSTDIR\\third-party-licenses\\dependency"', commands)
            self.assertNotIn("RMDir /r", commands)
            self.assertNotIn('RMDir "$INSTDIR"', commands)
            commit_commands = commit.read_text(encoding="utf-8")
            rollback_commands = rollback.read_text(encoding="utf-8")
            marker = '$PLUGINSDIR\\state\\0.attempted'
            self.assertLess(
                commit_commands.index('Rename "$INSTDIR\\KohdaLab Camera.exe"'),
                commit_commands.index(f'FileOpen $0 "{marker}" w'),
            )
            self.assertIn(
                'CopyFiles /SILENT "$PLUGINSDIR\\payload\\KohdaLab Camera.exe" "$INSTDIR"',
                commit_commands,
            )
            self.assertLess(
                rollback_commands.index(f'IfFileExists "{marker}" 0 payload_rollback_'),
                rollback_commands.index('Delete "$INSTDIR\\KohdaLab Camera.exe"'),
            )
            self.assertIn('Delete "$INSTDIR.install-backup\\KohdaLab Camera.exe"', cleanup_backup.read_text(encoding="utf-8"))

    def test_rejects_removed_external_nsis_source_override(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            payload = make_payload(Path(temporary))
            with self.assertRaises(SystemExit) as exit_code:
                installer.main([
                    "--payload", str(payload),
                    "--output", str(Path(temporary) / "setup.exe"),
                    "--nsis-source-root", r"C:\unreviewed-payload",
                    "--dry-run",
                ])
            self.assertEqual(exit_code.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
