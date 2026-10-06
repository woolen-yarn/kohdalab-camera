import sys
from kohdalab_camera.mac_activity import CaptureActivity

def test_capture_activity_ends_and_releases_exactly_once():
    activity=CaptureActivity()
    assert bool(activity.token)==(sys.platform=='darwin')
    activity.close()
    assert activity.token is None
    activity.close()

def test_capture_activity_does_not_disable_idle_or_display_sleep():
    assert not CaptureActivity.options & (1<<20)
    assert not CaptureActivity.options & (1<<40)
