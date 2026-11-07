"""LibGibson v0.8 parity slice -- transcript comparator.

Reads the per-language transcripts written by scripts/dev/parity_v08.sh and checks them against
the committed expectation (bindings/parity/v08/expected.json). Exit status 1 means a REGRESSION
against that expectation; known, documented gaps (the expected differing record ids) are not
regressions. See docs/V08_PARITY_SLICE.md.

    compare.py matrix  --script S --expected E --dir RUN_DIR
    compare.py check   --script S --expected E --ref C.jsonl --lang L --transcript T [--want ID,...]

`matrix` compares every language against C (the ABI reference) and against Rust, checks replay
determinism (run A == run B == rep 1 of the in-process run, and iter_identical) and the resource
bounds, and prints the capability matrix. `check` compares one transcript for one language against
the reference and the expectation (used by the negative controls): it exits 1 when it finds a
regression; with --want ID,...[,ORDER] it also prints whether the regression found is exactly that
set (ORDER = the record order changed).
"""

import argparse
import json
import sys

LANGS = ["c", "cpp", "py", "go", "rust"]
DIMS = ["EVT", "GEO", "CELL", "TXT", "ERR", "LIFE", "RES", "DET"]
DIM_TITLE = {
    "EVT": "event identity & ordering",
    "GEO": "geometry",
    "CELL": "rendered cells & styles",
    "TXT": "text & Unicode semantics",
    "ERR": "failure codes",
    "LIFE": "lifecycle/ownership",
    "RES": "resource bounds",
    "DET": "replay determinism",
}


def step_dims(script_path):
    dims = {}
    with open(script_path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\r\n")
            if line and not line.startswith("#"):
                tok = line.split(" ")
                dims[tok[0]] = tok[1]
    return dims


def dim_of(dims, rid):
    return dims.get(rid.split(".")[0], "?")


def canonical_problems(text):
    """Every line must be a JSON object with sorted keys, compact separators, and no floats."""
    problems = []
    for n, line in enumerate(text.splitlines(), 1):
        try:
            obj = json.loads(line)
        except ValueError as e:
            problems.append("line %d: not JSON (%s)" % (n, e))
            continue
        if not isinstance(obj, dict) or "id" not in obj or "k" not in obj:
            problems.append("line %d: not a record with id and k" % n)
            continue
        if json.dumps(obj, sort_keys=True, separators=(",", ":")) != line:
            problems.append("line %d: not canonical (key order / spacing / escaping)" % n)

        def has_float(v):
            if isinstance(v, float):
                return True
            if isinstance(v, list):
                return any(has_float(x) for x in v)
            return False

        if any(has_float(v) for v in obj.values()):
            problems.append("line %d: contains a float" % n)
    return problems


def records(text):
    out, order = {}, []
    for line in text.splitlines():
        rid = json.loads(line)["id"]
        if rid in out:
            out[rid] = out[rid] + "\n" + line  # duplicate id: keep both so it never compares equal
        else:
            order.append(rid)
            out[rid] = line
    return out, order


def diff_ids(a_text, b_text):
    a, ao = records(a_text)
    b, bo = records(b_text)
    ids = sorted(set(a) | set(b))
    differing = [i for i in ids if a.get(i) != b.get(i)]
    common = set(a) & set(b)
    order_ok = [i for i in ao if i in common] == [i for i in bo if i in common]
    return differing, order_ok


def read(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def resource_line(path):
    text = read(path) or ""
    for line in text.splitlines():
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("k") == "resource":
            return obj
    return None


def check_against(lang, text, ref_text, expected, dims, out_lines):
    """Regressions of one transcript vs the reference + expectation. Returns (new_ids, problems)."""
    problems = canonical_problems(text)
    if problems:
        # Not parseable as records: nothing below can be trusted.
        return [], problems, False
    # Completeness: exactly the expected record ids, each once. An omitted record must never pass
    # as "no difference", and a duplicate must never be absorbed.
    ids = [json.loads(line)["id"] for line in text.splitlines()]
    want_ids = expected["record_ids"]
    seen = set()
    dups = sorted({i for i in ids if i in seen or seen.add(i)})
    missing = sorted(set(want_ids) - set(ids))
    extra = sorted(set(ids) - set(want_ids))
    for rid in missing:
        problems.append("MISSING record %s (%s)" % (rid, dim_of(dims, rid)))
    for rid in extra:
        problems.append("UNEXPECTED record %s" % rid)
    for rid in dups:
        problems.append("DUPLICATE record %s" % rid)
    differing, order_ok = diff_ids(ref_text, text)
    allowed = set(expected["differs_from_c"].get(lang, []))
    new = sorted(set(differing) - allowed)
    healed = sorted(allowed - set(differing))
    # A known gap is pinned to its exact record: a change inside it is a regression too.
    pinned = expected.get("pinned", {}).get(lang, {})
    recs, _ = records(text)
    for rid in sorted(allowed & set(differing)):
        if rid in recs and recs[rid] != pinned.get(rid):
            new.append(rid)
            problems.append("CHANGED inside known gap %s (%s): pinned record differs" % (rid, dim_of(dims, rid)))
    new = sorted(set(new) | set(missing) | set(dups))
    if not order_ok:
        problems.append("record ORDER differs from the reference")
    for rid in sorted(set(differing) - allowed):
        problems.append("NEW difference vs c at %s (%s)" % (rid, dim_of(dims, rid)))
    for rid in healed:
        out_lines.append("  note: %s: expected difference at %s no longer present (expected.json is stale)" % (lang, rid))
    return new, problems, order_ok


def cmd_check(args):
    expected = json.load(open(args.expected, encoding="utf-8"))
    dims = step_dims(args.script)
    text = read(args.transcript)
    ref = read(args.ref)
    if text is None or ref is None or not text.strip():
        print("check %s: MISSING transcript" % args.lang)
        return 1
    notes = []
    new, problems, order_ok = check_against(args.lang, text, ref, expected, dims, notes)
    for line in notes:
        print(line)
    for p in problems:
        print("  %s: %s" % (args.lang, p))
    if args.want is not None:
        # The regression actually found, as ids plus the token ORDER for a record-order change.
        want = sorted(x for x in args.want.split(",") if x)
        got = sorted(new + ([] if order_ok else ["ORDER"]))
        print("check %s: found=%s want=%s -> %s" % (
            args.lang, ",".join(got) or "-", ",".join(want) or "-", "EXACT" if got == want else "MISMATCH"))
    return 1 if problems else 0


def cmd_matrix(args):
    expected = json.load(open(args.expected, encoding="utf-8"))
    dims = step_dims(args.script)
    d = args.dir
    regress = []
    notes = []
    texts = {lang: read("%s/%s.a.jsonl" % (d, lang)) for lang in LANGS}
    ref = texts["c"]
    if not ref:
        print("REGRESSION: no C reference transcript")
        return 1

    dim_ok = {lang: {dim: True for dim in DIMS} for lang in LANGS}
    summary = []
    for lang in LANGS:
        t = texts[lang]
        if not t:
            regress.append("%s: transcript missing (build or run failed)" % lang)
            for dim in DIMS:
                dim_ok[lang][dim] = False
            continue
        new, problems, _ = check_against(lang, t, ref, expected, dims, notes)
        for p in problems:
            regress.append("%s: %s" % (lang, p))
        for rid in new:
            dim_ok[lang][dim_of(dims, rid)] = False
        if any("ORDER" in p or "canonical" in p or "JSON" in p or "float" in p for p in problems):
            for dim in DIMS[:6]:
                dim_ok[lang][dim] = False
        dc, order_c = diff_ids(ref, t)
        rust = texts["rust"]
        dr, order_r = diff_ids(rust, t) if rust else (["<no rust transcript>"], False)
        want_r = set(expected["differs_from_rust"].get(lang, []))
        new_r = sorted(set(dr) - want_r) if rust else []
        for rid in new_r:
            regress.append("%s: NEW difference vs rust at %s" % (lang, rid))
            dim_ok[lang][dim_of(dims, rid)] = False
        for rid in sorted(want_r - set(dr)) if rust else []:
            notes.append("  note: %s: expected difference vs rust at %s no longer present (expected.json is stale)" % (lang, rid))
        nrec = len(t.splitlines())
        summary.append((lang, nrec, dc, order_c, dr, order_r))

        # replay determinism: two processes, plus every in-process repetition
        b = read("%s/%s.b.jsonl" % (d, lang))
        rep1 = read("%s/%s.rep.jsonl" % (d, lang))
        res = resource_line("%s/%s.rep.err" % (d, lang))
        det = b == t and rep1 == t and res is not None and res.get("iter_identical") == 1
        if not det:
            regress.append("%s: replay determinism failed (A==B:%s A==rep1:%s iter_identical:%s)" % (
                lang, b == t, rep1 == t, None if res is None else res.get("iter_identical")))
            dim_ok[lang]["DET"] = False
        # Leak signature: peak RSS still rising over the SECOND half of the repetitions. Growth
        # between rep 1 and the midpoint is runtime warm-up (e.g. a GC heap reaching its pacing
        # plateau) and is reported, not gated.
        bound = expected["resource"]["max_rss_second_half_growth_kb"]
        if res is None or "maxrss_kb_mid" not in res:
            regress.append("%s: no resource record" % lang)
            dim_ok[lang]["RES"] = False
        else:
            growth = res["maxrss_kb_n"] - res["maxrss_kb_mid"]
            if growth > bound:
                regress.append("%s: maxrss grew %d KB over the second half of %d reps (bound %d KB)" % (
                    lang, growth, res["reps"], bound))
                dim_ok[lang]["RES"] = False

    print("== transcript agreement (record ids that differ) ==")
    for lang, nrec, dc, order_c, dr, order_r in summary:
        print("  %-4s %3d records | vs c: %s%s | vs rust: %s%s" % (
            lang, nrec,
            "byte-identical" if not dc and order_c else "%d differ: %s" % (len(dc), ",".join(dc)),
            "" if order_c else " [ORDER DIFFERS]",
            "byte-identical" if not dr and order_r else "%d differ: %s" % (len(dr), ",".join(dr)),
            "" if order_r else " [ORDER DIFFERS]",
        ))

    print("== resource bounds (in-process repetitions of the whole script) ==")
    for lang in LANGS:
        res = resource_line("%s/%s.rep.err" % (d, lang))
        if res:
            print("  %-4s reps=%d wall_ms=%d per_rep_us=%d maxrss_kb rep1=%d mid=%d end=%d"
                  " warmup_growth=%d second_half_growth=%d iter_identical=%d" % (
                      lang, res["reps"], res["wall_ms"], res["per_rep_us"], res["maxrss_kb_1"],
                      res.get("maxrss_kb_mid", -1), res["maxrss_kb_n"],
                      res.get("maxrss_kb_mid", res["maxrss_kb_1"]) - res["maxrss_kb_1"],
                      res["maxrss_kb_n"] - res.get("maxrss_kb_mid", res["maxrss_kb_1"]), res["iter_identical"]))

    print("== capability matrix (label from expected.json; [ok] = observed matches it, [REGRESSION] = not) ==")
    labels = expected["labels"]
    head = "  %-28s" % "dimension" + "".join("%-22s" % lang for lang in LANGS)
    print(head)
    for dim in DIMS:
        row = "  %-28s" % DIM_TITLE[dim]
        for lang in LANGS:
            lab = labels[dim][lang][0]
            row += "%-22s" % ("%s %s" % (lab, "[ok]" if dim_ok[lang][dim] else "[REGRESSION]"))
        print(row)

    for line in notes:
        print(line)
    if regress:
        print("== REGRESSIONS ==")
        for r in regress:
            print("  " + r)
        return 1
    print("== no regression against expected.json ==")
    return 0


def cmd_resource(args):
    """Resource gate alone, for one resource record (used by the planted-leak control)."""
    expected = json.load(open(args.expected, encoding="utf-8"))
    res = resource_line(args.err)
    bound = expected["resource"]["max_rss_second_half_growth_kb"]
    if res is None or "maxrss_kb_mid" not in res:
        print("resource: no resource record in %s" % args.err)
        return 1
    growth = res["maxrss_kb_n"] - res["maxrss_kb_mid"]
    verdict = "EXCEEDED" if growth > bound else "within"
    print("resource %s: second_half_growth=%d KB (rep %d..%d) bound=%d KB -> %s" % (
        res["lang"], growth, (res["reps"] + 1) // 2, res["reps"], bound, verdict))
    return 1 if growth > bound else 0


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("matrix")
    m.add_argument("--script", required=True)
    m.add_argument("--expected", required=True)
    m.add_argument("--dir", required=True)
    c = sub.add_parser("check")
    c.add_argument("--script", required=True)
    c.add_argument("--expected", required=True)
    c.add_argument("--ref", required=True)
    c.add_argument("--lang", required=True)
    c.add_argument("--transcript", required=True)
    c.add_argument("--want")
    r = sub.add_parser("resource")
    r.add_argument("--expected", required=True)
    r.add_argument("--err", required=True)
    args = ap.parse_args()
    return {"matrix": cmd_matrix, "check": cmd_check, "resource": cmd_resource}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
