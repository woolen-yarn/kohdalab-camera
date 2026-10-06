"""Static extractor for the checked MeasureS InstallShield overlay; never executes PE.

Format references: lifenjoiner/ISx (2017) and pawstas80/Unpacker.
Decoding logic is covered by the notices in docs/third-party-notices.md.
"""
import argparse,hashlib,struct,zlib
from pathlib import Path

p=argparse.ArgumentParser();p.add_argument('setup',type=Path);p.add_argument('output',type=Path)
a=p.parse_args();data=a.setup.read_bytes()
# Bound parsing to the exact reviewed vendor binary, rather than guessing archives.
expected='ea12cfcb0dd9add9dc33fe1fbbdae7069b243983869bba4754d36906ff08cefc'
if hashlib.sha256(data).hexdigest()!=expected:
 raise SystemExit('Unsupported setup.exe hash; inspect format before extracting a different version')
start=data.find(b'InstallShield\0',0x90000)
if start<0:raise SystemExit('Overlay signature missing')
count=struct.unpack_from('<H',data,start+14)[0];offset=start+46
if not 1<=count<=100:raise SystemExit('Invalid entry count')
a.output.mkdir(parents=True,exist_ok=True)
for _ in range(count):
 if offset+312>len(data):raise SystemExit('Truncated entry')
 name=data[offset:offset+260].split(b'\0')[0].decode('cp1252')
 if not name or Path(name).name!=name or '\\' in name or name in ('.','..'):
  raise SystemExit('Unsafe archive name')
 flags,_,size=struct.unpack_from('<III',data,offset+260)
 compressed=struct.unpack_from('<H',data,offset+280)[0];offset+=312
 if size>len(data)-offset:raise SystemExit('Truncated payload')
 payload=data[offset:offset+size];offset+=size
 key=bytes(value^bytes.fromhex('13358607')[i%4] for i,value in enumerate(name.encode('cp1252')))
 if flags&6:
  payload=bytes((~(key[((i%1024) if flags&4 else i)%len(key)]^(((value<<4)|(value>>4))&255)))&255 for i,value in enumerate(payload))
 if compressed:
  inflater=zlib.decompressobj();payload=inflater.decompress(payload,64*1024*1024)
  if not inflater.eof or inflater.unconsumed_tail:raise SystemExit('Invalid or oversized compressed payload')
 with (a.output/name).open('xb') as stream:stream.write(payload)
 print(name,len(payload),hashlib.sha256(payload).hexdigest())
