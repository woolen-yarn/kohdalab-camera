"""Build the native-architecture Mac app with its independent USB worker."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
import hashlib,json,os,platform,shutil,subprocess,sys

from portable_notices import collect_notices

ROOT=Path(__file__).resolve().parents[1]
if sys.platform!='darwin' or platform.machine()!='arm64':
 raise SystemExit('Build this release on Apple Silicon macOS (arm64).')
work=ROOT/'build/macos-portable';work.mkdir(parents=True,exist_ok=True)
renderer=ROOT/'build/camera-render/libcamera_render.dylib'
if not renderer.is_file():raise RuntimeError('Build the native renderer first: scripts/build_camera_renderer.py')
spec=work/'camera.spec'
spec.write_text(f'''from PyInstaller.utils.hooks import collect_all
usb_datas,usb_binaries,usb_imports=collect_all('libusb_package')
a=Analysis([{str(ROOT/'scripts/windows_entry.py')!r}],pathex=[{str(ROOT/'src')!r}],
 binaries=usb_binaries+[({str(renderer)!r},'kohdalab_camera/_native')],datas=usb_datas,hiddenimports=usb_imports+['usb.backend.libusb1','kohdalab_camera.gui','kohdalab_camera.usb_worker','kohdalab_camera.cli','kohdalab_camera.windows_validation'],
 hookspath=[],hooksconfig={{}},runtime_hooks=[],excludes=[],noarchive=False)
pyz=PYZ(a.pure)
gui=EXE(pyz,a.scripts,[],exclude_binaries=True,name='KohdaLab Camera',console=False)
worker=EXE(pyz,a.scripts,[],exclude_binaries=True,name='camera-worker',console=True)
cli=EXE(pyz,a.scripts,[],exclude_binaries=True,name='camera-cli',console=True)
coll=COLLECT(gui,worker,cli,a.binaries,a.datas,strip=False,upx=False,name='KohdaLab Camera-mac')
app=BUNDLE(coll,name='KohdaLab Camera.app',bundle_identifier='jp.kohdalab.camera',
 info_plist={{'NSHighResolutionCapable':True,'NSCameraUsageDescription':'Capture images from a connected laboratory camera.'}})
''',encoding='utf-8')
if '--package-only' not in sys.argv:
 subprocess.run([sys.executable,'-m','PyInstaller','--noconfirm','--clean','--distpath',str(ROOT/'dist'),'--workpath',str(work/'pyinstaller'),str(spec)],check=True)
app=ROOT/'dist/KohdaLab Camera.app';support=app/'Contents/Resources/support';support.mkdir(parents=True,exist_ok=True)
shutil.copytree(ROOT/'docs',support/'docs',dirs_exist_ok=True)
licenses=support/'licenses';licenses.mkdir(parents=True,exist_ok=True)
collect_notices(ROOT,licenses)
with ZipFile(support/'SOURCE.zip','w',ZIP_DEFLATED) as archive:
 for name in ['src','scripts','tests','native','docs','portable','.github','README.md','LICENSE','CHANGELOG.md','CONTRIBUTING.md','ROADMAP.md','pyproject.toml','uv.lock']:
  path=ROOT/name
  for f in ([path] if path.is_file() else path.rglob('*')):
   if f.is_file() and '__pycache__' not in f.parts and not f.name.endswith('.pyc'):archive.write(f,f.relative_to(ROOT))
subprocess.run(['codesign','--force','--deep','--sign','-',str(app)],check=True)
subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
exe=app/'Contents/MacOS/KohdaLab Camera'
env={**os.environ,'QT_QPA_PLATFORM':'offscreen'}
subprocess.run([str(exe),'--smoke-test'],env=env,check=True)
smoke=exe.with_name('portable-smoke.json');assert json.loads(smoke.read_text())['ok']
shutil.copy2(smoke,ROOT/'dist/macos-portable-smoke.json');smoke.unlink()
# Keep the signed application unchanged after the smoke test artifact is removed.
subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
archive=ROOT/f'dist/kohdalab-camera-macos-{platform.machine()}-portable.zip'
subprocess.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(app),str(archive)],check=True)
(ROOT/'dist/SHA256SUMS-macos.txt').write_text(hashlib.sha256(archive.read_bytes()).hexdigest()+'  '+archive.name+'\n')
print(archive)
