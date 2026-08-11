# AGENTS.md

`dexvm`: a DEX (Dalvik bytecode) interpreter in Rust, tuned for running mihon/Tachiyomi (keiyoushi) manga extensions. Embeddable library; the extension's own dex bytecode does all source/parse work — host shims only provide `java.*`/`kotlin.*`/`okhttp3.*`/`org.jsoup.*`/`eu.kanade.tachiyomi.*` APIs.

## Build & test

- The meaningful test suite needs the `keiyoushi` feature (not in default): **`cargo test --features keiyoushi`**. Plain `cargo test` is a weak smoke.
- CI gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test` (default / `--no-default-features` / `--all-features`), `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS="-D warnings"`. Run all of these before finishing; clippy takes standard `-D warnings` seriously.
- VM instrumentation: `RUST_LOG=info cargo test --features keiyoushi -- --nocapture` shows `DBG`/`ERR`/`INV` traces from `src/vm/interpret.rs` (tests that call `tests/common/mod.rs::init_logger`).
- `tests/zz_probe.rs` is a scratch repro test (WIP, uncommitted). Add quick repro probes there; don't commit it.

## Test layout

- `tests/keiyoushi_flow.rs` — offline replay of `fixtures/live/` captures (skips if capture missing/BLOCKED). `DEXVM_APK` env var points the same flow suite at another fixture apk (comicfury, mangapill, moetruyen).
- `tests/live_keiyoushi.rs` — live capture against `api.akuma.moe`; **gated on `DEXVM_LIVE=1`** and repopulates `fixtures/live/` (writes a `BLOCKED` marker when the site's DDoS-Guard WAF answers). Same gating applies to `tests/live_moetruyen_net.rs`.
- `tests/moetruyen_*.rs` + `tests/live_moetruyen.rs` — moetruyen-specific (IMGX image decrypt); share `tests/common/mod.rs`, which holds the reference IMGX encryptor and fixture APK paths.
- Fixtures: real extension APKs in `fixtures/tachiyomi-*.apk`, the vm fixture under test is `fixtures/classes.dex`, capture store `fixtures/live/`, `fixtures/multidex.apk` for multidex.

## Architecture

- The VM never opens sockets: the host supplies HTTP via `Keiyoushi::set_http(_rc)`; requests are captured as `HttpData` and handed to the callback (real HTTP, fixture replay, or recording).
- `src/vm/mod.rs` — `Vm` core (classes, interning, arena, method/field resolution, invoke-target resolution). `src/vm/interpret.rs` — the interpreter loop. `src/vm/object.rs` — `Native` enum + garbage-collected arena. `src/vm/class.rs` — class loading/shim registration.
- Native shims are per-class `TABLE` constants, one file per Java class (`src/vm/native/<family>/...`). The leaf files were split out by `tools/regenerate.sh` from a baseline commit (`DEXVM_BASELINE` overrides auto-detection); package mod files (`src/vm/native/{mod,java,lang,util,text/regex}.rs`) are hand-maintained. See `tools/README.md` before touching those tables.
- `src/keiyoushi.rs` — typed bridge (`sources`, `popular`, `search`, `manga_details`, `chapters`, `pages`, `filters`); the `*_coro` variants replay dex suspend-coroutine continuations (`suspend_cont`).
- `src/bin/dexcli.rs` — debugging CLI over `fixtures/*.apk`: `--classes`, `--methods <class>`, `--code <class> <method>` (dumps all overloads), `--run`, `--call`. Fast way to inspect extension bytecode (e.g. probe a native call site before instrumenting the VM).
- Sandbox: everything denied by default; grant via `SandboxOptions` or `Context::grant`; host natives must call `vm.check_permission`, not just perform side effects.