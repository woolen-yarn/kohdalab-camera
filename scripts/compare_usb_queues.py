"""Compare USB queues in alternating order without changing application defaults."""
import argparse
import json
import time
from pathlib import Path

from kohdalab_camera.color import gain_to_register
from kohdalab_camera.legacy_usb import LegacyUsbCamera
from kohdalab_camera.usb_stream import QueuedBulkReader


def measure(seconds, exposure, gain, size, depth):
    QueuedBulkReader.transfer_bytes = size
    QueuedBulkReader.depth = depth
    camera = LegacyUsbCamera()
    camera.configure_controls({'exposure': exposure, 'gain': gain})
    result = {'exposure_rows': exposure, 'gain_register': gain,
              'transfer_bytes': size, 'queue_depth': depth,
              'error': None, 'profile_retained': True}
    count = 0
    start = None
    previous = None
    longest_gap = 0
    events = []
    try:
        camera.open()
        result['initial_sensor_state'] = camera.capture_transport_state()
        start = previous = time.monotonic()
        last_recoveries = 0
        while time.monotonic() - start < seconds:
            frame = camera.read()
            current = time.monotonic()
            longest_gap = max(longest_gap, current - previous)
            previous = current
            count += 1
            if camera.recoveries != last_recoveries:
                events.append({'at_seconds': current - start,
                               'pipe_recoveries': camera.pipe_recoveries,
                               'bus_resets': camera.bus_resets})
                last_recoveries = camera.recoveries
            if (frame.controls['exposure'].get('readback') != exposure
                    or frame.controls['gain'].get('readback') != gain):
                result['profile_retained'] = False
                raise RuntimeError('Requested sensor settings were not retained')
        result['final_sensor_state'] = camera.capture_transport_state()
        result['last_raw_mean'] = float(frame.sensor_raw.mean())
    except Exception as exc:
        result['error'] = str(exc)
    finally:
        elapsed = time.monotonic() - start if start is not None else 0
        result.update(seconds=elapsed, frames=count, fps=count / elapsed if elapsed else 0,
                      longest_frame_gap_seconds=longest_gap, recovery_events=events,
                      pipe_recoveries=camera.pipe_recoveries, bus_resets=camera.bus_resets)
        try:
            camera.close()
        except Exception as exc:
            result['cleanup_error'] = str(exc)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--seconds', type=float, default=40)
    parser.add_argument('--rounds', type=int, default=2)
    parser.add_argument('--exposure', type=int, default=300)
    parser.add_argument('--gain', type=float, default=2)
    parser.add_argument('--candidate-bytes', type=int, default=524288)
    parser.add_argument('--candidate-depth', type=int, default=8)
    parser.add_argument('--output', type=Path, default=Path('captures/queue-comparison'))
    args = parser.parse_args()
    if not args.execute:
        parser.error('Use --execute to access the connected camera; close the camera app first')
    if not 0 < args.seconds <= 600 or not 1 <= args.rounds <= 4:
        parser.error('Use 0..600 seconds and 1..4 rounds')
    if args.candidate_bytes not in (16384, 32768, 65536, 131072, 524288):
        parser.error('Unsupported diagnostic transfer size')
    if not 2 <= args.candidate_depth <= 64:
        parser.error('Queue depth must be 2..64')
    gain = gain_to_register(args.gain)
    args.output.mkdir(parents=True, exist_ok=True)
    original = QueuedBulkReader.transfer_bytes, QueuedBulkReader.depth
    results = []
    try:
        for _ in range(args.rounds):
            for size, depth in (original, (args.candidate_bytes, args.candidate_depth)):
                result = measure(args.seconds, args.exposure, gain, size, depth)
                results.append(result)
                (args.output / 'results.json').write_text(json.dumps(results, indent=2))
                print(json.dumps(result), flush=True)
                if 'cleanup_error' in result:
                    raise RuntimeError('Transfer cleanup failed; stop comparison before reopening')
    finally:
        QueuedBulkReader.transfer_bytes, QueuedBulkReader.depth = original
    if any(r['error'] or r['bus_resets'] or not r['profile_retained'] for r in results):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
