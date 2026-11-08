#!/usr/bin/env python3
"""Canonical forms for check-maintenance-candidate.sh (read stdin, write stdout).

  header    stdin: a C/C++ header already stripped of comments by `cc -fpreprocessed -dD -E -P`.
            stdout: the same token stream with whitespace runs outside string and character
            literals collapsed to one space; literals are kept byte for byte. Exit 3 if the text
            has a C++ raw string literal (not normalized; the caller fails closed).
  rust-abi  stdin: the concatenated Rust sources of src/ (each file preceded by a line
            `//@file <path>`). stdout: one sorted line per C-ABI-relevant item, comments and
            layout whitespace removed:
              export <name>                  every gibson_* symbol the sources define
              fn <path> <attrs> <name><generics>(<params>) -> <ret>   every extern "C" fn
                                             (attrs: every non-doc attribute, e.g. no_mangle, cfg)
              alias / use <path>: ...        type aliases and imports of every file that exports
              mod <attrs> <name>             src/lib.rs declarations of the modules that export
            (fn, alias and use lines are recorded for files that contain no_mangle/export_name)
              static <attrs> <name>: <type>  every static gibson_*
              macro <name> { <body> }        every macro_rules! whose body defines an extern "C" fn
              invoke <macro>!(<args>)        every invocation of such a macro
              type <path>: #[repr(..)] <struct|enum|union> <Name> <body>
              const GIBSON_<NAME>: <type> = <value>
            Two revisions with equal output export the same names with the same signatures,
            struct/enum layouts and discriminants. Exit 3 on a lexing error.
"""

import re
import sys


RUST_CHAR = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F]{1,6}\}|x[0-9a-fA-F]{2}|.)|[^\\'\n])'")


def collapse_outside_literals(text, raw_strings="c"):
    out, i, n, pending_space = [], 0, len(text), False
    while i < n:
        ch = text[i]
        if ch.isspace():
            pending_space = True
            i += 1
            continue
        if pending_space and out:
            out.append(" ")
        pending_space = False
        if ch == "'" and raw_strings == "rust":
            m = RUST_CHAR.match(text, i)
            if not m:  # a lifetime ('a, 'static), not a literal
                out.append(ch)
                i += 1
                continue
            out.append(m.group(0))
            i = m.end()
            continue
        if ch in "\"'":
            if raw_strings == "c" and ch == '"' and i > 0 and text[i - 1] == "R":
                raise ValueError("raw string literal")
            j = i + 1
            while j < n and text[j] != ch:
                if text[j] == "\\":
                    j += 1
                elif text[j] == "\n":
                    raise ValueError("unterminated literal")
                j += 1
            if j >= n:
                raise ValueError("unterminated literal")
            out.append(text[i : j + 1])
            i = j + 1
            continue
        out.append(ch)
        i += 1
    return "".join(out).strip()


RUST_SPECIAL = re.compile(r'//|/\*|(?<![A-Za-z0-9_])b?r#*"|"|\'')
RUST_STRING = re.compile(r'"(?:\\.|[^"\\])*"', re.S)


def strip_rust_comments(src):
    """Remove // and (nested) /* */ comments; keep string, raw string and char literals."""
    out, i, n = [], 0, len(src)
    while i < n:
        m = RUST_SPECIAL.search(src, i)
        if not m:
            out.append(src[i:])
            break
        out.append(src[i : m.start()])
        tok, j = m.group(0), m.start()
        if tok == "//":
            k = src.find("\n", j)
            i = n if k < 0 else k
        elif tok == "/*":
            depth, k = 1, j + 2
            while depth:
                o, c = src.find("/*", k), src.find("*/", k)
                if c < 0:
                    raise ValueError("unterminated block comment")
                if 0 <= o < c:
                    depth, k = depth + 1, o + 2
                else:
                    depth, k = depth - 1, c + 2
            out.append(" ")
            i = k
        elif tok.endswith('"') and "r" in tok:
            hashes = tok[tok.index("r") + 1 : -1]
            end = src.find('"' + hashes, m.end())
            if end < 0:
                raise ValueError("unterminated raw string")
            out.append(src[j : end + 1 + len(hashes)])
            i = end + 1 + len(hashes)
        elif tok == '"':
            sm = RUST_STRING.match(src, j)
            if not sm:
                raise ValueError("unterminated string")
            out.append(sm.group(0))
            i = sm.end()
        else:  # "'": a char literal, or a lifetime
            cm = RUST_CHAR.match(src, j)
            if cm:
                out.append(cm.group(0))
                i = cm.end()
            else:
                out.append("'")
                i = j + 1
    return "".join(out)


def balanced(text, start, open_ch, close_ch):
    """Index just past the bracket matching text[start] == open_ch (string-literal aware)."""
    depth, i, n = 0, start, len(text)
    while i < n:
        ch = text[i]
        if ch == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            i = j + 1
            continue
        if ch == open_ch:
            depth += 1
        elif ch == close_ch:
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError("unbalanced %s" % open_ch)


def norm(s):
    return collapse_outside_literals(s, raw_strings="rust")


def attrs_before(text, pos):
    """The #[...] attributes immediately preceding pos (whitespace and `pub`/`unsafe` skipped)."""
    head = text[max(0, pos - 4096) : pos]  # attributes sit right before the item
    attrs = []
    while True:
        head = head.rstrip()
        m = re.search(r"(pub(\s*\([^)]*\))?|unsafe|extern\s*\"C\"|const)$", head)
        if m:
            head = head[: m.start()]
            continue
        if head.endswith("]"):
            depth, k = 0, len(head) - 1
            while k >= 0:
                if head[k] == "]":
                    depth += 1
                elif head[k] == "[":
                    depth -= 1
                    if depth == 0:
                        break
                k -= 1
            if k > 0 and head[k - 1] == "#":
                attrs.append(norm(head[k - 1 :]))
                head = head[: k - 1]
                continue
        return attrs


def strip_test_modules(text):
    """Drop `#[cfg(test)] mod name { ... }` bodies: test code is not part of the shipped ABI."""
    out, i = [], 0
    pat = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(pub(\s*\([^)]*\))?\s+)?mod\s+[A-Za-z0-9_]+\s*\{")
    for m in pat.finditer(text):
        if m.start() < i:
            continue
        out.append(text[i : m.start()])
        i = balanced(text, m.end() - 1, "{", "}")
    out.append(text[i:])
    return "".join(out)


def rust_abi(stdin):
    records, files = set(), re.split(r"^//@file (.*)$", stdin, flags=re.M)
    sources = list(zip(files[1::2], files[2::2]))
    fn_macros = set()
    stripped = [(path, strip_test_modules(strip_rust_comments(src))) for path, src in sources]
    # Files that define exported symbols: their extern "C" fns, aliases and imports are recorded,
    # and so is the src/lib.rs declaration of their module.
    export_files = {path for path, text in stripped if re.search(r"\bno_mangle\b|\bexport_name\b", text)}
    export_mods = set()
    for path in export_files:
        m = re.match(r"src/([A-Za-z0-9_]+)(?:\.rs|/mod\.rs)$", path)
        if m:
            export_mods.add(m.group(1))
    for path, text in stripped:
        for m in re.finditer(r"macro_rules!\s*([A-Za-z0-9_]+)\s*\{", text):
            end = balanced(text, m.end() - 1, "{", "}")
            body = text[m.end() - 1 : end]
            if re.search(r'extern\s*"C"\s*fn', body):
                fn_macros.add(m.group(1))
                records.add("macro %s %s" % (m.group(1), norm(body)))
        exports_here = path in export_files
        for m in re.finditer(r'extern\s*"C"\s*fn\s+([A-Za-z0-9_$]+)\s*(<[^>{(]*>)?\s*\(', text):
            close = balanced(text, m.end() - 1, "(", ")")
            rest = text[close:]
            ret = re.match(r"\s*(->\s*[^{;]*?)?\s*(where\b[^{;]*)?[{;]", rest)
            if not ret:
                raise ValueError("cannot parse signature of " + m.group(1))
            attrs = [a for a in attrs_before(text, m.start()) if not re.match(r"#\s*\[\s*doc\b", a)]
            name = m.group(1)
            if not exports_here:
                continue
            records.add("fn %s %s %s%s %s %s" % (path, " ".join(sorted(attrs)), name, norm(m.group(2) or ""),
                                               norm(text[m.end() - 1 : close]),
                                               norm((ret.group(1) or "") + " " + (ret.group(2) or ""))))
            if name.startswith("gibson_") and any("no_mangle" in a for a in attrs):
                records.add("export " + name)
        if exports_here:
            for m in re.finditer(r"(?<![A-Za-z0-9_])type\s+([A-Za-z0-9_]+)\s*(<[^=;]*>)?\s*=\s*([^;]+);", text):
                records.add("alias %s: %s%s = %s" % (path, m.group(1), norm(m.group(2) or ""), norm(m.group(3))))
            for m in re.finditer(r"(?<![A-Za-z0-9_])use\s+([^;]+);", text):
                records.add("use %s: %s" % (path, norm(m.group(1))))
        if path == "src/lib.rs":
            for m in re.finditer(r"(?<![A-Za-z0-9_])mod\s+([A-Za-z0-9_]+)\s*;", text):
                if m.group(1) not in export_mods:
                    continue
                attrs = [a for a in attrs_before(text, m.start()) if not re.match(r"#\s*\[\s*doc\b", a)]
                records.add("mod %s %s" % (" ".join(sorted(attrs)), m.group(1)))
        for m in re.finditer(r"static\s+(mut\s+)?(gibson_[A-Za-z0-9_]+)\s*:\s*([^=;]+)", text):
            attrs = [a for a in attrs_before(text, m.start()) if "no_mangle" in a or "export_name" in a]
            records.add("static %s %s%s: %s" % (" ".join(attrs), m.group(1) or "", m.group(2), norm(m.group(3))))
            if attrs:
                records.add("export " + m.group(2))
        for m in re.finditer(r"\bconst\s+(GIBSON_[A-Z0-9_]+)\s*:\s*([^=;]+)=\s*([^;]+);", text):
            records.add("const %s: %s = %s" % (m.group(1), norm(m.group(2)), norm(m.group(3))))
        for m in re.finditer(r"#\s*\[\s*export_name\s*=\s*\"(gibson_[A-Za-z0-9_]+)\"", text):
            records.add("export " + m.group(1))
        for m in re.finditer(r"#\s*\[\s*repr\s*\(([^)]*)\)\s*\]", text):
            tail = text[m.end():]
            t = re.match(r"(\s*#\s*\[[^\]]*\])*\s*(pub(\s*\([^)]*\))?\s+)?(struct|enum|union)\s+([A-Za-z0-9_]+)\s*(<[^>{(]*>)?\s*", tail)
            if not t:
                continue
            start = m.end() + t.end()
            if start < len(text) and text[start] == "{":
                body = text[start : balanced(text, start, "{", "}")]
            elif start < len(text) and text[start] == "(":
                body = text[start : balanced(text, start, "(", ")")]
            else:
                body = ";"
            records.add("type %s: #[repr(%s)] %s %s%s %s" % (path, norm(m.group(1)), t.group(4), t.group(5),
                                                         norm(t.group(6) or ""), norm(body)))
    for path, text in stripped:
        for mac in fn_macros:
            for m in re.finditer(r"(?<![A-Za-z0-9_])%s!\s*\(" % re.escape(mac), text):
                args = text[m.end() - 1 : balanced(text, m.end() - 1, "(", ")")]
                records.add("invoke %s!%s" % (mac, norm(args)))
                first = re.match(r"\(\s*(gibson_[A-Za-z0-9_]+)", args)
                if first:
                    records.add("export " + first.group(1))
    return "\n".join(sorted(records)) + "\n"


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    data = sys.stdin.read()
    try:
        if mode == "header":
            sys.stdout.write(collapse_outside_literals(data) + "\n")
        elif mode == "rust-abi":
            sys.stdout.write(rust_abi(data))
        else:
            sys.stderr.write("usage: maintenance_surface.py header|rust-abi < input\n")
            return 2
    except ValueError as e:
        sys.stderr.write("maintenance_surface: %s\n" % e)
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
