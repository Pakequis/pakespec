#!/usr/bin/env python3
"""Generate THIRD-PARTY-NOTICES.md from the dependency tree of the release binaries.

Lists every crate (name, version, license expression, repository) and appends
the distinct license texts found inside the crate sources. Crates that keep their
license file only at a workspace root are listed with their SPDX expression.

Usage: python3 tools/gen_third_party_notices.py [output]
"""
import hashlib
import json
import os
import subprocess
import sys

LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE")


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "THIRD-PARTY-NOTICES.md"
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked"], text=True))
    # Keep only crates reachable from normal (non-dev, non-build-only) dependencies.
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    seen, stack = set(), [meta["resolve"]["root"]]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for d in nodes[i]["deps"]:
            if any(k["kind"] in (None, "build") for k in d["dep_kinds"]):
                stack.append(d["pkg"])
    pkgs = sorted((p for p in meta["packages"] if p["id"] in seen and p["id"] != meta["resolve"]["root"]),
                  key=lambda p: (p["name"], p["version"]))

    texts = {}  # sha -> (text, [crate names])
    rows = []
    for p in pkgs:
        d = os.path.dirname(p["manifest_path"])
        files = [f for f in sorted(os.listdir(d)) if f.upper().startswith(LICENSE_PREFIXES)]
        # Bundled font files carry their own license texts next to them.
        fonts = os.path.join(d, "fonts")
        if os.path.isdir(fonts):
            files += [os.path.join("fonts", f) for f in sorted(os.listdir(fonts)) if f.endswith(".txt")]
        for f in files:
            if os.path.isfile(os.path.join(d, f)):
                t = open(os.path.join(d, f), encoding="utf-8", errors="replace").read().strip()
                texts.setdefault(hashlib.sha256(t.encode()).hexdigest(), (t, []))[1].append(p["name"])
        rows.append(f"- {p['name']} {p['version']}: {p['license'] or 'see crate'}"
                    + (f" ({p['repository']})" if p["repository"] else ""))

    with open(out, "w", encoding="utf-8") as o:
        o.write("# Third-party notices\n\nPakespec is MIT licensed. The binaries include the "
                "following third-party crates.\n\n## Crates\n\n" + "\n".join(rows) + "\n\n## License texts\n")
        for t, names in sorted(texts.values(), key=lambda v: v[0]):
            names = sorted(set(names))
            o.write("\n---\n\nUsed by: " + ", ".join(names) + "\n\n```text\n" + t + "\n```\n")
    print(f"{len(pkgs)} crates, {len(texts)} distinct license texts -> {out}")


if __name__ == "__main__":
    main()
