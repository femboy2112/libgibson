#!/usr/bin/env python3
"""Hash Rust knockout WAVs, verify R16 baselines, and index local audition assets."""
import argparse
import hashlib
import html
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    root = args.directory
    repo = Path(__file__).resolve().parents[2]
    frozen = {}
    for line in (repo / "docs/fixtures/humanmusic-r16/final/SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        frozen[name] = digest
    paths = sorted(root.glob("*.wav"))
    digests = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
    (root / "SHA256SUMS").write_text("".join(f"{digest}  {name}\n" for name, digest in digests.items()))
    checks = []
    page = ["<!doctype html><meta charset='utf-8'><title>SWISS knockout audition</title>",
            "<h1>SWISS knockout audition</h1>",
            "<p>Frozen R16 mix; each knockout mutes one note. Compare the baseline, G5 and C4 first, "
            "then individual lead and keys notes. Which removes the horn blast? "
            "Multiple helpful removals leave interaction, masking or release-tail explanations unresolved. "
            "No assistant listening verdict.</p>"]
    playlist = ["#EXTM3U"]
    for site, g5, c4 in [(28, 31, 28), (36, 39, 36), (44, 47, 44), (60, 63, 60)]:
        prefix = f"swiss_r16_b{site:03}"
        baseline = f"{prefix}_baseline.wav"
        old = f"swiss_r16.b{site:03}.full.wav"
        assert digests[baseline] == frozen[old], f"frozen R16 window changed: {baseline}"
        checks.append(f"Verified {baseline} == frozen {old}: {digests[baseline]}")
        top = 76 if site == 60 else 79
        targeted = [baseline, f"{prefix}_mute_pad_{g5:04}_m{top}.wav", f"{prefix}_mute_pad_{c4:04}_m60.wav"]
        page.append(f"<h2>b{site:03}{' (accepted control)' if site == 60 else ''}</h2>")
        remaining = [p.name for p in paths if p.name.startswith(prefix) and p.name not in targeted]
        for name in targeted + remaining:
            assert name in digests
            playlist.extend([f"#EXTINF:-1,{name}", name])
            label = html.escape(name)
            page.append(f"<p>{label}<br><audio controls preload='none' src='{label}'></audio></p>")
    (root / "control_checks.txt").write_text("\n".join(checks) + "\n")
    (root / "audition.html").write_text("\n".join(page) + "\n")
    (root / "audition.m3u").write_text("\n".join(playlist) + "\n")
    print(f"{len(paths)} WAV hashes; four frozen baseline windows exact; audition.html and audition.m3u")


if __name__ == "__main__":
    main()
