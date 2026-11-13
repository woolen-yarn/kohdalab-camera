"""Package a clean, committed build as KohdaLab Camera.exe (Python 3.11+).

Usage: python scripts/package_windows.py --target-dir target --output dist
Run both release builds for x86_64-pc-windows-gnu first. Source and binary
archives retain required project and third-party license notices.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib
import zipfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--target-dir', type=Path, default=root / 'target')
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--validation-report', type=Path)
args = parser.parse_args()
version = tomllib.loads((root / 'crates/camera-lab/Cargo.toml').read_text())['package']['version']
if subprocess.check_output(['git', 'status', '--porcelain'], cwd=root).strip():
    raise SystemExit('Commit source changes before packaging.')
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
args.output.mkdir(parents=True, exist_ok=False)
portable = args.output / 'windows'
portable.mkdir()
build = args.target_dir / 'x86_64-pc-windows-gnu/release'
for source, destination in [('camera-lab.exe', 'KohdaLab Camera.exe'),
                            ('continuous-helper.exe', 'continuous-helper.exe')]:
    shutil.copyfile(build / source, portable / destination)
for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md']:
    shutil.copyfile(root / name, portable / name)
shutil.copytree(root / 'third-party-licenses', portable / 'third-party-licenses')
shutil.copytree(root / 'bindings' / 'control', portable / 'bindings' / 'control',
                ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
(portable / 'README.txt').write_text(
    f'KohdaLab Camera {version} — Windows x64\n\n'
    'Extract the entire ZIP and open KohdaLab Camera.exe.\n'
    'Keep continuous-helper.exe and all license files alongside the application.\n'
    'Physical support: SHODENSHA GR300 (tested unit; MT9T001, USB 0547:4d33).\n'
    'An existing WinUSB setup is required; no driver changes are automated.\n'
    'The optional Python control SDK is included under bindings/control.\n'
    'F1–F5 select tabs; Tab and Enter provide keyboard navigation.\n'
    'Optical/color calibration remains unqualified; the owner confirmed manual resize persistence.\n'
    'See the release notes for version-specific tests and five-minute validation.\n'
    'See https://github.com/woolen-yarn/kohdalab-camera for current support and limitations.\n',
    encoding='utf-8')
runtime = {p.name: sha(p) for p in portable.glob('*.exe')}
validation = json.loads(args.validation_report.read_text()) if args.validation_report else None
if validation and validation.get('runtime') != runtime:
    raise SystemExit('Validation report does not match the packaged binaries.')
(portable / 'RELEASE-MANIFEST.json').write_text(json.dumps({
    'product': 'KohdaLab Camera', 'version': version, 'git_commit': commit,
    'target': 'x86_64-pc-windows-gnu', 'validation': validation,
    'runtime': runtime,
}, indent=2))
binary_zip = args.output / f'kohdalab-camera-{version}-windows-x64.zip'
with zipfile.ZipFile(binary_zip, 'x', zipfile.ZIP_DEFLATED) as archive:
    for path in sorted(portable.rglob('*')):
        if path.is_file():
            archive.write(path, path.relative_to(portable))
source_zip = args.output / f'kohdalab-camera-{version}-source.zip'
subprocess.run(['git', 'archive', '--format=zip', '--prefix=kohdalab-camera/',
                '--output=' + str(source_zip.resolve()), 'HEAD'], cwd=root, check=True)
for path in [binary_zip, source_zip]:
    with zipfile.ZipFile(path) as archive:
        assert archive.testzip() is None, path
(args.output / 'SHA256SUMS.txt').write_text(
    ''.join(f'{sha(p)}  {p.name}\n' for p in [binary_zip, source_zip]))
print(json.dumps({'version': version, 'commit': commit, 'output': str(args.output)}))
