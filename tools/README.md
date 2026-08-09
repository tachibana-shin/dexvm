# tools/

Scripts used to refactor `src/vm/native/` from a few big files with monolithic
native tables into one file per Java class, each registering its own
`TABLE` (referred to as the "per-class split").

## Layout after the split

- `src/vm/native/java/<pkg>/<Class>.rs` — host shims + `pub(crate) const TABLE`
  for each Java class (e.g. `java/lang/string.rs` for `Ljava/lang/String;`).
- `src/vm/native/okhttp.rs` (okhttp3), `jsoup.rs`, `android.rs`,
  `keiyoushi.rs` (eu.kanade only), `kotlin.rs`, `injekt.rs` — library shims,
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
`sm_get_field!` / `sc_get_field!`), and full entry coverage vs git HEAD.

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
