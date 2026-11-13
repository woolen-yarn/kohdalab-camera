"""Record unsigned build origin and hash final distribution/signing artifacts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--distribution', type=Path, required=True)
parser.add_argument('--signing-inputs', type=Path, required=True)
args = parser.parse_args()
manifest = json.loads((args.distribution/'windows/RELEASE-MANIFEST.json').read_text())
commit = subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
if manifest['git_commit'] != commit:
    raise SystemExit('Payload commit does not match checkout')
origin = {
    'version': manifest['version'], 'source_commit': commit,
    'repository': os.environ.get('GITHUB_REPOSITORY'),
    'run_id': os.environ.get('GITHUB_RUN_ID'),
    'run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
    'workflow_ref': os.environ.get('GITHUB_WORKFLOW_REF'),
    'source_identity': Path('RECORDER-SOURCE-ID').read_text().strip(),
    'rustc': subprocess.check_output(['rustc','-Vv'],text=True).strip(),
    'cargo': subprocess.check_output(['cargo','-V'],text=True).strip(),
    'status': 'unsigned; no SignPath enrollment or origin verification claimed',
}
for directory in [args.distribution,args.signing_inputs]:
    (directory/'BUILD-ORIGIN.json').write_text(json.dumps(origin,indent=2)+'\n')
    files = sorted(p for p in directory.iterdir() if p.is_file() and p.name != 'SHA256SUMS.txt')
    (directory/'SHA256SUMS.txt').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in files))
print(json.dumps(origin,indent=2))
