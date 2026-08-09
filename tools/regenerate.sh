#!/usr/bin/env bash
# Regenerate the per-class native shim tree from git HEAD and verify it.
#
# The split scripts (split_lang.py / split_util.py / split_remaining.py) emit
# the per-class leaf files. The package mod files (native/mod.rs, java/mod.rs,
# java/lang/mod.rs, java/util/mod.rs, java/text/mod.rs,
# java/util/regex/mod.rs) are hand-maintained; this script copies them over
# the regenerated output so the result is a working tree.
#
# Usage:
#   tools/regenerate.sh OUT_DIR     regenerate into OUT_DIR
#   tools/regenerate.sh --check     regenerate into a temp dir, compare every
#                                   ne!() entry and pub(crate)/pub fn name
#                                   against the current tree, then
#                                   cargo check --features keiyoushi on a
#                                   detached worktree overlaid with the
#                                   regenerated files.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

GLUE="src/vm/native/mod.rs src/vm/native/java/mod.rs src/vm/native/java/lang/mod.rs src/vm/native/java/util/mod.rs src/vm/native/java/text/mod.rs src/vm/native/java/util/regex/mod.rs"

if [ "${1:-}" = "--check" ]; then
    OUT="$(mktemp -d)"
    WT="$(mktemp -d)"
    trap 'rm -rf "$OUT" "$WT"' EXIT
else
    OUT="${1:?usage: tools/regenerate.sh OUT_DIR | tools/regenerate.sh --check}"
fi

OUT="$OUT" python3 tools/split_lang.py
OUT="$OUT" python3 tools/split_util.py
OUT="$OUT" python3 tools/split_remaining.py

for f in $GLUE; do
    rel="${f#src/vm/native/}"
    mkdir -p "$(dirname "$OUT/$rel")"
    cp "$REPO/$f" "$OUT/$rel"
done

if [ "${1:-}" = "--check" ]; then
    # --- semantic diff: entries + fn names, ignoring header/layout noise ---
    python3 - "$OUT" <<'PYEOF'
import re, sys
from pathlib import Path

def sigs(root):
    entries, fns = {}, set()
    for p in Path(root).rglob('*.rs'):
        text = p.read_text()
        for m in re.finditer(r'ne!\("([^"]+;)"\s*,\s*"([^"]+)"\s*,\s*"([^"]+)"', text):
            key = (m.group(1), m.group(2), m.group(3))
            entries.setdefault(key, set()).add(str(p.relative_to(root)))
        for m in re.finditer(r'^pub(?:\(crate\))? fn (\w+)', text, re.M):
            fns.add(m.group(1))
    return entries, fns

gen, gen_fns = sigs(sys.argv[1])
cur, cur_fns = sigs('src/vm/native')

missing = {k: v for k, v in cur.items() if k not in gen}
extra = {k: v for k, v in gen.items() if k not in cur}
if missing or extra:
    for k in sorted(missing): print(f'MISSING {k}  {sorted(missing[k])}')
    for k in sorted(extra): print(f'EXTRA   {k}  {sorted(extra[k])}')
    sys.exit(1)
fn_only = gen_fns - cur_fns
if fn_only:
    print('unmatched regenerated fns:', sorted(fn_only))
    sys.exit(1)
print(f'entries: {len(cur)} identical; fns: {len(gen_fns)} generated')
PYEOF

    # --- compile check: overlay regenerated files on a detached worktree ---
    git worktree add --detach "$WT" HEAD >/dev/null
    cp -r "$OUT/java" "$WT/src/vm/native/java"
    for f in $GLUE; do
        cp "$OUT/${f#src/vm/native/}" "$WT/$f"
    done
    for f in okhttp.rs kotlin.rs keiyoushi.rs jsoup.rs android.rs injekt.rs; do
        cp "$OUT/$f" "$WT/src/vm/native/$f"
    done
    # vm/mod.rs references native::native_tables() since the refactor
    cp "$REPO/src/vm/mod.rs" "$WT/src/vm/mod.rs"
    if grep -q 'class.rs' "$REPO/src/context.rs"; then cp "$REPO/src/context.rs" "$WT/src/context.rs"; fi
    cd "$WT"
    LOG="$(mktemp)"
    if ! cargo check --features keiyoushi >"$LOG" 2>&1; then
        grep -B1 -A6 'error' "$LOG" | head -80
        echo "regeneration check FAILED (compile errors above)"
        exit 1
    fi
    echo "regeneration check OK"
fi
