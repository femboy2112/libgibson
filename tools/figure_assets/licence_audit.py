#!/usr/bin/env python3
"""Per-file licence audit for the M5-C body assets.  Pure python, no bpy.

Run:  python3 -I tools/figure_assets/licence_audit.py \
          --mpfb-data "$MPFB_DATA" --user-data "$MPFB_USER_DATA" \
          --out-md docs/evidence/m5c/LICENCE_MANIFEST.md \
          --out-json docs/evidence/m5c/LICENCE_MANIFEST.json

Rule (RULING #82 Q1 / BRIEF_COMMON 5): a third-party file is usable only if its
CC0 status is SHOWN by evidence on disk (a quoted header line, an in-file
"license" field, or a pack index entry) or by an entry the coordinator added to
licence_evidence_external.json (URL + quoted text).  Everything else is DROP:
that part goes procedural / self-authored.  This tool never invents evidence and
never fetches anything.  Paths in the outputs are relative to the asset roots, so
no host path is written.

Second mode:  --check-export OUT/manifest.json  cross-checks the sources an export
used against the audit result (sha256 equality, status != DROP).
"""
import argparse
import gzip
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CC0_RE = re.compile(r"CC0|CC-0|Creative Commons Zero", re.I)
OTHER_RE = re.compile(r"CC-BY|CC BY|Attribution|GPL|All rights reserved|non-?commercial", re.I)
HEADER_LINES = 60

# Facts recorded by the coordinator's install note (not re-derived here).
INSTALL_FACTS = [
    ("MPFB 2.0.18 extension zip sha256", "c075b3e9b94b8e5556a5f03f4653602d02ea306de08126e8f55ed6dee28a2dad",
     "matches extensions.blender.org API archive_hash (coordinator install note)"),
    ("MakeHuman system assets pack zip sha256", "b542127a8e25547c7c29c19f2d1d2adb9a664c80396ecd694095dbc8028a0107",
     "trust-on-first-use: no upstream checksum published (coordinator install note)"),
]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def open_text(path):
    if path.endswith(".gz"):
        return gzip.open(path, "rt", encoding="utf-8", errors="replace")
    return open(path, "r", encoding="utf-8", errors="replace")


def header_lines(path):
    # PNG bytes have no text header. The material's entry in the pack index
    # supplies the licensing evidence for its texture.
    if path.lower().endswith(".png"):
        return []
    out = []
    try:
        with open_text(path) as f:
            for i, line in enumerate(f):
                if i >= HEADER_LINES:
                    break
                out.append((i + 1, line.rstrip("\n")))
    except (OSError, EOFError) as exc:
        out.append((0, "<unreadable: %s>" % exc))
    return out


def json_license_field(path):
    """Return the top-level 'license' string of a small-ish JSON file, or None."""
    if not path.endswith(".json"):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            d = json.load(f)
    except (OSError, ValueError):
        return None
    if isinstance(d, dict) and isinstance(d.get("license"), str):
        return d["license"]
    return None


def evidence_for(path, entry, pack_index, external):
    """Return (status, [evidence strings])."""
    ev = []
    conflicts = []
    cc0 = False
    kind = None
    # 1. header lines
    hl = header_lines(path)
    cc0_lines = [(n, l) for n, l in hl if CC0_RE.search(l)]
    bad_lines = [(n, l) for n, l in hl if OTHER_RE.search(l) and not CC0_RE.search(l)]
    if cc0_lines:
        n0, n1 = cc0_lines[0][0], cc0_lines[-1][0]
        quote = " ".join(l.lstrip("# ").strip() for _, l in cc0_lines)
        ev.append('file header lines %d-%d: "%s"' % (n0, n1, quote))
        cc0 = True
        kind = "CC0-HEADER"
    for n, l in bad_lines:
        conflicts.append('file header line %d: "%s"' % (n, l.lstrip("# ").strip()))
    # 2. in-file licence field
    lic = json_license_field(path)
    if lic is not None:
        ev.append('in-file field "license": "%s"' % lic)
        if CC0_RE.search(lic):
            cc0 = True
            kind = kind or "CC0-FIELD"
        else:
            conflicts.append('in-file field "license": "%s"' % lic)
    # 3. pack index entry
    key = entry.get("pack_key")
    if key and pack_index is not None:
        rec = pack_index.get(key)
        if rec is None:
            ev.append("pack index has no entry for '%s'" % key)
        else:
            q = 'pack index packs/makehuman_system_assets.json["%s"]: license="%s", author="%s", source="%s"' % (
                key, rec.get("license"), rec.get("author"), rec.get("source"))
            ev.append(q)
            if isinstance(rec.get("license"), str) and CC0_RE.search(rec["license"]):
                cc0 = True
                kind = kind or "CC0-PACKINDEX"
            else:
                conflicts.append(q)
    # 4. external evidence from the coordinator
    for rec in external.get(entry["id"], []):
        q = rec.get("quote", "")
        if q and CC0_RE.search(q) and rec.get("url"):
            ev.append('external evidence %s (%s): "%s"' % (rec["url"], rec.get("retrieved", "date unrecorded"), q))
            cc0 = True
            kind = kind or "CC0-EXTERNAL"
    if conflicts:
        return "DROP", ev + ["CONFLICT: " + c for c in conflicts]
    if not cc0:
        ev.append("no CC0 statement found on disk or in licence_evidence_external.json")
        return "DROP", ev
    return kind, ev


def load_pack_index(user_root):
    p = os.path.join(user_root, "packs", "makehuman_system_assets.json")
    try:
        with open(p, "r", encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def load_external():
    p = os.path.join(HERE, "licence_evidence_external.json")
    out = {}
    try:
        with open(p, "r", encoding="utf-8") as f:
            d = json.load(f)
    except (OSError, ValueError):
        return out
    for rec in d.get("entries", []):
        out.setdefault(rec.get("id", ""), []).append(rec)
    return out


def expand_entries(assets, controls, mpfb_root, user_root):
    """Return a list of row dicts: id, role, root, files=[relpath,...], source, pack_key."""
    rows = []
    for e in assets["files"]:
        root = mpfb_root if e["root"] == "mpfb" else user_root
        if "glob" in e:
            pat = e["glob"]
            base = pat.split("**")[0].rstrip("/")
            found = []
            for dp, _dn, fn in os.walk(os.path.join(root, base)):
                for n in fn:
                    if n.endswith(pat.rsplit("*", 1)[-1]):
                        found.append(os.path.relpath(os.path.join(dp, n), root).replace(os.sep, "/"))
            rows.append(dict(e, files=sorted(found)))
        else:
            rows.append(dict(e, files=[e["path"]]))
    # control targets, enumerated from controls.json
    seen = set()
    for c in controls["controls"]:
        if c["kind"] != "targets":
            continue
        for side in ("neg", "pos"):
            for rel, _w in c[side]:
                if rel in seen:
                    continue
                seen.add(rel)
                rows.append({"id": "target:" + rel, "role": "control target (%s.%s)" % (c["name"], side),
                             "root": "mpfb", "files": ["targets/" + rel + ".target.gz"], "source": "mpfb"})
    return rows


def audit(mpfb_root, user_root):
    with open(os.path.join(HERE, "asset_list.json"), "r", encoding="utf-8") as f:
        assets = json.load(f)
    with open(os.path.join(HERE, "controls.json"), "r", encoding="utf-8") as f:
        controls = json.load(f)
    pack_index = load_pack_index(user_root)
    external = load_external()
    out_rows = []
    for e in expand_entries(assets, controls, mpfb_root, user_root):
        root = mpfb_root if e["root"] == "mpfb" else user_root
        files = []
        agg = hashlib.sha256()
        total = 0
        statuses = []
        for rel in e["files"]:
            full = os.path.join(root, rel)
            if not os.path.isfile(full):
                files.append({"path": rel, "sha256": None, "size": None, "status": "DROP",
                              "evidence": ["file missing under the %s root" % e["root"]]})
                statuses.append("DROP")
                continue
            sha = sha256_file(full)
            size = os.path.getsize(full)
            st, ev = evidence_for(full, e, pack_index, external)
            files.append({"path": rel, "sha256": sha, "size": size, "status": st, "evidence": ev})
            statuses.append(st)
            agg.update(("%s:%s\n" % (rel, sha)).encode())
            total += size
        if not files:
            statuses.append("DROP")
        status = "DROP" if "DROP" in statuses else sorted(set(statuses))[0] if len(set(statuses)) == 1 else "+".join(sorted(set(statuses)))
        out_rows.append({
            "id": e["id"], "role": e["role"], "root": e["root"], "source": e.get("source"),
            "source_url": assets["source_urls"].get(e.get("source", ""), ""),
            "evidence_only": bool(e.get("evidence_only")),
            "n_files": len(files), "size": total,
            "sha256": files[0]["sha256"] if len(files) == 1 else agg.hexdigest(),
            "sha256_kind": "file" if len(files) == 1 else "sha256 of sorted 'relpath:sha256' lines",
            "status": status, "files": files})
    return {"format": "figure-licence-manifest/1", "rows": out_rows,
            "self_authored": assets["self_authored"], "not_used": assets["not_used"],
            "install_facts": [{"what": a, "value": b, "note": c} for a, b, c in INSTALL_FACTS]}


def esc(s):
    return str(s).replace("|", "\\|").replace("\n", " ")


def render_md(res):
    rows = res["rows"]
    n_drop = sum(1 for r in rows if r["status"] == "DROP" and not r["evidence_only"])
    n_ok = sum(1 for r in rows if r["status"] != "DROP" and not r["evidence_only"])
    L = []
    L.append("# M5-C licence manifest")
    L.append("")
    L.append("Generated by `tools/figure_assets/licence_audit.py` from the files on disk. No host paths are written;")
    L.append("paths are relative to the asset roots (`mpfb` = MPFB extension data dir, `user` = MakeHuman system asset pack data dir).")
    L.append("Rule: a file is usable only when CC0 is *shown* by evidence (quoted below). Anything else is **DROP** and that part goes procedural/self-authored.")
    L.append("")
    L.append("Summary: %d rows usable (CC0 shown), %d rows DROP (not shown), %d self-authored." % (n_ok, n_drop, len(res["self_authored"])))
    L.append("")
    L.append("## Third-party rows")
    L.append("")
    L.append("| id | root:path | sha256 | bytes | status | evidence (where: quote) | source URL |")
    L.append("|---|---|---|---|---|---|---|")
    for r in rows:
        if r["n_files"] == 1:
            f = r["files"][0]
            path = "%s:%s" % (r["root"], f["path"])
            ev = " ; ".join(f["evidence"])
        else:
            path = "%s:%d files (aggregate)" % (r["root"], r["n_files"])
            ok = sum(1 for f in r["files"] if f["status"] != "DROP")
            first = r["files"][0]["evidence"][0] if r["files"] else "no files"
            ev = "%d of %d files carry CC0 evidence; e.g. %s" % (ok, r["n_files"], first)
        tag = " (evidence only, not shipped)" if r["evidence_only"] else ""
        L.append("| %s | %s | %s | %s | **%s**%s | %s | %s |" % (
            esc(r["id"]), esc(path), r["sha256"] or "MISSING", r["size"], r["status"], tag, esc(ev), esc(r["source_url"])))
    L.append("")
    L.append("## Self-authored (our own geometry, repo licence)")
    L.append("")
    for s in res["self_authored"]:
        L.append("- `%s`: %s. Licence: **%s**. No third-party bytes." % (s["id"], s["role"], s["licence"]))
    L.append("")
    L.append("## Deliberately not used")
    L.append("")
    for s in res["not_used"]:
        L.append("- " + s)
    L.append("")
    L.append("## Install provenance (recorded by the coordinator, not re-derived by this tool)")
    L.append("")
    for f in res["install_facts"]:
        L.append("- %s: `%s` (%s)" % (f["what"], f["value"], f["note"]))
    L.append("")
    L.append("## Reading the DROP rows")
    L.append("")
    dropped = [r for r in rows if r["status"] == "DROP" and not r["evidence_only"]]
    if dropped:
        L.append("These rows have no CC0 statement on disk. Their upstream licence may well be CC0 (the install note records")
        L.append("\"bundled base mesh/targets CC0 per MPFB docs\"), but that is second-hand and not shown by a file, so they stay")
        L.append("DROP until the coordinator adds the page URL and the quoted text to `tools/figure_assets/licence_evidence_external.json`")
        L.append("(format in that file) and the audit is re-run. The MPFB extension manifest declares the extension package GPL-3.0-or-later;")
        L.append("the data files carry no separate licence field.")
        L.append("")
        for r in dropped:
            L.append("- `%s` (%d file%s): %s" % (r["id"], r["n_files"], "" if r["n_files"] == 1 else "s", r["role"]))
    else:
        L.append("No DROP rows.")
    L.append("")
    return "\n".join(L) + "\n"


def check_export(res, manifest_path):
    with open(manifest_path, "r", encoding="utf-8") as f:
        man = json.load(f)
    by_sha = {}
    for r in res["rows"]:
        if r["sha256"] and r["n_files"] > 1:
            by_sha.setdefault(r["sha256"], []).append((r["id"], {"status": r["status"]}))
        for f in r["files"]:
            if f["sha256"]:
                by_sha.setdefault(f["sha256"], []).append((r["id"], f))
    bad = 0
    ok = 0
    for s in man.get("sources", []):
        hits = by_sha.get(s.get("sha256"), [])
        if not hits:
            print("CHECK FAIL source %s sha256 %s is not in the licence manifest" % (s.get("path"), s.get("sha256")))
            bad += 1
        elif any(h[1]["status"] == "DROP" for h in hits):
            print("CHECK FAIL source %s is DROP (no CC0 evidence)" % s.get("path"))
            bad += 1
        else:
            ok += 1
    print("LICENCE_CHECK ok=%d bad=%d" % (ok, bad))
    return bad


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--mpfb-data", default=os.environ.get("MPFB_DATA"))
    ap.add_argument("--user-data", default=os.environ.get("MPFB_USER_DATA"))
    ap.add_argument("--out-md")
    ap.add_argument("--out-json")
    ap.add_argument("--check-export", help="export manifest.json to cross-check against the audit")
    a = ap.parse_args(argv)
    for name, val in (("--mpfb-data / MPFB_DATA", a.mpfb_data), ("--user-data / MPFB_USER_DATA", a.user_data)):
        if not val or not os.path.isdir(val):
            print("ERROR: %s is not set to an existing directory" % name, file=sys.stderr)
            return 2
    res = audit(a.mpfb_data, a.user_data)
    if a.out_json:
        with open(a.out_json, "w", encoding="utf-8") as f:
            json.dump(res, f, indent=1, sort_keys=True)
            f.write("\n")
    if a.out_md:
        with open(a.out_md, "w", encoding="utf-8") as f:
            f.write(render_md(res))
    nd = sum(1 for r in res["rows"] if r["status"] == "DROP" and not r["evidence_only"])
    no = sum(1 for r in res["rows"] if r["status"] != "DROP" and not r["evidence_only"])
    print("LICENCE_AUDIT rows_ok=%d rows_drop=%d" % (no, nd))
    for r in res["rows"]:
        if r["status"] == "DROP" and not r["evidence_only"]:
            print("  DROP %s (%d files)" % (r["id"], r["n_files"]))
    if a.check_export:
        return 1 if check_export(res, a.check_export) else 0
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
