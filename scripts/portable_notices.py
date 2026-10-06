"""Preserve installed runtime license files in each portable distribution."""
from pathlib import Path
from importlib import metadata
import json
import shutil
import sys
import sysconfig


def collect_notices(root: Path, destination: Path):
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copytree(root/'portable/licenses', destination, dirs_exist_ok=True)
    shutil.copy2(root/'LICENSE', destination/'LICENSE')
    versions={}
    for name in ['numpy','Pillow','opencv-python','PySide6','PySide6_Essentials',
                 'PySide6_Addons','shiboken6','pyusb','libusb-package','PyInstaller']:
        distribution=metadata.distribution(name)
        versions[name]=distribution.version
        for entry in distribution.files or []:
            if any(word in entry.name.lower() for word in ['license','copying','copyright','notice']):
                source=Path(distribution.locate_file(entry))
                if source.is_file() and '..' not in entry.parts:
                    target=destination/name/Path(entry)
                    target.parent.mkdir(parents=True,exist_ok=True)
                    shutil.copy2(source,target)
    python_license=Path(sysconfig.get_path('stdlib'))/'LICENSE.txt'
    if not python_license.is_file():
        python_license=Path(sys.base_prefix)/'LICENSE.txt'
    if not python_license.is_file():
        raise RuntimeError('Python runtime license is missing')
    shutil.copy2(python_license,destination/'Python-LICENSE.txt')
    versions['Python']=sys.version.split()[0]
    (destination/'versions.json').write_text(json.dumps(versions,indent=2)+'\n')
