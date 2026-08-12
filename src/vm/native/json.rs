//! org.json host implementation (`JSONObject` / `JSONArray`). Values are
//! stored as [`JValue`] so strings, boxed numbers and nested objects all stay
//! ordinary VM objects. Parsing and serialization are delegated to
//! `serde_json` (with `preserve_order`, so key order matches org.json).
//! An extension-facing `JSONException` is registered as a throwable shim but
//! these natives never raise it: type mismatches degrade to `null` / 0 to
//! keep captures running.

use super::*;

use serde_json::{Map as JsonMap, Value as Json};

// ---------------------------------------------------------------------------
// JSON parsing (string -> JValue tree) via serde_json
// ---------------------------------------------------------------------------

fn json_to_jvalue(vm: &mut Vm, v: &Json) -> Result<JValue, String> {
    match v {
        Json::Null => Ok(JValue::Null),
        Json::Bool(b) => Ok(bool_box(vm, *b)),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                match i32::try_from(i) {
                    Ok(i) => Ok(JValue::Int(i)),
                    Err(_) => Ok(JValue::Long(i)),
                }
            } else if let Some(f) = n.as_f64() {
                Ok(JValue::Double(f))
            } else {
                Ok(JValue::Null)
            }
        }
        Json::String(s) => Ok(new_str(vm, s)),
        Json::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_jvalue(vm, item)?);
            }
            alloc(vm, "Lorg/json/JSONArray;", Native::JsonArr(out))
                .map_err(|e| format!("alloc: {e:?}"))
        }
        Json::Object(map) => {
            let mut out = Vec::with_capacity(map.len());
            for (k, v) in map {
                out.push((k.clone(), json_to_jvalue(vm, v)?));
            }
            alloc(vm, "Lorg/json/JSONObject;", Native::JsonObj(out))
                .map_err(|e| format!("alloc: {e:?}"))
        }
    }
}

fn bool_box(vm: &mut Vm, b: bool) -> JValue {
    alloc(vm, "Ljava/lang/Boolean;", Native::BoolBox(b)).unwrap_or(JValue::Null)
}

fn opt_jstr(vm: &mut Vm, v: JValue) -> Result<Option<String>, NatErr> {
    if v == JValue::Null {
        Ok(None)
    } else {
        Ok(Some(jstr(vm, v)?))
    }
}

// ---------------------------------------------------------------------------
// payload accessors
// ---------------------------------------------------------------------------

fn json_obj(vm: &mut Vm, v: JValue) -> Result<&Vec<(String, JValue)>, NatErr> {
    let npe = npe(vm);
    if !matches!(payload(vm, v), Some(Native::JsonObj(_))) {
        return Err(npe);
    }
    match payload(vm, v) {
        Some(Native::JsonObj(pairs)) => Ok(pairs),
        _ => unreachable!("payload checked"),
    }
}

fn json_obj_mut(vm: &mut Vm, v: JValue) -> Result<&mut Vec<(String, JValue)>, NatErr> {
    let npe = npe(vm);
    if !matches!(payload_mut(vm, v), Some(Native::JsonObj(_))) {
        return Err(npe);
    }
    match payload_mut(vm, v) {
        Some(Native::JsonObj(pairs)) => Ok(pairs),
        _ => unreachable!("payload checked"),
    }
}

fn json_arr(vm: &mut Vm, v: JValue) -> Result<&Vec<JValue>, NatErr> {
    let npe = npe(vm);
    if !matches!(payload(vm, v), Some(Native::JsonArr(_))) {
        return Err(npe);
    }
    match payload(vm, v) {
        Some(Native::JsonArr(items)) => Ok(items),
        _ => unreachable!("payload checked"),
    }
}

fn json_arr_mut(vm: &mut Vm, v: JValue) -> Result<&mut Vec<JValue>, NatErr> {
    let npe = npe(vm);
    if !matches!(payload_mut(vm, v), Some(Native::JsonArr(_))) {
        return Err(npe);
    }
    match payload_mut(vm, v) {
        Some(Native::JsonArr(items)) => Ok(items),
        _ => unreachable!("payload checked"),
    }
}

/// Owned lookup: clones the value out so callers can mutate `vm` afterwards.
fn json_find(vm: &mut Vm, obj: JValue, key: &str) -> Result<Option<JValue>, NatErr> {
    for (k, v) in json_obj(vm, obj)? {
        if *k == key {
            return Ok(Some(*v));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// value formatting via serde_json
// ---------------------------------------------------------------------------

/// org.json `numberToString`: integral doubles print as long integers, so
/// normalize before handing the value to serde_json.
fn normalize_number(n: f64) -> f64 {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 {
        n.trunc()
    } else {
        n
    }
}

/// JValue tree -> serde_json::Value (insertion order preserved).
fn jvalue_to_json(vm: &mut Vm, v: &JValue) -> Result<Json, NatErr> {
    let npe = npe(vm);
    match v {
        JValue::Null => Ok(Json::Null),
        JValue::Int(i) => Ok(Json::from(*i)),
        JValue::Long(l) => Ok(Json::from(*l)),
        JValue::Float(f) => Ok(Json::from(normalize_number(f64::from(*f)))),
        JValue::Double(d) => Ok(Json::from(normalize_number(*d))),
        JValue::Obj(_) => match payload(vm, *v) {
            Some(Native::Str(s)) => Ok(Json::String(s.clone())),
            Some(Native::IntBox(x)) => Ok(Json::from(*x)),
            Some(Native::LongBox(x)) => Ok(Json::from(*x)),
            Some(Native::FloatBox(x)) => Ok(Json::from(normalize_number(f64::from(*x)))),
            Some(Native::DoubleBox(x)) => Ok(Json::from(normalize_number(*x))),
            Some(Native::BoolBox(b)) => Ok(Json::from(*b)),
            Some(Native::JsonObj(pairs)) => {
                let pairs = pairs.clone();
                let mut map = JsonMap::new();
                for (k, val) in &pairs {
                    map.insert(k.clone(), jvalue_to_json(vm, val)?);
                }
                Ok(Json::Object(map))
            }
            Some(Native::JsonArr(items)) => {
                let items = items.clone();
                let mut arr = Vec::with_capacity(items.len());
                for item in &items {
                    arr.push(jvalue_to_json(vm, item)?);
                }
                Ok(Json::Array(arr))
            }
            _ => Err(npe),
        },
    }
}

/// `JSONObject.getString`-style rendering: strings unwrap, numbers print as
/// `numberToString`, booleans as true/false, null as "null".
fn value_to_string(vm: &mut Vm, v: &JValue) -> Result<String, NatErr> {
    let json = jvalue_to_json(vm, v)?;
    Ok(match &json {
        Json::String(s) => s.clone(),
        Json::Null => "null".into(),
        Json::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        _ => json.to_string(),
    })
}

fn quote_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn serialize_value(vm: &mut Vm, v: &JValue) -> Result<String, NatErr> {
    Ok(jvalue_to_json(vm, v)?.to_string())
}

// ---------------------------------------------------------------------------
// natives
// ---------------------------------------------------------------------------

fn json_object_init(vm: &mut Vm, args: &[JValue]) -> R {
    match args.len() {
        1 => alloc(vm, "Lorg/json/JSONObject;", Native::JsonObj(Vec::new())),
        2 => match opt_jstr(vm, args[1])? {
            Some(s) => parse_json_text(vm, &s, false),
            None => Err(npe(vm)),
        },
        _ => Err(npe(vm)),
    }
}

fn parse_json_text(vm: &mut Vm, text: &str, want_array: bool) -> R {
    let parsed: Json = serde_json::from_str(text)
        .map_err(|e| nat_fatal(JvmError::Resolution(format!("JSON parse error: {e}"))))?;
    match parsed {
        Json::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in &items {
                out.push(json_to_jvalue(vm, item).map_err(|e| {
                    nat_fatal(JvmError::Resolution(format!("JSON parse error: {e}")))
                })?);
            }
            alloc(vm, "Lorg/json/JSONArray;", Native::JsonArr(out))
        }
        value if !want_array => json_to_jvalue(vm, &value)
            .map_err(|e| nat_fatal(JvmError::Resolution(format!("JSON parse error: {e}")))),
        _ => Err(nat_fatal(JvmError::Resolution(
            "JSON parse error: expected array".into(),
        ))),
    }
}

fn json_object_init_map(vm: &mut Vm, args: &[JValue]) -> R {
    let npe = npe(vm);
    let mut out = Vec::new();
    match payload(vm, args[1]) {
        Some(Native::Map(entries)) => {
            for (k, v) in entries {
                let key = match payload(vm, *k) {
                    Some(Native::Str(s)) => s.clone(),
                    _ => return Err(npe),
                };
                out.push((key, *v));
            }
        }
        _ => return Err(npe),
    }
    alloc(vm, "Lorg/json/JSONObject;", Native::JsonObj(out))
}

fn json_get(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    Ok(json_find(vm, args[0], &key)?.unwrap_or(JValue::Null))
}

fn json_get_string(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = json_find(vm, args[0], &key)?.ok_or_else(|| npe(vm))?;
    let text = value_to_string(vm, &value)?;
    Ok(new_str(vm, &text))
}

fn json_get_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    Ok(json_find(vm, args[0], &key)?.unwrap_or(JValue::Null))
}

fn json_get_long(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = json_find(vm, args[0], &key)?.ok_or_else(|| npe(vm))?;
    Ok(JValue::Long(long_of(vm, value)))
}

fn json_get_int(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = json_find(vm, args[0], &key)?.ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(int_of(vm, value)))
}

fn json_get_bool(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = json_find(vm, args[0], &key)?.ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(i32::from(bool_of(vm, value))))
}

fn json_has(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let has = json_find(vm, args[0], &key)?.is_some();
    Ok(JValue::Int(i32::from(has)))
}

fn json_keys(vm: &mut Vm, args: &[JValue]) -> R {
    let names: Vec<String> = json_obj(vm, args[0])?
        .iter()
        .map(|(k, _)| k.clone())
        .collect();
    let keys: Vec<JValue> = names.iter().map(|k| new_str(vm, k)).collect();
    let list = alloc(vm, "Ljava/util/ArrayList;", Native::List(keys))?;
    let id = match list {
        JValue::Obj(id) => id,
        _ => unreachable!("alloc returns Obj"),
    };
    alloc(
        vm,
        "Ljava/util/Iterator;",
        Native::Iter(IterKind::List { list: id, idx: 0 }),
    )
}

fn json_put_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let pairs = json_obj_mut(vm, args[0])?;
    if let Some((_, slot)) = pairs.iter_mut().find(|(k, _)| *k == key) {
        *slot = args[2];
    } else {
        pairs.push((key, args[2]));
    }
    Ok(args[0])
}

fn json_put_prim(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = args[2];
    let pairs = json_obj_mut(vm, args[0])?;
    if let Some((_, slot)) = pairs.iter_mut().find(|(k, _)| *k == key) {
        *slot = value;
    } else {
        pairs.push((key, value));
    }
    Ok(args[0])
}

fn json_put_bool(vm: &mut Vm, args: &[JValue]) -> R {
    let b = args[2] != JValue::Null && int_of(vm, args[2]) != 0;
    let boxed = bool_box(vm, b);
    json_put_prim(vm, &[args[0], args[1], boxed])
}

fn json_opt_string(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let dflt = match args.len() {
        3 => opt_jstr(vm, args[2])?.unwrap_or_default(),
        _ => String::new(),
    };
    match json_find(vm, args[0], &key)? {
        Some(value) if value != JValue::Null => {
            let text = value_to_string(vm, &value)?;
            Ok(new_str(vm, &text))
        }
        _ => Ok(new_str(vm, &dflt)),
    }
}

fn json_opt_long(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let dflt = match args.len() {
        3 => long_of(vm, args[2]),
        _ => 0,
    };
    match json_find(vm, args[0], &key)? {
        Some(value) if value != JValue::Null => Ok(JValue::Long(long_of(vm, value))),
        _ => Ok(JValue::Long(dflt)),
    }
}

fn json_opt_int(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let dflt = match args.len() {
        3 => int_of(vm, args[2]),
        _ => 0,
    };
    match json_find(vm, args[0], &key)? {
        Some(value) if value != JValue::Null => Ok(JValue::Int(int_of(vm, value))),
        _ => Ok(JValue::Int(dflt)),
    }
}

fn json_opt_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    Ok(json_find(vm, args[0], &key)?.unwrap_or(JValue::Null))
}

fn json_obj_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let text = serialize_value(vm, &args[0])?;
    Ok(new_str(vm, &text))
}

fn json_quote(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    Ok(new_str(vm, &quote_string(&s)))
}

fn json_array_init(vm: &mut Vm, args: &[JValue]) -> R {
    match args.len() {
        1 => alloc(vm, "Lorg/json/JSONArray;", Native::JsonArr(Vec::new())),
        2 => match opt_jstr(vm, args[1])? {
            Some(s) => parse_json_text(vm, &s, true),
            None => Err(npe(vm)),
        },
        _ => Err(npe(vm)),
    }
}

fn json_array_length(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(json_arr(vm, args[0])?.len() as i32))
}

fn json_array_get(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]) as usize;
    match json_arr(vm, args[0])?.get(idx) {
        Some(v) => Ok(*v),
        None => Ok(JValue::Null),
    }
}

fn json_array_get_string(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]) as usize;
    match json_arr(vm, args[0])?.get(idx).copied() {
        Some(v) => {
            let text = value_to_string(vm, &v)?;
            Ok(new_str(vm, &text))
        }
        None => Err(npe(vm)),
    }
}

fn json_array_get_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]) as usize;
    match json_arr(vm, args[0])?.get(idx) {
        Some(v) => Ok(*v),
        None => Ok(JValue::Null),
    }
}

fn json_array_put(vm: &mut Vm, args: &[JValue]) -> R {
    json_arr_mut(vm, args[0])?.push(args[1]);
    Ok(args[0])
}

fn json_array_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let text = serialize_value(vm, &args[0])?;
    Ok(new_str(vm, &text))
}

// ---------------------------------------------------------------------------
// table
// ---------------------------------------------------------------------------

pub(crate) const JSON_TABLE: &[NativeEntry] = &[
    ne!(
        "Lorg/json/JSONObject;",
        "<init>",
        "()V",
        true,
        json_object_init
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        json_object_init
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "<init>",
        "(Ljava/util/Map;)V",
        true,
        json_object_init_map
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "get",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        true,
        json_get
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "getString",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        json_get_string
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "getJSONObject",
        "(Ljava/lang/String;)Lorg/json/JSONObject;",
        true,
        json_get_obj
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "getJSONArray",
        "(Ljava/lang/String;)Lorg/json/JSONArray;",
        true,
        json_get_obj
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "getLong",
        "(Ljava/lang/String;)J",
        true,
        json_get_long
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "getInt",
        "(Ljava/lang/String;)I",
        true,
        json_get_int
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "getBoolean",
        "(Ljava/lang/String;)Z",
        true,
        json_get_bool
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "has",
        "(Ljava/lang/String;)Z",
        true,
        json_has
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "keys",
        "()Ljava/util/Iterator;",
        true,
        json_keys
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "put",
        "(Ljava/lang/String;Ljava/lang/Object;)Lorg/json/JSONObject;",
        true,
        json_put_obj
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "put",
        "(Ljava/lang/String;I)Lorg/json/JSONObject;",
        true,
        json_put_prim
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "put",
        "(Ljava/lang/String;J)Lorg/json/JSONObject;",
        true,
        json_put_prim
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "put",
        "(Ljava/lang/String;D)Lorg/json/JSONObject;",
        true,
        json_put_prim
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "put",
        "(Ljava/lang/String;Z)Lorg/json/JSONObject;",
        true,
        json_put_bool
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optString",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        json_opt_string
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optString",
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        true,
        json_opt_string
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optLong",
        "(Ljava/lang/String;)J",
        true,
        json_opt_long
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optLong",
        "(Ljava/lang/String;J)J",
        true,
        json_opt_long
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optInt",
        "(Ljava/lang/String;)I",
        true,
        json_opt_int
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optInt",
        "(Ljava/lang/String;I)I",
        true,
        json_opt_int
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optJSONObject",
        "(Ljava/lang/String;)Lorg/json/JSONObject;",
        true,
        json_opt_obj
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "optJSONArray",
        "(Ljava/lang/String;)Lorg/json/JSONArray;",
        true,
        json_opt_obj
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "remove",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        true,
        json_get
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "toString",
        "()Ljava/lang/String;",
        true,
        json_obj_to_string
    ),
    ne!(
        "Lorg/json/JSONObject;",
        "quote",
        "(Ljava/lang/String;)Ljava/lang/String;",
        false,
        json_quote
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "<init>",
        "()V",
        true,
        json_array_init
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        json_array_init
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "length",
        "()I",
        true,
        json_array_length
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "get",
        "(I)Ljava/lang/Object;",
        true,
        json_array_get
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "getString",
        "(I)Ljava/lang/String;",
        true,
        json_array_get_string
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "getJSONObject",
        "(I)Lorg/json/JSONObject;",
        true,
        json_array_get_obj
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "getJSONArray",
        "(I)Lorg/json/JSONArray;",
        true,
        json_array_get_obj
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "put",
        "(Ljava/lang/Object;)Lorg/json/JSONArray;",
        true,
        json_array_put
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "put",
        "(I)Lorg/json/JSONArray;",
        true,
        json_array_put
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "opt",
        "(I)Ljava/lang/Object;",
        true,
        json_array_get
    ),
    ne!(
        "Lorg/json/JSONArray;",
        "toString",
        "()Ljava/lang/String;",
        true,
        json_array_to_string
    ),
];
