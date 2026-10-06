"""Build the shared-runtime x64 portable on Windows; Python/uv aren't needed by users."""
from pathlib import Path
import subprocess, sys, shutil, hashlib, json
from zipfile import ZipFile, ZIP_DEFLATED

from portable_notices import collect_notices

ROOT=Path(__file__).resolve().parents[1]
if sys.platform != 'win32':
    raise SystemExit('Build this portable on Windows x64.')
work=ROOT/'build/windows-portable'
work.mkdir(parents=True,exist_ok=True)
renderer=ROOT/'build/camera-render/camera-render.dll'
if not renderer.is_file():raise RuntimeError('Build the native renderer first: scripts/build_camera_renderer.py')
spec=work/'camera.spec'
spec.write_text(f'''from PyInstaller.utils.hooks import collect_all
usb_datas, usb_binaries, usb_imports = collect_all('libusb_package')
a = Analysis([{str(ROOT/'scripts/windows_entry.py')!r}], pathex=[{str(ROOT/'src')!r}],
    binaries=usb_binaries+[({str(renderer)!r},'kohdalab_camera/_native')], datas=usb_datas,
    hiddenimports=usb_imports+['usb.backend.libusb1','kohdalab_camera.gui','kohdalab_camera.usb_worker','kohdalab_camera.cli','kohdalab_camera.windows_validation'],
    hookspath=[], hooksconfig={{}}, runtime_hooks=[], excludes=[], noarchive=False)
pyz = PYZ(a.pure)
gui = EXE(pyz,a.scripts,[],exclude_binaries=True,name='KohdaLab Camera',console=False)
worker = EXE(pyz,a.scripts,[],exclude_binaries=True,name='camera-worker',console=True)
cli = EXE(pyz,a.scripts,[],exclude_binaries=True,name='camera-cli',console=True)
coll = COLLECT(gui,worker,cli,a.binaries,a.datas,strip=False,upx=False,name='KohdaLab Camera')
''',encoding='utf-8')
if '--package-only' not in sys.argv:
    subprocess.run([sys.executable,'-m','PyInstaller','--noconfirm','--clean','--distpath',str(ROOT/'dist'),'--workpath',str(work/'pyinstaller'),str(spec)],check=True)
folder=ROOT/'dist/KohdaLab Camera'
(folder/'support').mkdir(exist_ok=True)
for name in ['Camera-USB-Setup.exe', 'libwdi.dll']:
    candidates = [ROOT/'build/usb-setup'/name, ROOT/'support'/name]
    source = next((p for p in candidates if p.is_file()), None)
    if source is None:
        raise RuntimeError(f'Build USB setup first: missing {name}')
    shutil.copy2(source, folder/'support'/name)
subprocess.run([str(folder/'support/Camera-USB-Setup.exe'), '--self-test'], check=True)
shutil.copytree(ROOT/'docs',folder/'support/docs',dirs_exist_ok=True)
licenses=folder/'support/licenses';licenses.mkdir(parents=True,exist_ok=True)
collect_notices(ROOT,licenses)
with ZipFile(folder/'support/SOURCE.zip','w',ZIP_DEFLATED) as z:
    for name in ['src','scripts','tests','native','docs','portable','.github','README.md','LICENSE','CHANGELOG.md','CONTRIBUTING.md','ROADMAP.md','pyproject.toml','uv.lock']:
        p=ROOT/name
        for f in ([p] if p.is_file() else p.rglob('*')):
            if f.is_file() and '__pycache__' not in f.parts and not f.name.endswith('.pyc'):
                z.write(f,f.relative_to(ROOT))
(folder/'README.txt').write_text('Extract the whole folder, then open KohdaLab Camera.exe.\nConnect your 0547:4D33 camera and click Connect.\nWindows may request administrator/driver approval for initial WinUSB setup.\nSee support/docs/windows-portable.md for driver restore and validation.\n',encoding='utf-8')
subprocess.run([str(folder/'KohdaLab Camera.exe'),'--smoke-test'],check=True)
assert json.loads((folder/'portable-smoke.json').read_text())['ok']
archive=ROOT/'dist/kohdalab-camera-windows-x64-portable.zip'
with ZipFile(archive,'w',ZIP_DEFLATED) as z:
    for f in folder.rglob('*'):
        if f.is_file() and 'validation' not in f.relative_to(folder).parts:
            z.write(f,Path('KohdaLab Camera')/f.relative_to(folder))
(ROOT/'dist/SHA256SUMS.txt').write_text(hashlib.sha256(archive.read_bytes()).hexdigest()+'  '+archive.name+'\n')
print(archive)
