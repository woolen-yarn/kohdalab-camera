"""Refresh the diagnostic source identity from build inputs before rebuilding.

The marker excludes itself to avoid a circular hash. Git commit and executable
hashes in release manifests remain the authoritative distribution identities.
"""
from pathlib import Path
import hashlib

root = Path(__file__).resolve().parents[1]
paths = [root / name for name in ['Cargo.toml', 'Cargo.lock', 'build.rs']]
for folder in ['src', 'helper-entry', 'platform', 'assets']:
    paths.extend(p for p in (root / folder).rglob('*') if p.is_file()
                 and p.suffix in {'.rs', '.toml', '.lock', '.rc', '.manifest', '.ico', '.png'})
digest = hashlib.sha256()
for path in sorted(set(paths)):
    digest.update(path.relative_to(root).as_posix().encode() + b'\0')
    digest.update(path.read_bytes() + b'\0')
marker = digest.hexdigest()
(root / 'RECORDER-SOURCE-ID').write_text(marker + '\n')
print(marker)
