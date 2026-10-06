"""Determine USB Bayer parity using documented per-color sensor gains."""
import argparse,json
from pathlib import Path
import numpy as np
from kohdalab_camera.legacy_usb import LegacyUsbCamera
from kohdalab_camera.storage import save_frame

p=argparse.ArgumentParser();p.add_argument('--execute',action='store_true')
p.add_argument('--output',type=Path,default=Path('captures/bayer-phase'))
a=p.parse_args()
if not a.execute:raise SystemExit('Add --execute for the connected camera.')
a.output.mkdir(parents=True,exist_ok=True)
class ColorGainProbe(LegacyUsbCamera):
    def write_register(self,address,value):
        super().write_register(address,value)
        if address==0x35:
            # Known MT9T001 color registers: blue=4x, red=2x, greens=1x.
            super().write_register(0x2c,32)
            super().write_register(0x2d,16)

planes=[]
for cls in (LegacyUsbCamera,ColorGainProbe):
    camera=cls();camera.configure_controls({'exposure':128,'gain':8})
    try:
        camera.open();frames=[camera.read() for _ in range(12)]
        raw=np.median([f.sensor_raw for f in frames[4:]],axis=0)
        planes.append(np.array([[np.median(raw[i::2,j::2]) for j in range(2)] for i in range(2)]))
        save_frame(frames[-1],a.output)
    finally:camera.close()
ratios=(planes[1]+1)/(planes[0]+1)
flat=ratios.ravel();order=np.argsort(flat)
if flat[order[-1]]<2 or flat[order[-2]]<1.3 or max(flat[order[:2]])>1.3:
    raise SystemExit(f'Inconclusive color gain response: {ratios}')
pattern=['G']*4;pattern[order[-1]]='B';pattern[order[-2]]='R';pattern=''.join(pattern)
if pattern not in ('GRBG','GBRG','RGGB','BGGR'):raise SystemExit('Invalid Bayer parity')
result={'baseline_medians':planes[0].tolist(),'color_gain_medians':planes[1].tolist(),
        'ratios':ratios.tolist(),'bayer_pattern':pattern,'method':'blue register 2C=32; red 2D=16; greens=8'}
(a.output/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
