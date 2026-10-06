import numpy as np
import pytest
from kohdalab_camera.color import demosaic

@pytest.mark.parametrize('pattern',['GRBG','GBRG','RGGB','BGGR'])
def test_sensor_color_sites_map_to_rgb_without_mutating_raw(pattern):
    raw=np.zeros((8,10),np.uint8)
    values={'R':180,'G':90,'B':30}
    for index,site in enumerate(pattern):raw[index//2::2,index%2::2]=values[site]
    original=raw.copy();rgb=demosaic(raw,pattern)
    assert np.all(rgb==[180,90,30])
    assert np.array_equal(raw,original)


def test_brightness_and_orientation_preserve_original_sensor():
    from kohdalab_camera.color import render_sensor
    raw=np.arange(24,dtype=np.uint8).reshape(4,6);original=raw.copy()
    rgb=render_sensor(raw,'gray',gamma=1,brightness_ev=1,mirror=True,flip=True,rotation=90)
    expected=np.rot90((raw*2)[::-1,::-1],-1)
    assert np.array_equal(rgb[:,:,0],expected)
    assert rgb.flags.c_contiguous and np.array_equal(raw,original)

@pytest.mark.parametrize('factor,code',[(1,8),(2,16),(4,32),(5,84),(8,96)])
def test_analog_gain_encoding(factor,code):
    from kohdalab_camera.color import gain_to_register,register_to_gain
    assert gain_to_register(factor)==code
    assert register_to_gain(code)==factor

@pytest.mark.parametrize('pattern,digest',[
    ('GRBG','622fb1a09fc0df32e135dd57249f07172ec579722c5b0fa0ee46899925042b96'),
    ('GBRG','d20a1b12c0f7155694eb44b9ea955e5d525a8ab99ea0c3f7fd91640f49ef4d04'),
    ('RGGB','cbe1f0211d469bffb285b9e40aed57413d2554b3db5c240739067ddea091178e'),
    ('BGGR','40bdf6f148324fd9d4e937d500a4e5c0bfc94998a5f2490d519ac31c52ca7a55'),
])
def test_optimized_render_matches_preoptimization_pixels(pattern,digest):
    # Digests generated from the prior float32 implementation, before optimizing.
    import hashlib
    from kohdalab_camera.color import render_sensor
    raw=np.random.default_rng(42).integers(0,256,(65,67),dtype=np.uint8)
    original=raw.copy()
    rgb=render_sensor(raw,pattern=pattern,white_balance=np.array([1.351,.701,2.33],np.float32),
                      brightness_ev=1.5,gamma=2.2,mirror=True,flip=True,rotation=90)
    assert hashlib.sha256(rgb.tobytes()).hexdigest()==digest
    assert rgb.flags.c_contiguous and np.array_equal(raw,original)
