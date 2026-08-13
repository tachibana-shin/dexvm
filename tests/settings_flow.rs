//! Preference definitions + get/save round-trips through the AndroidX
//! preference and Android SharedPreferences host shims, end to end:
//!
//! - definitions: `ConfigurableSource.setupPreferenceScreen(screen)` is
//!   invoked by the host and the declared preferences are materialized
//!   (`preference_definitions`);
//! - get: `get_settings` returns the default state (empty) and, after a
//!   save, the persisted value;
//! - save: `update_setting` writes through the same SharedPreferences
//!   store the extension reads, and a fresh engine sees the value from
//!   disk (Android's lazy load on first read).
//!
//! Uses the keiyoushi `vi.cuutruyenmoe` 1.6.3 fixture, which declares a
//! single `website_password` EditTextPreference (default `"5"`).

use dexvm::keiyoushi::Keiyoushi;

const APK: &str = "fixtures/tachiyomi-vi.cuutruyenmoe-v1.6.3.apk";
/// Preference file name the extension requests via
/// `getSharedPreferences("source_<source-id-hash>", ...)`.
const PREFS_FILE: &str = "source_3973269831131863421";
const APK_PRE16: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";

fn init_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}

#[test]
fn mihon_preference_definitions_and_roundtrip() {
    init_logger();
    let dir = std::env::temp_dir().join(format!("dexvm-prefs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prefs_path = dir.join("prefs.bin");
    let _ = std::fs::remove_file(&prefs_path);

    let mut ext = Keiyoushi::open(APK).unwrap();
    ext.set_shared_preferences_path(&prefs_path);
    let srcs = ext.sources().unwrap();
    assert_eq!(srcs.len(), 1);
    let src = &srcs[0];

    // 1. definitions materialize without any network access. The preference
    // screen becomes a group whose single child is the website_password
    // EditTextPreference.
    let defs = ext.preference_definitions(src).unwrap();
    assert_eq!(defs.len(), 1, "expected one preference screen root: {defs:?}");
    let screen = &defs[0];
    assert_eq!(screen.title.as_deref(), None, "screen has no title: {screen:?}");
    let wp = screen
        .children
        .iter()
        .find(|d| d.key.as_deref() == Some("website_password"))
        .unwrap_or_else(|| panic!("cuutruyen must declare the website_password preference: {screen:?}"));
    assert_eq!(wp.title.as_deref(), Some("Mật khẩu truy cập website"));
    assert_eq!(wp.summary.as_deref(), Some("Mặc định: 5"));
    // default_value is the java String "5" (arena object holding Str)
    let default_str = match wp.default_value {
        dexvm::vm::value::JValue::Obj(id) => ext
            .string_of(id)
            .unwrap_or_else(|| panic!("default must be a string, got object {id}")),
        other => panic!("default must be a string, got {other:?}"),
    };
    assert_eq!(default_str, "5");

    // 2. get: nothing persisted yet
    let got = ext.get_settings(PREFS_FILE);
    assert!(
        got.is_empty(),
        "fresh extension must report no saved settings: {got:?}"
    );

    // 3. save
    ext.update_setting(
        PREFS_FILE,
        "website_password",
        dexvm::context::SettingValue::String("hunter2".into()),
    )
    .unwrap();
    let got = ext.get_settings(PREFS_FILE);
    assert_eq!(
        got.get("website_password"),
        Some(&dexvm::context::SettingValue::String("hunter2".into()))
    );

    // 4. a fresh engine sees the persisted value from disk
    drop(ext);
    let mut ext2 = Keiyoushi::open(APK).unwrap();
    ext2.set_shared_preferences_path(&prefs_path);
    let got = ext2.get_settings(PREFS_FILE);
    assert_eq!(
        got.get("website_password"),
        Some(&dexvm::context::SettingValue::String("hunter2".into())),
        "persisted value must survive a fresh engine"
    );

    let _ = std::fs::remove_file(&prefs_path);
}

#[test]
fn pre16_switch_preference_definitions() {
    init_logger();
    let dir = std::env::temp_dir().join(format!("dexvm-prefs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prefs_path = dir.join("prefs.bin");
    let _ = std::fs::remove_file(&prefs_path);

    let mut ext = Keiyoushi::open(APK_PRE16).unwrap();
    ext.set_shared_preferences_path(&prefs_path);
    let srcs = ext.sources().unwrap();
    let src = &srcs[0];

    let defs = ext.preference_definitions(src).unwrap();
    assert_eq!(defs.len(), 1, "expected one preference screen root: {defs:?}");
    let pref = &defs[0].children[0];
    assert_eq!(pref.key.as_deref(), Some("pref_title"));
    assert_eq!(
        pref.title.as_deref(),
        Some("Display manga title as full title")
    );
    assert!(
        matches!(pref.default_value, dexvm::vm::value::JValue::Int(0)),
        "default_value was {:?}",
        pref.default_value
    );
    assert!(pref.enabled);

    // same get/save round-trip, via the extension's own prefs file name
    let got = ext.get_settings("source_akuma_prefs");
    assert!(got.is_empty());
    ext.update_setting(
        "source_akuma_prefs",
        "pref_title",
        dexvm::context::SettingValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        ext.get_settings("source_akuma_prefs").get("pref_title"),
        Some(&dexvm::context::SettingValue::Bool(true))
    );

    let _ = std::fs::remove_file(&prefs_path);
}
