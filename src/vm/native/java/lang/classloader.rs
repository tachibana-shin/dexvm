//! java.lang.ClassLoader access to resources embedded in the input APK.

use crate::vm::native::*;

fn get_resource_as_stream(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let name = name.trim_start_matches('/');
    let Some(bytes) = vm.resources.get(name).cloned() else {
        return Ok(JValue::Null);
    };
    alloc(
        vm,
        "Ljava/io/ByteArrayInputStream;",
        Native::ByteArrayInputStream { bytes, pos: 0 },
    )
}

pub(crate) const TABLE: &[NativeEntry] = &[ne!(
    "Ljava/lang/ClassLoader;",
    "getResourceAsStream",
    "(Ljava/lang/String;)Ljava/io/InputStream;",
    true,
    get_resource_as_stream
)];
