"""Defaults to displaying the inferred chip-read request; --execute sends it."""
import argparse,json
from dataclasses import asdict
from kohdalab_camera.legacy_protocol import register_read, probe_chip
p=argparse.ArgumentParser()
p.add_argument('--execute',action='store_true',help='Attempt one inferred sensor chip-ID read')
p.add_argument('--prepare',action='store_true',help='Apply recovered bridge/output setup, then stop after reading')
a=p.parse_args()
if a.execute:
 try: print(json.dumps(probe_chip(prepare=a.prepare),indent=2))
 except Exception as e: p.exit(1,f'{e}\n')
else:
 r=register_read()
 print(json.dumps({**asdict(r),'windows_block_hex':r.windows_block().hex(),'executed':False},indent=2))
