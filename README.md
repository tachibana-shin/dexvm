# dexvm

[![Crates.io](https://img.shields.io/crates/v/dexvm)](https://crates.io/crates/dexvm)
[![docs.rs](https://img.shields.io/docsrs/dexvm)](https://docs.rs/dexvm)
[![License: Apache](https://img.shields.io/badge/license-Apache-green.svg)](#license)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/tachibana-shin/dexvm)

A minimal DEX (Dalvik bytecode) VM in Rust, embeddable as a library.

`dexvm` loads a plain `.dex` file or an `.apk`/`.zip` archive and executes the
bytecode directly with host-provided shims for the Java standard library — no
Android runtime, no JVM, no subprocess required. The core is a general-purpose
DEX interpreter tuned and extensively tested for one workload: running
**mihon/Tachiyomi manga extensions** (the keiyoushi ecosystem), for which it
ships a typed bridge plus full host shims for the extension API.

```
dex / apk ──► dexvm::Context ──► DEX interpreter
                │
                ├── java.* shims   (String, regex, text, collections, io)
                ├── kotlin.* / injekt shims
                ├── okhttp3.* shims (request building; never networked)
                ├── org.jsoup.* shims (DOM parsing via dom_query)
                ├── android.* stubs
                ├── eu.kanade.tachiyomi.* shims      [feature: tachiyomi]
                └── your own native tables          (per-context + global)
```

## Quick start

```rust,no_run
use dexvm::vm::value::JValue;
use dexvm::{Context, SandboxOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bare .dex files and .apk (dex-in-zip) containers both work.
    let data = std::fs::read("fixtures/classes.dex")?;
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all())?;

    // Instantiate a class and call instance methods.
    ctx.call("Lcom/example/App;", "<init>", &[JValue::Int(1)])?;
    let v = ctx.call("Lcom/example/App;", "greet", &[])?;
    println!("{v:?}");
    Ok(())
}
```

Every capability (network, filesystem, process, env) is denied by default and
throws unless granted via `SandboxOptions` or `Context::grant`.

## Running mihon / keiyoushi extensions

With default features you get the `tachiyomi` shims (`eu.kanade.tachiyomi.*`)
and the typed `keiyoushi` bridge:

```rust,no_run
use dexvm::keiyoushi::{HttpData, HttpResp, Keiyoushi, FilterState};

let mut ext = Keiyoushi::open("fixtures/tachiyomi-all.akuma-v1.4.10.apk")?;
// The VM never opens a socket by itself: you supply the HTTP callback.
ext.set_http(move |req: &HttpData| HttpResp {
    code: 200, message: "OK".into(), headers: Vec::new(), body: "<html/>".into(),
});

let srcs = ext.sources()?;
let page = ext.popular(&srcs[0], 1)?;
let filt = ext.filters(&srcs[0])?;
```

The extension's *own* dex code does all the work: base URLs, source names,
headers and parses are executed from bytecode, never hard-coded by the VM.
Outbound requests are captured as `HttpData` and handed to the callback you
registered, so you can perform real HTTP with any client (ureq/reqwest/...),
replay fixtures, or record traffic.

## Feature flags

Each host-shim family is its own feature — all enabled by default:

| feature      | provides host shims for …                                      |
|--------------|---------------------------------------------------------------|
| `java`       | `java.*` stdlib (String, regex, text, collections, io)        |
| `kotlin`     | `kotlin.*` stdlib                                             |
| `injekt`     | koan / injekt DI                                              |
| `jsoup`      | `org.jsoup.*` (DOM-style selection, backed by `dom_query`)     |
| `android`    | `android.*` stubs                                             |
| `okhttp`     | `okhttp3.*` request building (never networked)                |
| `tachiyomi`  | `eu.kanade.tachiyomi.*` mihon extension API                   |
| `keiyoushi`  | compat alias for the full mihon bundle + the typed bridge     |

`--no-default-features --features java` gives a lean VM with just the
standard library. `keiyoushi` expands to
`["tachiyomi", "kotlin", "injekt", "jsoup", "android", "okhttp"]`, so
`--features keiyoushi` is the full playbook. The mihon runtime is tuned for
this bundle; other combinations opt into only what you need.

## Registering your own native libraries

Private / custom libraries (mihon, your own host APIs) are registered
at runtime — per context or process-wide:

```rust,no_run
use dexvm::vm::native::{register_global, NativeEntry, NatErr};
use dexvm::vm::value::JValue;
use dexvm::vm::Vm;

fn ping(vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
    Ok(vm.alloc_string("pong"))
}
static PING: &[NativeEntry] = &[NativeEntry {
    class: "Lcom/example/host/Ping;",
    name: "ping",
    sig: "()Ljava/lang/String;",
    instance: false,
    f: ping,
}];

// process-wide: installed into every context created afterwards
register_global(PING);
// per-context: only `ctx` gets it
// ctx.register_natives(&[PING]);
```

Globally-registered tables are flattened into the dispatch of every context
created after registration, and the class is loaded on demand as a shim.
Natives get the full `Vm` (arena, interning, `check_permission`), so host
libraries can enforce the sandbox: call `vm.check_permission(&p)` before
side effects.

See `examples/host_native.rs` for a runnable version of this pattern.

## Sandbox

`Context` starts with everything denied; grants come from `SandboxOptions`
or `Context::grant`. Host natives check `vm.has_permission` /
`vm.check_permission` to turn actions off by default.

## Testing

- `cargo test` — generic VM tests
- `cargo test --features keiyoushi` — extension pipeline tests, offline
- `DEXVM_LIVE=1 cargo test --features keiyoushi --test live_keiyoushi` —
  live capture against `api.akuma.moe` (repopulates `fixtures/live/`)

## Status

- Interpreter covers the full DEX-1 instruction set used by real extension
  workloads (parsers, try/catch, defaults-arg bridges); complex HTML
  manipulation goes through the `jsoup` shims.
- Native dispatch is one `natives` map: built-in tables → global registry →
  per-context registrations.