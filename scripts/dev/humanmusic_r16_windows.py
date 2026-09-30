#!/usr/bin/env python3
"""Cut R16 listening windows from continuously rendered WAVs; hash every artifact.

No synthesis or envelope model lives here. Cropping the full render preserves all incoming
voice/reverb state. Optional numpy spectral fields are measurements of PCM, not a musical
quality score or a psychoacoustic roughness model.
"""
import argparse
import hashlib
import json
from pathlib import Path
import wave


def crop(source, target, start, end):
    with wave.open(str(source), 'rb') as reader:
        params = reader.getparams()
        lo = round(start * params.framerate)
        hi = min(round(end * params.framerate), params.nframes)
        reader.setpos(lo)
        data = reader.readframes(hi - lo)
    with wave.open(str(target), 'wb') as writer:
        writer.setparams(params)
        writer.writeframes(data)
    return {'source': source.name, 'window': target.name,
            'start_seconds': start, 'end_seconds': end,
            'start_frame': lo, 'end_frame': hi}


def spectral(path):
    import numpy as np
    with wave.open(str(path), 'rb') as reader:
        sr, channels = reader.getframerate(), reader.getnchannels()
        assert reader.getsampwidth() == 2
        pcm = np.frombuffer(reader.readframes(reader.getnframes()), dtype='<i2')
    samples = pcm.reshape(-1, channels).astype(np.float64).mean(axis=1) / 32768.0
    n = 8192
    frequency = np.fft.rfftfreq(n, 1.0 / sr)
    rows = []
    for offset in range(0, len(samples) - n + 1, n // 2):
        power = np.abs(np.fft.rfft(samples[offset:offset+n] * np.hanning(n))) ** 2
        total = float(power.sum())
        if total <= 1e-12:
            continue
        peaks = np.flatnonzero((power[1:-1] > power[:-2]) & (power[1:-1] >= power[2:])) + 1
        peaks = peaks[np.argsort(power[peaks])[-32:]]
        nearby = 0.0
        for i, a in enumerate(peaks):
            for b in peaks[i+1:]:
                if 20.0 <= abs(frequency[a]-frequency[b]) <= 80.0:
                    nearby += float(2 * np.sqrt(power[a] * power[b]) / total)
        rows.append((float((frequency * power).sum() / total),
                     float(power[frequency >= 2000].sum() / total), nearby))
    means = np.mean(rows, axis=0).tolist() if rows else [0.0, 0.0, 0.0]
    return dict(file=path.name, fft_frames=len(rows), rms=float(np.sqrt(np.mean(samples**2))),
                centroid_hz=means[0], power_fraction_above_2khz=means[1],
                nearby_peak_pair_crosspower_20_80hz=means[2])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--spectral', action='store_true')
    args = parser.parse_args()
    receipts = []
    for arm in ['r14', 'r15', 'r16']:
        for stem in ['full', 'pad', 'support']:
            source = args.directory / f'swiss_{arm}.{stem}.wav'
            if not source.exists():
                continue
            # Two-beat lead-in, full bad chord, and two-beat departure; identical source sample
            # boundaries across arms. The 60-beat control has the same key-voicing family.
            for beat in [28, 36, 44, 60]:
                target = args.directory / f'swiss_{arm}.b{beat:03d}.{stem}.wav'
                receipts.append(crop(source, target, (beat-2)*60/118, (beat+6)*60/118))
    (args.directory / 'windows.json').write_text(json.dumps(receipts, indent=2)+'\n')
    if args.spectral:
        rows = [spectral(args.directory / r['window']) for r in receipts]
        (args.directory / 'spectral.json').write_text(json.dumps({
            'instrument': 'PCM i16; mono mean; Hann 8192; hop4096; 32 largest local spectral peaks',
            'boundary': 'Deterministic spectral exposure/interference proxy; not perceptual roughness or musical quality. Shared synthesis lineage; no independent audition.',
            'rows': rows}, indent=2)+'\n')
    sums = []
    for path in sorted(args.directory.glob('*.wav')):
        sums.append(hashlib.sha256(path.read_bytes()).hexdigest()+'  '+path.name)
    (args.directory / 'SHA256SUMS').write_text('\n'.join(sums)+'\n')
    print(f'{len(receipts)} windows; {len(sums)} WAV hashes')


if __name__ == '__main__':
    main()
