//! app.cash.quickjs.QuickJs host implementation backed by a real QuickJS
//! engine via `rquickjs`. `evaluate` executes JavaScript and converts the
//! result to VM values; `compile`/`execute` round-trip a compiled-script token
//! (QuickJS bytecode export is not exposed by rquickjs, so the source stays
//! embedded and the returned byte array just identifies it).

use super::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::{Context, Runtime, Value};

use crate::vm::object::QuickJsHost;

fn quickjs_payload(vm: &mut Vm, v: JValue) -> Result<Rc<QuickJsHost>, NatErr> {
    let npe = npe(vm);
    match payload(vm, v) {
        Some(Native::QuickJs(host)) => Ok(Rc::clone(host)),
        _ => Err(npe),
    }
}

fn quickjs_create(vm: &mut Vm, _args: &[JValue]) -> R {
    log::debug!("quickjs: creating engine");
    let rt = Runtime::new().map_err(|e| iae(vm, format!("quickjs runtime: {e}")))?;
    let ctx = Context::full(&rt).map_err(|e| iae(vm, format!("quickjs context: {e}")))?;
    let host = Rc::new(QuickJsHost {
        rt: Rc::new(rt),
        ctx: Rc::new(ctx),
        next: RefCell::new(1),
        scripts: RefCell::new(HashMap::new()),
    });
    alloc(vm, "Lapp/cash/quickjs/QuickJs;", Native::QuickJs(host))
}

fn js_to_jvalue(vm: &mut Vm, v: &Value) -> Result<JValue, NatErr> {
    let npe = npe(vm);
    if v.is_null() || v.is_undefined() {
        return Ok(JValue::Null);
    }
    if let Some(b) = v.as_bool() {
        return Ok(JValue::Int(i32::from(b)));
    }
    if let Some(n) = v.as_number() {
        return Ok(JValue::Double(n));
    }
    if let Some(s) = v.as_string() {
        let s = s
            .to_string()
            .map_err(|e| nat_fatal(JvmError::Resolution(format!("quickjs string: {e}"))))?;
        return Ok(new_str(vm, &s));
    }
    log::warn!("quickjs: non-primitive result downgraded to null");
    Err(npe)
}

fn quickjs_evaluate(vm: &mut Vm, args: &[JValue]) -> R {
    let src = jstr(vm, args[1])?;
    let host = quickjs_payload(vm, args[0])?;
    host.ctx.with(|ctx| match ctx.eval::<Value, _>(src) {
        Ok(value) => js_to_jvalue(vm, &value),
        Err(e) => {
            log::warn!("quickjs: evaluate failed: {e}");
            Ok(JValue::Null)
        }
    })
}

fn quickjs_close(_vm: &mut Vm, args: &[JValue]) -> R {
    let npe = npe(_vm);
    match payload_mut(_vm, args[0]) {
        Some(Native::QuickJs(host)) => {
            host.scripts.borrow_mut().clear();
            Ok(JValue::Null)
        }
        _ => Err(npe),
    }
}

fn quickjs_compile(vm: &mut Vm, args: &[JValue]) -> R {
    let src = jstr(vm, args[1])?;
    let name = match args[2] {
        JValue::Null => String::new(),
        _ => jstr(vm, args[2]).unwrap_or_default(),
    };
    let host = quickjs_payload(vm, args[0])?;
    let mut next = host.next.borrow_mut();
    let token = *next;
    *next += 1;
    let token = token.to_le_bytes().to_vec();
    host.scripts.borrow_mut().insert(token.clone(), (src, name));
    alloc_arr(vm, "B", token.len(), move || {
        ArrayData::Byte(token.iter().map(|&b| b as i8).collect())
    })
}

fn quickjs_execute(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let host = quickjs_payload(vm, args[0])?;
    let script = host
        .scripts
        .borrow()
        .get(&bytes)
        .cloned()
        .ok_or_else(|| iae(vm, "quickjs: unknown script token"))?;
    host.ctx.with(|ctx| match ctx.eval::<Value, _>(script.0) {
        Ok(value) => js_to_jvalue(vm, &value),
        Err(e) => {
            log::warn!("quickjs: execute failed: {e}");
            Ok(JValue::Null)
        }
    })
}

pub(crate) const QUICKJS_TABLE: &[NativeEntry] = &[
    ne!(
        "Lapp/cash/quickjs/QuickJs;",
        "create",
        "()Lapp/cash/quickjs/QuickJs;",
        false,
        quickjs_create
    ),
    ne!(
        "Lapp/cash/quickjs/QuickJs;",
        "evaluate",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        true,
        quickjs_evaluate
    ),
    ne!(
        "Lapp/cash/quickjs/QuickJs;",
        "close",
        "()V",
        true,
        quickjs_close
    ),
    ne!(
        "Lapp/cash/quickjs/QuickJs;",
        "compile",
        "(Ljava/lang/String;Ljava/lang/String;)[B",
        true,
        quickjs_compile
    ),
    ne!(
        "Lapp/cash/quickjs/QuickJs;",
        "execute",
        "([B)Ljava/lang/Object;",
        true,
        quickjs_execute
    ),
];
