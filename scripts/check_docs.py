#!/usr/bin/env python3
"""Check local documentation links and reachability from the docs index."""
from __future__ import annotations
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
IGNORED = {'.git', 'target', 'build', 'dist', '__pycache__', 'vendor', 'third-party-licenses'}
LINK = re.compile(r'\]\((<[^>]+>|[^\s)]+)(?:\s+[^)]*)?\)')
HTML_SOURCE = re.compile(r'\b(?:src|href)=[\"\x27]([^\"\x27]+)[\"\x27]')


def documents() -> list[Path]:
    paths = set(ROOT.glob('*.md'))
    for folder in ['docs', '.github', 'bindings', 'native', 'packaging', 'crates', 'assets', 'notebook', '.signpath']:
        base = ROOT / folder
        if base.is_dir():
            paths.update(base.rglob('*.md'))
    return sorted(path for path in paths if not any(part in IGNORED for part in path.relative_to(ROOT).parts)
                  and 'releases' not in path.relative_to(ROOT).parts)


def main() -> int:
    errors = []
    checked = 0
    docs_root = ROOT / 'docs'
    index = docs_root / 'README.md'
    pages = documents()
    docs_links = {path: set() for path in pages if path.is_relative_to(docs_root)}
    for path in pages:
        text = re.sub(r'^(```|~~~).*?^\1[^\n]*$', '', path.read_text(encoding='utf-8'), flags=re.M | re.S)
        targets = [match.group(1).strip('<>') for match in LINK.finditer(text)]
        targets += HTML_SOURCE.findall(text)
        for target in targets:
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc or not parsed.path or parsed.path.startswith('/'):
                continue
            local = (path.parent / unquote(parsed.path)).resolve()
            checked += 1
            if not local.is_relative_to(ROOT.resolve()) or not local.exists():
                errors.append(f'{path.relative_to(ROOT)}: {target}')
            elif path in docs_links and local in docs_links:
                docs_links[path].add(local)
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        print(f'FAIL: {len(errors)} broken local links', file=sys.stderr)
        return 1
    if index not in docs_links:
        print('FAIL: missing docs/README.md', file=sys.stderr)
        return 1
    reachable = set()
    pending = [index]
    while pending:
        path = pending.pop()
        if path in reachable:
            continue
        reachable.add(path)
        pending.extend(docs_links[path] - reachable)
    unindexed = sorted(docs_links.keys() - reachable)
    if unindexed:
        for path in unindexed:
            print(f'Unreachable from docs/README.md: {path.relative_to(ROOT)}', file=sys.stderr)
        return 1
    print(f'PASS: {len(pages)} documents, {checked} local links, {len(reachable) - 1} indexed docs pages')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
