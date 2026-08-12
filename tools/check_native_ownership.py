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
    "android/mod.rs": {
        "Leu/kanade/",
        "Lkotlinx/",
        "Ljava/",
        "Landroidx/",
    },
    "jsoup/mod.rs": {"Leu/kanade/"},
    "okhttp/mod.rs": {"Leu/kanade/"},
    "serialization.rs": {"Lcom/squareup/"},
    "kotlin/mod.rs": {"Ljava/net/", "Lkotlinx/"},
}

ALLOWED_PREFIXES = {
    "kotlin/text.rs": ("Lkotlin/text/",),
    "kotlin/collections.rs": ("Lkotlin/collections/",),
    "kotlin/sequences.rs": ("Lkotlin/sequences/",),
    "kotlin/ranges.rs": ("Lkotlin/ranges/", "Lkotlin/internal/ProgressionUtilKt;", "Lkotlin/collections/IntIterator;"),
    "kotlin/tuples.rs": ("Lkotlin/Pair;", "Lkotlin/Triple;", "Lkotlin/TuplesKt;"),
    "kotlin/time.rs": ("Lkotlin/time/",),
    "kotlin/result.rs": ("Lkotlin/Result;", "Lkotlin/ResultKt;"),
    "kotlin/intrinsics.rs": ("Lkotlin/jvm/internal/Intrinsics;",),
    "kotlin/lazy.rs": ("Lkotlin/Lazy;", "Lkotlin/LazyKt;"),
    "kotlin/unsigned.rs": ("Lkotlin/UInt;", "Lkotlin/UByte;"),
    "kotlin/jvm.rs": ("Lkotlin/jvm/", "Lkotlin/coroutines/jvm/"),
    "kotlin/io.rs": ("Lkotlin/io/",),
}

errors = []
for path in ROOT.rglob("*.rs"):
    relative = str(path.relative_to(ROOT))
    rules = FORBIDDEN.get(relative)
    allowed = ALLOWED_PREFIXES.get(relative)
    if not rules and not allowed:
        continue
    text = path.read_text(encoding="utf-8")
    for descriptor in re.findall(r'ne!\(\s*"(L[^";]+;)', text):
        if rules and any(descriptor.startswith(prefix) for prefix in rules):
            errors.append(f"{path.relative_to(ROOT)}: {descriptor}")
    if allowed:
        for descriptor in re.findall(r'ne!\(\s*"(L[^";]+;)', text):
            if not descriptor.startswith(allowed):
                errors.append(f"{relative}: {descriptor} (outside {allowed})")

if errors:
    print("native ownership violations:")
    print("\n".join(f"- {item}" for item in errors))
    sys.exit(1)
print("native ownership OK")
