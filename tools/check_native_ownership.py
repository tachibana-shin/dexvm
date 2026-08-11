#!/usr/bin/env python3
"""Check native table descriptors against their owning module.

This is intentionally conservative: aggregate Java/Kotlin tables are allowed,
while Android and extension-specific tables must not silently absorb unrelated
packages.  It is a guardrail for future bridge additions.
"""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1] / "src/vm/native"

FORBIDDEN = {
    "android.rs": {
        "Leu/kanade/",
        "Lkotlinx/",
        "Ljava/",
        "Landroidx/",
    },
    "jsoup.rs": {"Leu/kanade/"},
    "okhttp.rs": {"Leu/kanade/"},
    "serialization.rs": {"Lcom/squareup/"},
    "kotlin.rs": {"Ljava/net/", "Lkotlinx/"},
}

errors = []
for path in ROOT.rglob("*.rs"):
    rules = FORBIDDEN.get(path.name)
    if not rules:
        continue
    text = path.read_text(encoding="utf-8")
    for descriptor in re.findall(r'ne!\(\s*"(L[^";]+;)', text):
        if any(descriptor.startswith(prefix) for prefix in rules):
            errors.append(f"{path.relative_to(ROOT)}: {descriptor}")

if errors:
    print("native ownership violations:")
    print("\n".join(f"- {item}" for item in errors))
    sys.exit(1)
print("native ownership OK")
