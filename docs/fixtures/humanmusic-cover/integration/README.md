# Integrated cover controls

Log custody: `raw/*.txt.gz` retain exact original stdout/stderr bytes. `raw-log-sha256.json` hashes their decompressed bytes. Adjacent `.txt` displays only strip trailing whitespace/extra final blank lines for the repository whitespace gate; no result or failure text is removed.
