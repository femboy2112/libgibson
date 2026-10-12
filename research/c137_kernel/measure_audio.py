#!/usr/bin/env python3
"""Measure PCM and calibration TSV independently of the HumanMusic runtime.

Only the optional spectral summary uses NumPy. All other work uses Python's
standard library. These are descriptive observations, never listening gates.
"""

import argparse
import array
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys
import wave


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def stats(values):
    return {
        "count": len(values),
        "min": min(values) if values else None,
        "max": max(values) if values else None,
        "mean": statistics.fmean(values) if values else None,
        "population_sd": statistics.pstdev(values) if values else None,
    }


def quantile(values, fraction):
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def interval_union(intervals):
    end = -math.inf
    duration = 0.0
    for start, stop in sorted(intervals):
        if stop > max(start, end):
            duration += stop - max(start, end)
        end = max(end, stop)
    return duration


def measure_tsv(path):
    roles = {}
    tempo = beats_per_bar = total_beats = None
    chord_count = 0
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        try:
            if fields[0] == "T":
                tempo, beats_per_bar, total_beats = map(float, fields[1:4])
            elif fields[0] == "N":
                role = fields[1]
                start, duration, pitch, velocity = map(float, fields[2:6])
                if duration < 0:
                    raise ValueError("negative written gate")
                roles.setdefault(role, []).append((start, duration, pitch, velocity))
            elif fields[0] == "D":
                start, velocity = map(float, fields[2:4])
                roles.setdefault("drums", []).append((start, None, None, velocity))
            elif fields[0] == "C":
                chord_count += 1
            else:
                raise ValueError(f"unknown row type {fields[0]!r}")
        except (ValueError, IndexError) as error:
            raise ValueError(f"{path}:{line_number}: {error}") from error
    if not all(value is not None and math.isfinite(value) and value > 0
               for value in (tempo, beats_per_bar, total_beats)):
        raise ValueError(f"{path}: missing or invalid T header")
    result = {"tempo_bpm": tempo, "beats_per_bar": beats_per_bar,
              "total_beats": total_beats, "chord_count": chord_count, "roles": {}}
    bar_count = math.ceil(total_beats / beats_per_bar)
    for role, notes in sorted(roles.items()):
        if any(not math.isfinite(value) for note in notes for value in note if value is not None):
            raise ValueError(f"{path}: non-finite score value in {role}")
        counts = [0] * bar_count
        velocity_sum = [0.0] * bar_count
        outside = 0
        for start, _, _, velocity in notes:
            bar = math.floor(start / beats_per_bar)
            if 0 <= bar < bar_count:
                counts[bar] += 1
                velocity_sum[bar] += velocity
            else:
                outside += 1
        summary = {
            "events": len(notes),
            "stored_velocity": stats([n[3] for n in notes]),
            "distinct_stored_velocities": len({n[3] for n in notes}),
            "attacks_per_bar": counts,
            "stored_velocity_sum_per_bar": velocity_sum,
            "active_bars": sum(value > 0 for value in counts),
            "attacks_outside_declared_span": outside,
        }
        if role != "drums":
            intervals = [(max(0.0, n[0]), min(total_beats, n[0] + n[1])) for n in notes]
            intervals = [(start, end) for start, end in intervals if end > start]
            ordered = sorted(notes)
            latest_end = -math.inf
            overlapped_attacks = 0
            for start, duration, _, _ in ordered:
                if start < latest_end - 1e-6:
                    overlapped_attacks += 1
                latest_end = max(latest_end, start + duration)
            summary.update({
                "pitch": stats([n[2] for n in notes]),
                "written_gate_beats": sum(n[1] for n in notes),
                "written_occupancy_fraction": interval_union(intervals) / total_beats,
                "attacks_overlapping_prior_written_gate": overlapped_attacks,
                "adjacent_same_pitch_retriggers_within_quarter_beat": sum(
                    0 < b[0] - a[0] <= 0.25 and a[2] == b[2]
                    for a, b in zip(ordered, ordered[1:])),
            })
        result["roles"][role] = summary
    result["limits"] = (
        "TSV stored velocity and written gates only; effective gain, voice choke/release, "
        "bus levels, source identity, hearings, and acoustic masking are not reconstructed. "
        "A polyphonic role can legitimately have overlapping written gates."
    )
    return result


def decode_pcm(raw, sample_width):
    if sample_width == 1:
        return [(value - 128) / 128.0 for value in raw]
    if sample_width in (2, 4):
        values = array.array("h" if sample_width == 2 else "i")
        values.frombytes(raw)
        if sys.byteorder != "little":
            values.byteswap()
        scale = float(1 << (8 * sample_width - 1))
        return [value / scale for value in values]
    if sample_width == 3:
        return [int.from_bytes(raw[i:i + 3], "little", signed=True) / 8388608.0
                for i in range(0, len(raw), 3)]
    raise ValueError(f"unsupported PCM sample width {sample_width}")


def measure_wav(path, spectrum=True):
    try:
        import numpy as np
    except ImportError:
        np = None
    with wave.open(str(path), "rb") as stream:
        channels, width, rate, frames = (stream.getnchannels(), stream.getsampwidth(),
                                        stream.getframerate(), stream.getnframes())
        if stream.getcomptype() != "NONE":
            raise ValueError(f"{path}: only uncompressed integer PCM is supported")
        peak = energy = 0.0
        sample_count = read_frames = nonfinite = 0
        windows = []
        window_frames = rate
        fft_energy = [0.0, 0.0, 0.0, 0.0]
        fft_windows = 0
        while raw := stream.readframes(window_frames):
            samples = decode_pcm(raw, width)
            if len(samples) % channels:
                raise ValueError(f"{path}: incomplete PCM frame")
            if any(not math.isfinite(sample) for sample in samples):
                nonfinite += sum(not math.isfinite(sample) for sample in samples)
                raise ValueError(f"{path}: non-finite decoded PCM")
            energy += sum(sample * sample for sample in samples)
            peak = max(peak, max(map(abs, samples), default=0.0))
            sample_count += len(samples)
            count = len(samples) // channels
            read_frames += count
            if count == window_frames:
                rms = math.sqrt(sum(sample * sample for sample in samples) / len(samples))
                windows.append(rms)
                if spectrum and np is not None:
                    # Per-channel power avoids cancellation from a stereo mono sum.
                    signal = np.asarray(samples).reshape(count, channels)
                    signal = signal - signal.mean(axis=0, keepdims=True)
                    transformed = np.fft.rfft(signal * np.hanning(count)[:, None], axis=0)
                    power = (np.abs(transformed) ** 2).sum(axis=1)
                    frequency = np.fft.rfftfreq(count, 1.0 / rate)
                    for index, (lo, hi) in enumerate(((20, 80), (80, 200), (200, 1000), (20, 20000))):
                        fft_energy[index] += float(power[(frequency >= lo) & (frequency < min(hi, rate / 2))].sum())
                    fft_windows += 1
        if read_frames != frames:
            raise ValueError(f"{path}: header declares {frames} frames; read {read_frames}")
    result = {
        "encoding": f"signed PCM{width * 8}" if width > 1 else "unsigned PCM8",
        "sample_rate": rate, "channels": channels, "frames": frames,
        "duration_seconds": frames / rate,
        "finite_decoded_pcm": nonfinite == 0,
        "peak": peak, "rms_all_channels": math.sqrt(energy / sample_count) if sample_count else 0.0,
        "complete_one_second_windows": len(windows),
        "window_rms": stats(windows),
        "window_rms_p10": quantile(windows, 0.1) if windows else None,
        "window_rms_p90": quantile(windows, 0.9) if windows else None,
    }
    if fft_windows:
        denominator = fft_energy[3]
        result["spectral_occupancy"] = {
            "method": "summed per-channel one-second Hann-window rFFT power; per-window DC removed",
            "numpy_version": np.__version__, "windows": fft_windows,
            "relative_to": "20 Hz to min(20000 Hz, Nyquist)",
            "fraction_20_80_hz": fft_energy[0] / denominator if denominator else None,
            "fraction_80_200_hz": fft_energy[1] / denominator if denominator else None,
            "fraction_200_1000_hz": fft_energy[2] / denominator if denominator else None,
        }
    else:
        result["spectral_occupancy"] = {"status": "not measured", "numpy_available": np is not None}
    result["limits"] = (
        "Written integer PCM only: finite decoded samples cannot establish that pre-quantization "
        "float render was finite. RMS is unweighted, not perceptual loudness. Spectral occupancy "
        "is not masking, role attribution, intelligibility, or musical acceptance."
    )
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path, help="WAV or fusion calibration TSV paths")
    parser.add_argument("--out", type=Path, help="JSON output; otherwise stdout")
    parser.add_argument("--no-spectrum", action="store_true", help="skip optional NumPy FFT")
    args = parser.parse_args()
    results = []
    for path in args.paths:
        suffix = path.suffix.lower()
        if suffix not in (".wav", ".tsv"):
            parser.error(f"unsupported input suffix: {path}")
        measured = measure_wav(path, not args.no_spectrum) if suffix == ".wav" else measure_tsv(path)
        results.append({"path": str(path.resolve()), "sha256": sha256(path),
                        "bytes": path.stat().st_size, "kind": suffix[1:], "measurement": measured})
    document = {"schema": "c137-independent-audio-measurements-v1", "artifacts": results}
    text = json.dumps(document, indent=2, allow_nan=False) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text)
    else:
        print(text, end="")


if __name__ == "__main__":
    main()
