//! Host library demo: register new native methods at runtime, both
//! per-context and process-wide, then call them from dex code.
//!
//! Run with: `cargo run --example host_native`

use dexvm::vm::native::{register_global, NativeEntry};
use dexvm::vm::value::JValue;
use dexvm::vm::Vm;
use dexvm::{Context, SandboxOptions};
use dexvm::vm::native::NatErr;

/// A native that echoes a string back ("host: <input>").
fn echo(vm: &mut Vm, args: &[JValue]) -> Result<JValue, NatErr> {
    let input = match &vm.arena.objects[args[0].as_obj() as usize].native {
        Some(dexvm::vm::object::Native::Str(s)) => s.clone(),
        _ => String::new(),
    };
    Ok(vm.alloc_string(&format!("host: {input}")))
}

/// A native registered for *every* context (process-wide).
fn ping(vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
    Ok(vm.alloc_string("pong"))
}

static HOST_TABLE: &[NativeEntry] = &[NativeEntry {
    class: "Lcom/example/host/Host;",
    name: "echo",
    sig: "(Ljava/lang/String;)Ljava/lang/String;",
    instance: false,
    f: echo,
}];

static PING_TABLE: &[NativeEntry] = &[NativeEntry {
    class: "Lcom/example/host/Ping;",
    name: "ping",
    sig: "()Ljava/lang/String;",
    instance: false,
    f: ping,
}];

fn main() {
    // Global registration: visible to every Context created from now on.
    register_global(PING_TABLE);

    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();

    // Per-context registration: only this context sees `echo`.
    ctx.register_natives(&[HOST_TABLE]).unwrap();

    let arg = ctx.vm().alloc_string("world");
    let JValue::Obj(ret) = ctx.call("Lcom/example/host/Host;", "echo", &[arg]).unwrap() else {
        panic!("echo did not return a string");
    };
    let s = match &ctx.vm().arena.objects[ret as usize].native {
        Some(dexvm::vm::object::Native::Str(s)) => s.clone(),
        _ => unreachable!(),
    };
    assert_eq!(s, "host: world");
    println!("per-context native: {s}");

    // Global native calls work too (registered above).
    let JValue::Obj(ret) = ctx.call("Lcom/example/host/Ping;", "ping", &[]).unwrap() else {
        panic!("ping did not return a string");
    };
    let s = match &ctx.vm().arena.objects[ret as usize].native {
        Some(dexvm::vm::object::Native::Str(s)) => s.clone(),
        _ => unreachable!(),
    };
    assert_eq!(s, "pong");
    println!("global native: {s}");
}