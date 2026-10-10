#!/usr/bin/env python3
"""Create a blind PCM16 listening packet; keep the key outside the public zip.

Input is the tab-separated manifest emitted by `humanmusic_argument`. Only Python's
standard library is needed. Normalization is an RMS comparison control, not LUFS
or a psychoacoustic loudness guarantee. The original WAVs are never modified.
"""

import argparse
import array
import csv
import hashlib
import json
import math
from pathlib import Path
import random
import sys
import wave
import zipfile


def pcm(path):
    with wave.open(str(path), "rb") as reader:
        params = reader.getparams()
        if params.sampwidth != 2 or params.comptype != "NONE":
            raise ValueError(f"expected PCM16: {path}")
        samples = array.array("h", reader.readframes(params.nframes))
    if sys.byteorder != "little":
        samples.byteswap()
    if not samples:
        raise ValueError(f"empty audio: {path}")
    rms = math.sqrt(sum(float(x) ** 2 for x in samples) / len(samples)) / 32767
    peak = max(abs(x) for x in samples) / 32767
    if rms == 0:
        raise ValueError(f"silent audio cannot be loudness matched: {path}")
    return params, samples, rms, peak


def make_packet(manifest, destination, seed, target_rms):
    with manifest.open(newline="") as handle:
        rows = list(csv.DictReader(handle, delimiter="\t"))
    if not rows or "path" not in rows[0]:
        raise ValueError("manifest must contain a path column and at least one row")
    # Stem labels are a substantial cue. Only full mixes enter the blind default.
    rows = [r for r in rows if r.get("stem", "mix") == "mix"]
    if not rows:
        raise ValueError("no full mixes in manifest")
    random.Random(seed).shuffle(rows)
    public = destination / "listener"
    private = destination / "key"
    public.mkdir(parents=True, exist_ok=True)
    private.mkdir(parents=True, exist_ok=True)
    key = []
    comparison_frames = {}
    comparisons = sorted({row.get("comparison", "") for row in rows} - {""})
    random.Random(seed ^ 0xF041).shuffle(comparisons)
    opaque_groups = {name: f"group_{index:03}" for index, name in enumerate(comparisons, 1)}
    effective_target = target_rms
    # One common target prevents an individually peak-limited clip from receiving
    # a level disadvantage. The loudest crest factor determines the common ceiling.
    for row in rows:
        source = Path(row["path"])
        if not source.is_absolute():
            source = manifest.parent / source
        _, _, rms, peak = pcm(source)
        effective_target = min(effective_target, 0.98 * rms / peak)
    for index, row in enumerate(rows, 1):
        source = Path(row["path"])
        if not source.is_absolute():
            source = manifest.parent / source
        params, samples, rms, peak = pcm(source)
        comparison = row.get("comparison", "")
        timing = (params.framerate, params.nchannels, params.nframes, row.get("tempo", ""))
        if comparison:
            if comparison in comparison_frames and comparison_frames[comparison] != timing:
                raise ValueError(f"tempo/duration/rate mismatch in comparison {comparison}")
            comparison_frames[comparison] = timing
        gain = effective_target / rms
        normalized = array.array("h", (round(x * gain) for x in samples))
        if sys.byteorder != "little":
            normalized.byteswap()
        filename = f"clip_{index:03}.wav"
        output = public / filename
        with wave.open(str(output), "wb") as writer:
            writer.setparams(params)
            writer.writeframes(normalized.tobytes())
        key.append({
            "clip": filename,
            "blind_group": opaque_groups.get(comparison, ""),
            **row,
            "input_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "output_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
            "input_rms": rms,
            "input_peak": peak,
            "gain": gain,
            "output_rms": rms * gain,
            "output_peak": peak * gain,
            "rms_target_reached": math.isclose(rms * gain, effective_target, abs_tol=2e-5),
        })
    instructions = """# HumanMusic relationship listening\n\nListen without opening the answer key. Filenames do not identify the condition.\nRMS was matched with peak limiting; this controls a level advantage, not perceived\nloudness in every timbre. The exact gain and hashes are in the separate key.\n\nFor each clip, listen for relationships, not literal words:\n\n1. Does a recognizable idea ask and receive a specifically related answer?\n2. Is a promised arrival denied, then earned by a transformed return?\n3. Do two distinct ideas become compatible through a recognizable relationship?\n4. Is the evidence instead incoherent, missing, or ambiguous?\n\nRecord the relationship you heard, the moments that support it, and confidence\n(low/medium/high). Separately record whether it is musically compelling, then give\nACCEPT, REJECT, or AMBIGUOUS. These are separate judgments. There is no inferred\nlistener verdict in this packet. Silence, absent carriers, and source scrambling\nare controls; do not presume every clip contains a successful argument.\n"""
    (public / "README.md").write_text(instructions)
    with (public / "responses.tsv").open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["clip", "heard_relationship", "supporting_moments", "confidence", "compelling", "verdict"])
        for item in key:
            writer.writerow([item["clip"], "", "", "", "", ""])
    with (public / "groups.tsv").open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["group", "clip"])
        for item in sorted(key, key=lambda item: (item["blind_group"], item["clip"])):
            if item["blind_group"]:
                writer.writerow([item["blind_group"], item["clip"]])
    (private / "answer_key.json").write_text(json.dumps({
        "randomization_seed": seed,
        "requested_target_rms": target_rms,
        "common_target_rms": effective_target,
        "normalization": "one common PCM16 RMS target reduced globally to cap every peak at 0.98; no silence padding or tempo manipulation",
        "manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
        "clips": key,
    }, indent=2) + "\n")
    archive = destination / "blind_listening.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as writer:
        for path in sorted(public.iterdir()):
            writer.write(path, path.name)
    print(json.dumps({"packet": str(archive), "key": str(private / "answer_key.json"), "clips": len(key)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--seed", type=int, default=20261009)
    parser.add_argument("--target-rms", type=float, default=0.08)
    args = parser.parse_args()
    if not 0 < args.target_rms < 0.5:
        parser.error("target RMS must be in (0, 0.5)")
    make_packet(args.manifest, args.destination, args.seed, args.target_rms)


if __name__ == "__main__":
    main()
