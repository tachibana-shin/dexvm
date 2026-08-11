# tools/

Scripts used to refactor `src/vm/native/` from a few big files with monolithic
native tables into one file per Java class, each registering its own
`TABLE` (referred to as the "per-class split").

## Layout after the split

- `src/vm/native/java/<pkg>/<Class>.rs` — host shims + `pub(crate) const TABLE`
  for each Java class (e.g. `java/lang/string.rs` for `Ljava/lang/String;`).
- `src/vm/native/okhttp/mod.rs` (okhttp3), `jsoup/mod.rs`, `android/mod.rs`,
  `keiyoushi.rs` (eu.kanade only), `kotlin/mod.rs`, `injekt/mod.rs` — library shims,
  each with its own table.
- Package mod files are **hand-maintained** (not generated): `native/mod.rs`
  (macro definitions, `register()`, `native_tables()`, helpers),
  `java/mod.rs`, `java/lang/mod.rs`, `java/util/mod.rs` (also holds the shared
  `coll_elems` / `list_alloc` / `set_alloc` helpers), `java/text/mod.rs`,
  `java/util/regex/mod.rs`.

## Regenerating the leaf files

The split scripts read their sources from the **pre-refactor baseline commit**
(the parent of the commit that deleted the monolithic tables, i.e. `HEAD^` of
the refactor commit — auto-detected, overridable with `DEXVM_BASELINE`), so
they keep working even though `HEAD` now contains the refactored tree:

```sh
tools/regenerate.sh OUT_DIR          # regenerate into OUT_DIR
tools/regenerate.sh --check          # semantic diff + cargo check --features keiyoushi
```

`--check` compares every `ne!(...)` entry (class/method/sig) and fn name
between the regenerated tree and the current tree, then compiles the result
on a detached worktree. Entry counts must match exactly (838 entries, minus
the two known dropped duplicates):

- `Lokhttp3/Request$Builder;.build` — the keiyoushi-side entry wins
- `HttpSource.getHeaders` — the `http_source_get_headers_default` impl wins

## Verifying the live tree

```sh
python3 tools/verify_native_tables.py
```

Checks the current tree directly: no duplicate table keys, every `ne!()`
target fn is defined (including field-macro-generated fns such as
`sm_get_field!` / `sc_get_field!`), and full entry coverage vs the split
baseline. New API entries are reported for visibility but are allowed.

## Auditing every Keiyoushi extension APK

`keiyoushi_audit.rb` downloads the current v2 Keiyoushi repository index and
all selected APKs, caches them under `target/`, asks `dexcli --api-coverage`
to inspect every `classes*.dex`, and ranks missing Rust bridges by the number
of affected APKs and call sites. It uses the full `index.json`; the legacy
`index.min.json` now only contains upgrade notices.

Build the analyzer once, then run the complete audit:

```sh
cargo build --features keiyoushi --bin dexcli
ruby tools/keiyoushi_audit.rb
```

Reports are written to `target/keiyoushi-audit/report.yaml`, a `report.md`
summary, and a very small `report.min.yaml`. Downloads
are cached and assigned a local SHA-256 digest, so reruns only analyze them.
Useful smaller runs:

```sh
# One language, eight concurrent workers
ruby tools/keiyoushi_audit.rb --language vi --jobs 8

# Name/package filter and a small sample
ruby tools/keiyoushi_audit.rb --match 'manga|comic' --limit 25

# Analyze APKs already on disk without network access
ruby tools/keiyoushi_audit.rb --local fixtures --offline

# Validate cached bytes before reuse
ruby tools/keiyoushi_audit.rb --verify-cache

# Skip Markdown, or explicitly request compact JSON as well
ruby tools/keiyoushi_audit.rb --no-markdown --json target/keiyoushi-audit/report.json
```

The YAML report retains per-extension results without repeating full gap
records: each gap is stored once under a compact ID and extensions contain only
an ID-to-use-count map. Optional JSON uses the same normalized schema and is
written compactly. The Markdown report defaults to the 250 highest-impact gaps
and maps each signature to the likely Rust bridge source file. Use `--top` to
change that limit. `report.min.yaml` contains only missing API signatures grouped
by kind; each value is `[affected APKs, calls]`, with no origin or bridge-file
metadata. Use `--minified PATH` to change its location. A failed download or
malformed APK is reported without discarding successful results; the process
exits with status 2 when any such partial failure occurred.

## Split history

1. `split_lang.py` — `java/lang/*` (from HEAD `lang.rs`, `string.rs`,
   `math.rs`, `io.rs` lazies) + `java/io.rs` (PrintStream) + the
   `kotlin_stringskt_append.rs` scratch file.
2. `split_util.py` — `java/util/*`, `java/text/*`, `java/nio.rs`, and tops up
   `java/io.rs` with `ps_init` / `lazy_print_stream` (from HEAD `collections.rs`,
   `sync.rs`, `regex.rs`, `io.rs`, `text.rs`).
3. `split_remaining.py` — routes `KEIYOUSHI_TABLE` by class into
   `keiyoushi.rs` / `okhttp.rs` / `jsoup.rs` / `android.rs` / `kotlin.rs`,
   moves the `NATIVE_TABLE` kotlin + injekt entries, and dedupes overlapping
   entries.

Run them in order with `OUT=<dir>` to target a directory other than
`src/vm/native` (used by `regenerate.sh`).
