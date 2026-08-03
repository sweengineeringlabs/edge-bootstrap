#!/usr/bin/env python3
"""Fail if this workspace's own Cargo.toml files pin the same git dependency
package to more than one tag/rev.

Scope is deliberately narrow: only dependency declarations found in this
repo's own Cargo.toml files are compared against each other. Transitive
duplicates introduced by a third-party crate's own internal pins (e.g. one
upstream dep still on an old edge-domain tag while another has moved on) are
out of our control and are not flagged -- this only catches the mistake of
forgetting to update one of several copies of the same pin when bumping a
dependency across a multi-crate workspace.
"""
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Matches a single-line `key = { ... git = "URL" ... }` dependency entry.
DEP_LINE_RE = re.compile(
    r'^([A-Za-z0-9_-]+)\s*=\s*\{([^}]*git\s*=\s*"[^"]+"[^}]*)\}',
)
GIT_RE = re.compile(r'git\s*=\s*"([^"]+)"')
TAG_RE = re.compile(r'\btag\s*=\s*"([^"]+)"')
REV_RE = re.compile(r'\brev\s*=\s*"([^"]+)"')
PACKAGE_RE = re.compile(r'\bpackage\s*=\s*"([^"]+)"')


def find_cargo_tomls(root: Path):
    return sorted(
        p
        for p in root.rglob("Cargo.toml")
        if "target" not in p.parts
    )


def parse_git_deps(path: Path):
    entries = []
    for line in path.read_text(encoding="utf-8").splitlines():
        m = DEP_LINE_RE.match(line.strip())
        if not m:
            continue
        key, body = m.group(1), m.group(2)
        git_m = GIT_RE.search(body)
        if not git_m:
            continue
        git_url = git_m.group(1)
        package_m = PACKAGE_RE.search(body)
        package = package_m.group(1) if package_m else key
        tag_m = TAG_RE.search(body)
        rev_m = REV_RE.search(body)
        ref_kind = "tag" if tag_m else "rev" if rev_m else None
        ref_val = tag_m.group(1) if tag_m else rev_m.group(1) if rev_m else None
        if ref_kind is None:
            continue
        entries.append((package, git_url, ref_kind, ref_val, path))
    return entries


def main():
    tomls = find_cargo_tomls(REPO_ROOT)
    by_package_url = {}
    for path in tomls:
        for package, git_url, ref_kind, ref_val, src in parse_git_deps(path):
            by_package_url.setdefault((package, git_url), set()).add(
                (ref_kind, ref_val, str(src.relative_to(REPO_ROOT)))
            )

    conflicts = {
        key: refs for key, refs in by_package_url.items() if len({(k, v) for k, v, _ in refs}) > 1
    }

    if not conflicts:
        print(f"deps consistency: OK ({len(tomls)} Cargo.toml files, {len(by_package_url)} distinct git deps)")
        return 0

    print("deps consistency: FAIL -- same package pinned to different tag/rev across this workspace\n")
    for (package, git_url), refs in sorted(conflicts.items()):
        print(f"  {package}  ({git_url})")
        for ref_kind, ref_val, src in sorted(refs, key=lambda r: r[2]):
            print(f"    {ref_kind}={ref_val:<40} <- {src}")
        print()
    return 1


if __name__ == "__main__":
    sys.exit(main())
