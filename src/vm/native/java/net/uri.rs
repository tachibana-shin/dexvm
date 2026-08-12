//! java.net.URI host shims.

use crate::vm::native::*;

fn init(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    vm.arena.objects[args[0].as_obj() as usize].native = Some(Native::URI(value));
    Ok(JValue::Null)
}

fn get_host(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match payload(vm, args[0]) {
        Some(Native::URI(value)) => value.clone(),
        _ => return Err(npe(vm)),
    };
    let authority = value
        .split("://")
        .last()
        .and_then(|part| part.strip_prefix("//").or(Some(part)))
        .unwrap_or(&value);
    Ok(new_str(
        vm,
        authority.split(['/', ':']).next().unwrap_or(""),
    ))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/net/URI;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        init
    ),
    ne!(
        "Ljava/net/URI;",
        "getHost",
        "()Ljava/lang/String;",
        true,
        get_host
    ),
];
