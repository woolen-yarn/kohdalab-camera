"""Fetch the pinned official NSIS compiler for a disposable Windows build runner."""
from pathlib import Path
import argparse
import hashlib
import io
import urllib.request
import zipfile

SHA256 = '56581f90db321581c5381193d796fffcf2d24b2f8fed2160a6c6a3baa67f2c4f'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
if args.output.exists():
    raise SystemExit('Compiler destination must be new.')
urls = [
    'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.12/nsis-3.12.zip',
]
data = None
errors = []
for url in urls:
    try:
        with urllib.request.urlopen(url, timeout=90) as response:
            candidate = response.read(16 * 1024 * 1024 + 1)
        if hashlib.sha256(candidate).hexdigest() != SHA256:
            raise ValueError('NSIS archive hash mismatch')
        data = candidate
        break
    except Exception as error:
        errors.append(str(error))
if data is None:
    raise SystemExit('Unable to fetch pinned NSIS: ' + '; '.join(errors))
with zipfile.ZipFile(io.BytesIO(data)) as archive:
    for item in archive.infolist():
        path = Path(item.filename)
        if path.is_absolute() or '..' in path.parts or ':' in item.filename or '\\' in item.filename:
            raise SystemExit('Unsafe compiler archive path')
    archive.extractall(args.output)
print(f'NSIS 3.12 SHA-256 {SHA256}')
