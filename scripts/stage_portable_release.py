"""Stage versioned release assets and one combined checksum manifest."""
from pathlib import Path
import hashlib
import shutil
import tomllib
from zipfile import ZipFile

ROOT=Path(__file__).resolve().parents[1]
version=tomllib.loads((ROOT/'pyproject.toml').read_text())['project']['version']
destination=ROOT/'releases'/f'portable-v{version}'
destination.mkdir(parents=True,exist_ok=True)
checksums=[]
for platform in ['windows-x64','macos-arm64']:
    source=ROOT/'dist'/f'kohdalab-camera-{platform}-portable.zip'
    with ZipFile(source) as archive:
        invalid=archive.testzip()
        if invalid:raise RuntimeError(f'Invalid archive entry: {invalid}')
        if not any(name.endswith('support/SOURCE.zip') for name in archive.namelist()):
            raise RuntimeError(f'Missing source bundle: {source}')
    target=destination/f'kohdalab-camera-{version}-{platform}-portable.zip'
    shutil.copy2(source,target)
    checksums.append(f'{hashlib.sha256(target.read_bytes()).hexdigest()}  {target.name}\n')
(destination/'SHA256SUMS.txt').write_text(''.join(checksums))
print(destination)
