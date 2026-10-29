"""Convert the project camera artwork to PNG and multi-resolution Windows ICO.

Requires Pillow. The source artwork is retained for future icon updates.
"""
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
assets = root / 'crates' / 'camera-lab' / 'assets'
with Image.open(assets / 'kohdalab-camera-artwork.png') as artwork:
    icon = artwork.convert('RGBA').resize((256, 256), Image.Resampling.LANCZOS)
    icon.save(assets / 'kohdalab-camera.png')
    icon.save(assets / 'camera.ico', sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
