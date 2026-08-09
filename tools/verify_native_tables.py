#!/usr/bin/env python3
"""Verify the integrity of the per-class native shim tree.

Checks performed against src/vm/native/:
  1. No duplicate (class, method, signature) keys across all tables.
  2. Every fn referenced by a ne!() entry is defined somewhere in the tree
     (searching all leaf files; re-export chains via package mod.rs are not
     simulated, so table-only classes rely on sibling files defining them).
  3. Every entry from the pre-refactor tables (baseline commit) is still present
     under the same class, except the two known dropped duplicates
     (Request$Builder.build, HttpSource.getHeaders).
  4. Entry counts per class match the baseline commit exactly.

Usage: python3 tools/verify_native_tables.py
"""
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path('src/vm/native')

# The two entries dropped during the split (keiyoushi version wins).
DROPPED = {
    ('Lokhttp3/Request$Builder;', 'build', '()Lokhttp3/Request;'),
    ('Leu/kanade/tachiyomi/source/online/HttpSource;', 'getHeaders', '()Lokhttp3/Headers;'),
}

ENTRY_RE = re.compile(r'ne!\("([^"]+;)"\s*,\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,\s*\w+,\s*(\w+)\)')
FN_RE = re.compile(r'^(?:pub(?:\(crate\))?\s+)?fn\s+(\w+)', re.M)
# field macros (sm_get_field! / sm_set_field! / sc_get_field! / ...) define fns
MACRO_FN_RE = re.compile(r'^\w+!\s*\(\s*\w+,', re.M)


def parse_tree(root):
    """Return {entry_key: [files]} and the set of all defined fn names."""
    entries = {}
    fns = set()
    for p in sorted(root.rglob('*.rs')):
        text = p.read_text()
        for m in ENTRY_RE.finditer(text):
            entries.setdefault((m.group(1), m.group(2), m.group(3)), set()).add(str(p))
        fns.update(FN_RE.findall(text))
        for m in MACRO_FN_RE.finditer(text):
            fns.add(m.group(0).split('(', 1)[1].split(',', 1)[0].strip())
    return entries, fns


def git_base():
    """Last commit whose tree still has the pre-refactor monolith tables
    (parent of the commit that deleted src/vm/native/lang.rs)."""
    env = os.environ.get('DEXVM_BASELINE')
    if env:
        return env
    r = subprocess.run(
        ['git', 'log', 'HEAD', '--diff-filter=D', '--format=%H', '-1', '--', 'src/vm/native/lang.rs'],
        capture_output=True, text=True)
    if r.returncode == 0 and r.stdout.strip():
        r2 = subprocess.run(['git', 'rev-parse', r.stdout.strip() + '^'], capture_output=True, text=True)
        if r2.returncode == 0 and r2.stdout.strip():
            return r2.stdout.strip()
    return 'HEAD'


def head_entries():
    """Parse the pre-refactor tables from the baseline commit."""
    base = git_base()
    entries = {}
    for path in ('src/vm/native/mod.rs', 'src/vm/native/keiyoushi.rs'):
        r = subprocess.run(['git', 'show', f'{base}:{path}'], capture_output=True, text=True)
        if r.returncode != 0:
            print(f'cannot read git HEAD:{path}: {r.stderr.strip()}')
            sys.exit(1)
        for m in ENTRY_RE.finditer(r.stdout):
            # skip the throwable_ctors_table! macro body (4 placeholder entries)
            if m.group(4).startswith('ctor_'):
                continue
            entries.setdefault((m.group(1), m.group(2), m.group(3)), 0)
    return entries


def main():
    errors = 0
    cur, fns = parse_tree(ROOT)

    # 1. duplicate keys
    for key in sorted(cur):
        if len(cur[key]) > 1:
            print(f'DUPLICATE {key} in {sorted(cur[key])}')
            errors += 1

    # 2. referenced fns must exist
    for key, files in sorted(cur.items()):
        fn = key[0].replace('Ljava/', '').replace('L', '', 1).replace('/', '.')
        for f in files:
            text = Path(f).read_text()
            for m in ENTRY_RE.finditer(text):
                if (m.group(1), m.group(2), m.group(3)) == key and m.group(4) not in fns:
                    print(f'MISSING FN {m.group(4)} for {key} in {f}')
                    errors += 1

    # 3+4. coverage vs git HEAD
    head = head_entries()
    missing = [k for k in head if k not in DROPPED and k not in cur]
    extra = [k for k in cur if k not in head]
    if missing:
        print(f'{len(missing)} entries from the pre-refactor baseline are missing:')
        for k in missing[:10]:
            print('  ', k)
        errors += 1
    if extra:
        print(f'{len(extra)} new entries not in the baseline:')
        for k in extra[:10]:
            print('  ', k)
        errors += 1

    # per-class counts
    head_by_class = {}
    for cls, m, sig in head:
        head_by_class[cls] = head_by_class.get(cls, 0) + 1
    cur_by_class = {}
    for cls, m, sig in cur:
        cur_by_class[cls] = cur_by_class.get(cls, 0) + 1
    for cls in sorted(set(head_by_class) | set(cur_by_class)):
        if cls not in {k[0] for k in DROPPED} and head_by_class.get(cls, 0) != cur_by_class.get(cls, 0):
            print(f'COUNT {cls}: BASE={head_by_class.get(cls, 0)} now={cur_by_class.get(cls, 0)}')
            errors += 1

    total = len(cur)
    print(f'{total} entries, {len(fns)} fns, {len(head)} baseline entries')
    if errors:
        print(f'FAILED: {errors} problem(s)')
        sys.exit(1)
    print('verify OK')


if __name__ == '__main__':
    main()
