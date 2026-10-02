import collections,hashlib,json,pathlib,re,sys
root=pathlib.Path(__file__).resolve().parents[4]
old=root/'docs/fixtures/humanmusic-r17/verification'
out=root/'docs/fixtures/humanmusic-consolidation/known-r17-replay'
families=['accepted pitch changed','off-lattice','new temporal claim','new held identity flip']
def parse(text):
    cases=[]; failures=[]; passes=[]; reason=None
    for line in text.splitlines():
        line=re.sub(r'^test r17_fresh_world_seed_tempo_sweep \.\.\. ', '', line)
        if any(line.startswith(f+' ') for f in families): reason=line
        if line.startswith('FAIL seed='):
            case=line[5:]
            assert reason and case in reason,(case,reason)
            failures.append({'case':case,'reason':reason});cases.append(case);reason=None
        elif line.startswith('PASS seed='):
            case=line[5:].split(' notes=')[0]
            passes.append(case);cases.append(case)
    assert len(cases)==120 and len(set(cases))==120,(len(cases),len(set(cases)))
    counts=collections.Counter(next(f for f in families if row['reason'].startswith(f+' ')) for row in failures)
    return {'executed':len(cases),'pass':len(passes),'fail':len(failures),'failure_families':dict(counts),'failures':failures,'passes':passes}
original=json.loads((old/'fresh-failures.json').read_text())
calibrated=parse((old/'fresh-sweep.txt').read_text())
assert calibrated['failures']==original['failures'],'parser fails frozen original receipts'
assert all(calibrated[k]==original[k] for k in ['pass','fail','failure_families'])
if len(sys.argv)==1:
    print('Parser calibrated: all 120 cases, 61 passes, 59 exact first-reason records match original JSON')
    sys.exit()
current=parse(pathlib.Path(sys.argv[1]).read_text())
current['source']='44e2d7efabc28e684701240b5466d06a91d541dd'
current['original_source']=original['source']
current['known_not_holdout']=True
old_by={r['case']:r['reason'] for r in original['failures']}
new_by={r['case']:r['reason'] for r in current['failures']}
current['comparison']={'all_first_reasons_identical':old_by==new_by,'passes_identical':calibrated['passes']==current['passes'],'new_failure_cases':sorted(new_by.keys()-old_by.keys()),'removed_failure_cases':sorted(old_by.keys()-new_by.keys()),'changed_first_reasons':[c for c in old_by.keys()&new_by.keys() if old_by[c]!=new_by[c]]}
current['families_by_world']={f:dict(collections.Counter(re.search(r'world=(\S+)',r['case'])[1] for r in current['failures'] if r['reason'].startswith(f+' '))) for f in families}
(out/'classification.json').write_text(json.dumps(current,indent=2)+'\n')
print(json.dumps({k:v for k,v in current.items() if k not in ['failures','passes']},indent=2))
