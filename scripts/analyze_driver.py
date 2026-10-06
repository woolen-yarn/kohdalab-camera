"""Static PE inspection; never loads or executes vendor binaries."""
import argparse,json,re
from pathlib import Path
import pefile
from capstone import Cs,CS_ARCH_X86,CS_MODE_32,CS_MODE_64

p=argparse.ArgumentParser();p.add_argument('input',type=Path);p.add_argument('output',type=Path)
p.add_argument('--applications',action='store_true',help='Also inspect EXE/DLL without executing them')
a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
inputs=[a.input] if a.input.is_file() else a.input.iterdir()
for f in inputs:
 extensions={'.sys','.ax','.ds'} | ({'.exe','.dll'} if a.applications else set())
 if f.suffix.lower() not in extensions:continue
 pe=pefile.PE(str(f));base=pe.OPTIONAL_HEADER.ImageBase
 imports={i.address:f'{d.dll.decode()}!{i.name.decode() if i.name else i.ordinal}' for d in getattr(pe,'DIRECTORY_ENTRY_IMPORT',[]) for i in d.imports}
 cs=Cs(CS_ARCH_X86,CS_MODE_64 if pe.FILE_HEADER.Machine==0x8664 else CS_MODE_32)
 cs.skipdata=True
 lines=[]
 for s in pe.sections:
  if not s.Characteristics&0x20000000:continue
  for i in cs.disasm(s.get_data(),base+s.VirtualAddress):
   comment=''
   for addr,name in imports.items():
    if f'0x{addr:x}' in i.op_str:comment=f' ; {name}'
    if pe.FILE_HEADER.Machine==0x8664 and '[rip +' in i.op_str:
     m=re.search(r'\[rip \+ (0x[0-9a-f]+)\]',i.op_str)
     if m and i.address+i.size+int(m[1],16)==addr:comment=f' ; {name}'
   lines.append(f'{i.address:016x} {i.mnemonic:8} {i.op_str}{comment}')
 (a.output/(f.name+'.asm')).write_text('\n'.join(lines))
 (a.output/(f.name+'.imports.json')).write_text(json.dumps(imports,indent=2))
 raw=f.read_bytes()
 strings=re.findall(rb'[\x20-\x7e]{5,}',raw)
 (a.output/(f.name+'.strings.txt')).write_text('\n'.join(s.decode() for s in strings))
 print(f.name,len(lines),'instructions')
