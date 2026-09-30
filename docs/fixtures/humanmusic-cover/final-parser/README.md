# Final symbolic parser gate

**Observed:** 7 tests executed, process exit 0; pinned packages exactly match `requirements-symbolic.txt`: True. Source boundary `586d74716c63e705ca0b1d08068c10d1b7a7352a`; 18 parser/source/fixture files match that commit and stayed byte-identical during the run (0 pre-run differences; 0 run-time changes). Unexecuted holdout preparation was present and is disclosed in `receipt.json`.

Command, UTC times, environment, interpreter identity and raw-output SHA256 are retained in `receipt.json` and `environment.json`.

```sh
target/humanmusic-consolidation/ingest-venv/bin/python -m unittest discover -s scripts/audio -p test_import_symbolic.py -v
```

All 11 historical receipt/provenance hashes matched: the importer, six Ode receipts and four downloaded source artifacts. `source-manifest-before.json` and `source-manifest-after.json` also cover the partial Swing chart and provenance. This rechecks byte custody; it does not independently verify the source assertions.

`output.txt` removes trailing whitespace for repository display; `output.raw.gz` retains the exact combined stdout/stderr bytes. Unit controls cover admitted pitch/rhythm/tie/rest syntax, unsupported syntax, missing and changing metadata, pitch/rhythm falsifiers, MIDI zero-velocity note-offs, delta accumulation, unterminated notes and asynchronous MIDI rejection.

No parser extraction was rerun, no music was generated, no Cargo or holdout command ran, and no audio or listening result is claimed. These are development parser controls, not fresh musical holdout cases.
