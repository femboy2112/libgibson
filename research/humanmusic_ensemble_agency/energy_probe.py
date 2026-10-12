#!/usr/bin/env python3
"""C137 multi-role energy diagnosis on fusion_gates --dump-tsv data.

This is a Score event diagnostic, NOT PCM loudness or causal proof. The current
TSV records T/N/D/C, but does not export all source/provenance/action edges.
The right causal experiment mutates one player's *plan* then recomposes, not
just correlations from one recording.
"""
from __future__ import annotations
import argparse
import json
import math
import statistics
import unittest
from dataclasses import dataclass
from pathlib import Path

ROLES = ('lead', 'keys', 'bass', 'pad', 'drums')


@dataclass(frozen=True)
class Event:
    role: str
    beat: float
    dur: float
    vel: float


def parse_tsv(text: str) -> tuple[float, float, list[Event]]:
    total = None
    tempo = None
    events: list[Event] = []
    for lineno, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.startswith('#'):
            continue
        fields = line.split('\t')
        try:
            if fields[0] == 'T':
                if len(fields) < 4 or total is not None:
                    raise ValueError('duplicate/invalid T')
                tempo, bpb, total = map(float, fields[1:4])
                if not (math.isfinite(total) and 0 < total <= 100000 and
                        math.isfinite(tempo) and 1 <= tempo <= 1000 and bpb > 0):
                    raise ValueError('invalid timeline')
            elif fields[0] == 'N':
                if len(fields) < 6:
                    raise ValueError('invalid note fields')
                role, beat, dur, vel = fields[1], float(fields[2]), float(fields[3]), float(fields[5])
                if role not in ROLES[:4]:
                    raise ValueError('invalid note role')
                events.append(Event(role, beat, dur, vel))
            elif fields[0] == 'D':
                if len(fields) < 4:
                    raise ValueError('invalid drum fields')
                events.append(Event('drums', float(fields[2]), 0.0, float(fields[3])))
            elif fields[0] == 'C':
                pass
            else:
                raise ValueError('unrecognized row kind')
        except (ValueError, TypeError) as err:
            raise ValueError(f'line {lineno}: {err}') from err
    if total is None or tempo is None:
        raise ValueError('missing timeline T')
    for e in events:
        if not (math.isfinite(e.beat) and 0 <= e.beat < total and
                math.isfinite(e.vel) and 0 <= e.vel <= 1 and
                math.isfinite(e.dur) and e.dur >= 0):
            raise ValueError(f'invalid event {e}')
    return tempo, total, events


def make_lanes(total: float, events: list[Event], binsize: float):
    if not (math.isfinite(binsize) and 0.0625 <= binsize <= 8):
        raise ValueError('binsize not finite/in range')
    size = math.ceil(total / binsize)
    if size > 1000000:
        raise ValueError('too many bins')
    onsets = {r: [0.] * size for r in ROLES}
    coverage = {r: [0.] * size for r in ROLES}
    count = {r: 0 for r in ROLES}
    for e in events:
        idx = min(int(e.beat / binsize), size-1)
        onsets[e.role][idx] += e.vel
        count[e.role] += 1
        if e.role == 'drums':
            continue
        last = min(total, e.beat + e.dur)
        j = idx
        while j < size and j*binsize < last:
            overlap = max(0., min(last, (j+1)*binsize) - max(e.beat, j*binsize))
            coverage[e.role][j] += e.vel * overlap / binsize
            j += 1
    return onsets, coverage, count


def pearson(x: list[float], y: list[float]):
    if len(x) != len(y) or len(x) < 3:
        return None
    mx, my = statistics.fmean(x), statistics.fmean(y)
    dx, dy = [v-mx for v in x], [v-my for v in y]
    xx = sum(v*v for v in dx)
    yy = sum(v*v for v in dy)
    return sum(a*b for a,b in zip(dx,dy))/math.sqrt(xx*yy) if xx>1e-12 and yy>1e-12 else None


def lagged_corr(x: list[float], y: list[float], lag: int):
    """Positive lag means y's samples lag x's by lag bins."""
    if lag >= 0:
        return pearson(x[:len(x)-lag] if lag else x, y[lag:])
    return pearson(x[-lag:], y[:lag])


def moving_avg(x: list[float], radius: int):
    result = []
    prefix = [0.] * (len(x)+1)
    for i,v in enumerate(x):prefix[i+1] = prefix[i]+v
    for i in range(len(x)):
        lo,hi=max(0,i-radius), min(len(x),i+radius+1)
        result.append((prefix[hi]-prefix[lo])/(hi-lo))
    return result


def residual(x: list[float], window_bins: int):
    # Remove a broad form envelope but retain bar-to-bar and immediate responses.
    avg = moving_avg(x, max(1,window_bins//2))
    return [a-b for a,b in zip(x,avg)]


def fall_response(lead: list[float], keys: list[float], future_bins: int=2):
    """Bounded 'keys enters after lead's attack drops' observation, not cause."""
    opportunities=0
    responses=0
    for i in range(1,len(lead)):
        if lead[i-1] > 1e-7 and lead[i] <= 1e-7:
            opportunities+=1
            if any(v > 1e-7 for v in keys[i:min(len(keys),i+future_bins)]):
                responses+=1
    return dict(lead_attack_drop_windows=opportunities, keys_onset_followups=responses,
                fraction=(responses/opportunities if opportunities else None),
                unconditional_keys_active_fraction=sum(v>1e-7 for v in keys)/len(keys) if keys else None)


def analyze(total: float, events: list[Event], binsize: float=0.5):
    onset,coverage,counts=make_lanes(total,events,binsize)
    radius=max(1,round(8.0/binsize))
    raw={}
    short={}
    delayed={}
    for idx,a in enumerate(ROLES):
        for b in ROLES[idx+1:]:
            key=f'{a}:{b}'
            raw[key]=pearson(onset[a],onset[b])
            short[key]=pearson(residual(onset[a],radius),residual(onset[b],radius))
            c={lag:lagged_corr(residual(onset[a],radius),residual(onset[b],radius),lag)
               for lag in (-4,-3,-2,-1,0,1,2,3,4)}
            values=[(lag,val) for lag,val in c.items() if val is not None]
            best=max(values,key=lambda kv: abs(kv[1])) if values else None
            delayed[key]={'strongest_lag_bins':best[0],'correlation':best[1]} if best else None
    return dict(scope='Score note onsets/velocities, not measured acoustic energy or causal proof',
      bin_beats=binsize,bins=math.ceil(total/binsize),event_counts=counts,
      raw_attack_correlation=raw,detrended_attack_correlation=short,
      strongest_shortscale_lag=delayed,
      lead_attack_drop_to_keys=fall_response(onset['lead'],onset['keys'],max(1,round(1/binsize))),
      fraction_silent_score_bins=sum(all(coverage[r][i]<1e-7 for r in ROLES[:4]) and onset['drums'][i]<1e-7
                  for i in range(len(onset['drums'])))/len(onset['drums']),
      mean_onset_energy={r:statistics.fmean(v) for r,v in onset.items()},
      mean_sounding_energy_proxy={r:statistics.fmean(v) for r,v in coverage.items()},
      caution=('Zero-lag correlation is not an artistic goal; anticorrelation may be conversation. '
               'Shared form and compressor/FX confound waveform dynamics. '
               'A staggered response remains observational until controlled causal interventions.'))


class Characterization(unittest.TestCase):
    def test_native_tsv_protocol(self):
        s='T\t144\t4\t8\nN\tlead\t0.5\t1\t64\t0.7\tA\tSome(0)\tNone\nD\tKick\t1.0\t0.8\tA\nC\t0\t4\t9\tMin\tTonic\t0\n'
        tempo,total,events=parse_tsv(s)
        self.assertEqual((tempo,total,len(events)),(144.,8.,2))
        self.assertEqual(analyze(total,events)['event_counts']['drums'],1)

    def test_uniform_copy_has_perfect_zero_lag(self):
        values=[0.2,0.4,0.3,0.8,0.5,0.3,0.9,0.6]*3
        events=[]
        for i,v in enumerate(values):
            for r in ROLES:
                events.append(Event(r,float(i),0.45,v))
        a=analyze(24,events,1.)
        self.assertAlmostEqual(a['raw_attack_correlation']['lead:keys'],1.)
        self.assertAlmostEqual(a['raw_attack_correlation']['bass:drums'],1.)

    def test_complementary_key_response_is_not_simultaneous_uniformity(self):
        events=[]
        for b in range(0,20,2):
            events.append(Event('lead',float(b),0.45,.8))
            events.append(Event('keys',float(b+1),0.45,.6))
        a=analyze(20,events,1.)
        self.assertEqual(a['lead_attack_drop_to_keys']['fraction'],1.0)
        self.assertLess(a['raw_attack_correlation']['lead:keys'],-0.95)

    def test_same_song_can_have_true_silence(self):
        events=[Event('lead',0.,0.5,.8), Event('bass',0.,0.5,.7),Event('drums',0.,0.,.6)]
        x=analyze(4.,events,.5)
        self.assertGreater(x['fraction_silent_score_bins'],.5)

    def test_reject_malformed_event_and_size(self):
        with self.assertRaises(ValueError):parse_tsv('T\t144\t4\t8\nN\tkeys\t9\t.5\t64\t.4\n')
        with self.assertRaises(ValueError):analyze(10, [], 1e-10)
        with self.assertRaises(ValueError):parse_tsv('N\tlead\t1\t1\t60\t.5\n')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--tsv', type=Path, help='Score TSV from fusion_gates --dump-tsv')
    p.add_argument('--out',type=Path,help='write JSON report')
    p.add_argument('--bin-beats',type=float,default=0.5)
    p.add_argument('--self-test',action='store_true')
    args=p.parse_args()
    if args.self_test:
        suite=unittest.defaultTestLoader.loadTestsFromTestCase(Characterization)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    if not args.tsv:p.error('--tsv required unless --self-test')
    tempo,total,events=parse_tsv(args.tsv.read_text())
    report=dict(source_tsv=str(args.tsv),tempo_bpm=tempo,total_beats=total,
                **analyze(total,events,args.bin_beats))
    output=json.dumps(report,indent=2,allow_nan=False)+'\n'
    if args.out:
        args.out.parent.mkdir(parents=True,exist_ok=True)
        args.out.write_text(output)
    print(output,end='')
    return 0


if __name__=='__main__':raise SystemExit(main())
