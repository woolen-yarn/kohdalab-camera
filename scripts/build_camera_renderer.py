"""Compile the pixel-exact renderer for this Mac or Windows x64."""
from pathlib import Path
import argparse,shutil,subprocess,sys

ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--windows',action='store_true',help='Cross-compile Windows x64 with MinGW');args=p.parse_args()
out=ROOT/'build/camera-render';out.mkdir(parents=True,exist_ok=True)
source=ROOT/'native/camera_render.c'
windows=args.windows or sys.platform=='win32'
if windows:
 compiler=shutil.which('x86_64-w64-mingw32-gcc') or (shutil.which('gcc') if sys.platform=='win32' else None)
 if not compiler:raise SystemExit('Install an x64 MinGW compiler to build camera-render.dll.')
 destination=out/'camera-render.dll'
 command=[compiler,'-O3','-shared','-static','-s',str(source),'-o',str(destination)]
elif sys.platform=='darwin':
 compiler=shutil.which('clang')
 if not compiler:raise SystemExit('The macOS command-line compiler is required.')
 destination=out/'libcamera_render.dylib'
 command=[compiler,'-O3','-fPIC','-dynamiclib',str(source),'-o',str(destination)]
else:raise SystemExit('Supported native renderer targets: macOS and Windows x64.')
subprocess.run(command,check=True);print(destination)
