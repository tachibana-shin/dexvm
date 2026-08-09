//! android / androidx framework host shims (keiyoushi feature).

use super::*;

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

pub(crate) fn context_get_shared_prefs(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/SharedPreferences;", Native::Opaque)
}

pub(crate) fn shared_prefs_get_boolean(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[2])
}

pub(crate) fn prefs_obj(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn prefs_ctx(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/Context;", Native::Opaque)
}
pub(crate) fn prefs_set(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}


// ---------------------------------------------------------------------------
// android native table
// ---------------------------------------------------------------------------

pub(crate) const ANDROID_TABLE: &[NativeEntry] = &[
    ne!("Landroid/content/Context;", "getSharedPreferences", "(Ljava/lang/String;I)Landroid/content/SharedPreferences;", true, context_get_shared_prefs),
    ne!("Landroid/content/SharedPreferences;", "getBoolean", "(Ljava/lang/String;Z)Z", true, shared_prefs_get_boolean),
    ne!("Landroidx/preference/Preference;", "<init>", "(Landroid/content/Context;)V", true, prefs_obj),
    ne!("Landroidx/preference/Preference;", "setKey", "(Ljava/lang/String;)V", true, prefs_set),
    ne!("Landroidx/preference/Preference;", "setTitle", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/Preference;", "setSummary", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/Preference;", "setDefaultValue", "(Ljava/lang/Object;)V", true, prefs_set),
    ne!("Landroidx/preference/PreferenceScreen;", "<init>", "(Landroid/content/Context;)V", true, prefs_obj),
    ne!("Landroidx/preference/PreferenceScreen;", "getContext", "()Landroid/content/Context;", true, prefs_ctx),
    ne!("Landroidx/preference/PreferenceScreen;", "setTitle", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "<init>", "(Landroid/content/Context;)V", true, prefs_obj),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setKey", "(Ljava/lang/String;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setTitle", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setSummary", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setDefaultValue", "(Ljava/lang/Object;)V", true, prefs_set),
];
