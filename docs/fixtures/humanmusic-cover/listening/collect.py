#!/usr/bin/env python3
"""Collect existing, completed cover lab artifacts. Never generate or choose replacements."""
from pathlib import Path
import datetime
import gzip
import hashlib
import json
import re
import subprocess
import wave

ROOT = Path(__file__).resolve().parents[4]
DEST = Path(__file__).resolve().parent
SOURCE = '04e81fc34a32644a223567cae6165d194027952f'
RUNS = [('ode', 'ode-final', 'ode-lab'), ('swing-partial', 'swing-partial-final', 'swing-lab'), ('generated', 'generated-final', 'generated-lab')]

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def rust_fields(text):
    # Preserve anything beyond the limited scalar/list vocabulary verbatim.
    entries, start, depth, quoted, escape = [], 0, 0, False, False
    for i, c in enumerate(text):
        if quoted:
            if escape: escape = False
            elif c == '\\': escape = True
            elif c == '"': quoted = False
        elif c == '"': quoted = True
        elif c in '[({': depth += 1
        elif c in '])}': depth -= 1
        elif c == ',' and depth == 0:
            entries.append(text[start:i]); start = i + 1
    entries.append(text[start:])
    result = {}
    for entry in entries:
        key, value = entry.strip().split(':', 1)
        value = value.strip()
        if value in ('true', 'false'): parsed = value == 'true'
        elif value == 'None': parsed = None
        elif value == '[]': parsed = []
        elif value.isdigit(): parsed = int(value)
        else: parsed = {'rust_text': value}
        result[key] = parsed
    return result

def report(text):
    result = {'raw_report_is_authority': True, 'axis_knowledge': dict(re.findall(r'^  ([a-z-]+): (Invariant|Unknown|Free)$', text, re.M)), 'variants': []}
    result['cover_map_canonical'] = re.search(r'^CoverMap canonical=(\w+)$', text, re.M).group(1)
    headers = list(re.finditer(r'^([a-z_0-9]+): (seed=.+|INFEASIBLE .+)$', text, re.M))
    first = None
    for i, header in enumerate(headers):
        name, metadata = header.groups()
        block = text[header.end():headers[i+1].start() if i+1<len(headers) else len(text)]
        row = {'name': name}
        if metadata.startswith('INFEASIBLE '):
            row.update(status='infeasible', reason=metadata.removeprefix('INFEASIBLE ')); result['variants'].append(row); continue
        row.update(dict(re.findall(r'(seed|world|key|tempo|language|score|perf)=([^ ]+)', metadata)))
        for field in ('seed', 'key'): row[field] = int(row[field])
        row['tempo'] = float(row['tempo'])
        match = re.search(r'^CoverConformance: (PASS|FAIL) \(metric length (true|false)\)$', block, re.M)
        row['conformance'] = match.group(1); row['metric_length_passed'] = match.group(2) == 'true'
        row['axis_checks'] = [{'axis': a, 'result': r, 'relation_and_detail': relation} for a,r,relation in re.findall(r'^  ([a-z-]+): (PASS|FAIL) — (.+)$', block, re.M)]
        match = re.search(r'^Pipeline (PASS|FAIL)(?: \(preserved\))?: CoverPipelineReceipt \{ (.+) \}$', block, re.M)
        row['pipeline'] = match.group(1); row['pipeline_receipt'] = rust_fields(match.group(2))
        row['status'] = 'passed_declared_machine_checks' if row['pipeline'] == row['conformance'] == 'PASS' else 'preserved_failed_candidate'
        freedom = re.search(r'^Freedom versus first cover: CoverFreedom \{ (.+) \}$', block, re.M)
        if freedom:
            row['freedom_reference'] = first; row['freedom'] = rust_fields(freedom.group(1))
        if first is None: first = name
        result['variants'].append(row)
    result['counts'] = {key: sum(v['status'] == key for v in result['variants']) for key in ('passed_declared_machine_checks', 'preserved_failed_candidate', 'infeasible')}
    return result

def copy_text(source, relative):
    raw = source.read_bytes()
    normalized = ('\n'.join(line.rstrip() for line in raw.decode('utf-8').splitlines()) + '\n').encode()
    destination = DEST / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(normalized)
    entry = {'source': str(source.relative_to(ROOT)), 'receipt': str(destination.relative_to(ROOT)), 'source_sha256': hashlib.sha256(raw).hexdigest(), 'receipt_sha256': digest(destination), 'source_bytes': len(raw), 'normalized_trailing_whitespace': raw != normalized}
    if raw != normalized:
        archived = DEST / 'raw' / (str(relative) + '.gz')
        archived.parent.mkdir(parents=True, exist_ok=True)
        archived.write_bytes(gzip.compress(raw, mtime=0))
        entry['raw_gzip'] = str(archived.relative_to(ROOT)); entry['raw_gzip_sha256'] = digest(archived)
    return entry

def main():
    if (DEST/'receipt.json').exists():
        raise SystemExit('Receipt exists; refusing to rebind the initial source or overwrite evidence.')
    for _, name, _ in RUNS:
        if not (ROOT/'target/humanmusic-cover'/name/'report.txt').is_file():
            raise SystemExit(f'{name}: no complete report yet')
    subprocess.run(['git','diff','--exit-code',SOURCE,'--','src','Cargo.toml','Cargo.lock','examples/cover_music_lab.rs'],cwd=ROOT,check=True)
    receipt = {'schema':'humanmusic-cover-listening/v1', 'collected_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(), 'source_commit':SOURCE, 'source_claim':'Root ran all three unchanged declared labs serially from this production/example source; no replacements selected.', 'rustc':subprocess.check_output(['rustc','+1.98.1','--version','--verbose'],cwd=ROOT,text=True).strip(), 'runs':{}, 'text_artifacts':[], 'wav_artifacts':[], 'human_recognition':'UNVERIFIED', 'independence':'Development listening corpus, not an untouched holdout. Shared generator/observer provenance; machine claims do not establish recognition.'}
    binary=ROOT/'target/release/examples/cover_music_lab'
    receipt['lab_binary']={'path':str(binary.relative_to(ROOT)),'sha256':digest(binary)}
    paths = subprocess.check_output(['git','ls-tree','-r','--name-only',SOURCE,'--','src','Cargo.toml','Cargo.lock','examples/cover_music_lab.rs'],cwd=ROOT,text=True).splitlines()
    source_manifest = {p: digest(ROOT/p) for p in paths}
    (DEST/'source-manifest.json').write_text(json.dumps({'source_commit':SOURCE,'files':source_manifest},indent=2)+'\n')
    receipt['source_manifest_sha256']=digest(DEST/'source-manifest.json')
    for label,name,logname in RUNS:
        directory=ROOT/'target/humanmusic-cover'/name
        parsed=report((directory/'report.txt').read_text())
        parsed['output_directory']=str(directory.relative_to(ROOT)); receipt['runs'][label]=parsed
        for file in sorted(directory.iterdir()):
            if file.suffix in ('.txt','.tsv'):
                receipt['text_artifacts'].append(copy_text(file,Path(label)/file.name))
            elif file.suffix == '.wav':
                with wave.open(str(file),'rb') as wav:
                    entry={'path':str(file.relative_to(ROOT)),'run':label,'sha256':digest(file),'bytes':file.stat().st_size,'channels':wav.getnchannels(),'sample_rate':wav.getframerate(),'sample_width_bytes':wav.getsampwidth(),'frames':wav.getnframes(),'duration_seconds':wav.getnframes()/wav.getframerate()}
                variant=next((v for v in parsed['variants'] if v['name']==file.stem),None)
                entry['claim']=variant['status'] if variant else 'source_melody_only' if label=='ode' else 'generated_reference'
                receipt['wav_artifacts'].append(entry)
        receipt['text_artifacts'].append(copy_text(ROOT/'target/humanmusic-consolidation/final-logs'/f'{logname}.txt',Path('logs')/f'{logname}.txt'))
    receipt['source_facts']={
        'ode':{'authority':'docs/fixtures/humanmusic-cover/sources/mutopia-528/provenance.json','selected_voice':'sop','selected_notes':62,'retained_source_voice_notes':245,'key':'G major','meter':'4/4','tempo_bpm':100,'metric_length_beats':64,'source_harmony':'UNKNOWN','source_form_families':'UNKNOWN','raw_lily_midi_match':False,'midi_notes':238,'declared_per_staff_exact_unison_coalescences':7,'projected_match':True,'independence':'Different parsers/encodings of the SAME edition; not independent historical sources.'},
        'swing-partial':{'authority':'docs/fixtures/humanmusic-cover/swing-partial/provenance.json','status':'maintainer provisional ordered harmony/form skeleton','asserted_tonic':'A','asserted_mode':'Ionian','approximate_tempo_bpm':95,'approximate_duration_seconds':179,'straight_timing':'assumption, not measurement','unknown':['source melody','melody rhythm','bass figure','drum pattern','intro harmony','exact section/chord durations','internal repetition counts','source arrangement'],'generated_target_assumptions':['one bar per supplied chord','generated tonic intro'],'lyrics_retained':False,'audio_acquired':False,'recognizable_song_claim':'UNVERIFIED; these are partial-cover skeletons'},
        'generated':{'reference_world':'BLACK_ICE','trace':'demo_trace(64.0)','song_seed':2112,'grammar':'HookArc','reference_entry':'perform_pocketed','observed_contract_pins':['motif','groove','harmonic-contour','form']}}
    (DEST/'receipt.json').write_text(json.dumps(receipt,indent=2,ensure_ascii=False)+'\n')
    (DEST/'WAV_SHA256SUMS').write_text(''.join(f"{w['sha256']}  {w['path']}\n" for w in receipt['wav_artifacts']))
    lines=['# Cover listening corpus at 04e81fc', '', 'These are the original serial lab outputs, with no replacement seeds or post-contact fitting. WAVs remain local under `target/humanmusic-cover/`; hashes and structural/audio metadata are in `receipt.json` and `WAV_SHA256SUMS`. Human recognition and musical quality are **UNVERIFIED**. The render command returned successfully even when a candidate failed its explicit pipeline checks.', '', '| Run | Declared machine checks pass | Preserved failed candidates | Infeasible targets |', '| --- | ---: | ---: | ---: |']
    for label,run in receipt['runs'].items():
        c=run['counts'];lines.append(f"| {label} | {c['passed_declared_machine_checks']} | {c['preserved_failed_candidate']} | {c['infeasible']} |")
    lines += ['', 'Every generated candidate passed its selected cover-axis projection. Failures remain failures: Ode transposed/faster has three temporal-function claims; all three generated-source candidates have `song: false`, and its VAPOR candidate also has one temporal-function claim. The two infeasible generated-source targets violate the pinned-harmony target vocabulary.', '', 'The Ode map pins observed soprano motif, canonical attacks and source rest boundaries; unavailable harmony/form/groove/bass remain Unknown. It renders the observed source melody plus six candidate interpretations, including original G-major/100-BPM frame. Every comparison to the first candidate reports changes in support voicing, dynamics, percussion detail and interaction choices.', '', 'Swing outputs preserve only the supplied ordered relative harmony and section-family topology. Melody and exact timing remain Unknown; one target bar per chord and the tonic intro are explicit generation assumptions. Passing this partial relation does not establish a recognizable cover of Swing & A Miss.', '', 'Generated-source outputs pin motif, groove, harmonic contour and form. All available outputs preserve those coordinates and vary all four measured free-coordinate groups, but their failed song/pipeline checks prevent a lawful-cover claim.', '', 'Source facts and normalization limits are recorded in receipt.json, including the raw 245-versus-238 symbolic mismatch and declared seven-unison staff projection. Whitespace-normalized text copies retain original byte hashes; any changed original is also preserved as deterministic gzip in raw/. `collect.py` only collects existing completed files and refuses to overwrite an existing receipt.', '', 'Listen without titles where possible: is the recognized melody the same song? Do different bands interpret it rather than merely change patches? The partial skeleton cannot answer source-melody recognition. Accepted R17 BLACK_ICE preservation is a separate frozen comparison, not established by these cover outputs.']
    (DEST/'README.md').write_text('\n'.join(lines)+'\n')
    files=sorted(p for p in DEST.rglob('*') if p.is_file() and p.name!='SHA256SUMS')
    (DEST/'SHA256SUMS').write_text(''.join(f'{digest(p)}  {p.relative_to(DEST)}\n' for p in files))
    print(json.dumps({'source_commit':SOURCE,'runs':{k:v['counts'] for k,v in receipt['runs'].items()},'wav_count':len(receipt['wav_artifacts']),'text_count':len(receipt['text_artifacts'])},indent=2))

if __name__=='__main__':main()
