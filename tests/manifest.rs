//! Manifest (`AndroidManifest.xml` / `resources.arsc`) parsing tests over
//! the keiyoushi fixture APKs.

use dexvm::manifest::ManifestError;
use dexvm::{Context, SandboxOptions};

const CUUTRUYEN: &str = "fixtures/tachiyomi-vi.cuutruyenmoe-v1.6.3.apk";
const AKUMA: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";

fn open(apk: &str) -> Context {
    Context::open(apk).unwrap()
}

#[test]
fn cuutruyen_manifest_fields() {
    let mut ctx = open(CUUTRUYEN);
    let m = ctx.manifest().unwrap();
    assert_eq!(
        m.package_id,
        "eu.kanade.tachiyomi.extension.vi.cuutruyenmoe"
    );
    assert_eq!(m.app_name, "Tachiyomi: CuuTruyen (unoriginal)");
    assert_eq!(m.version_name.as_deref(), Some("1.6.3"));
    // Keiyoushi builds target Android 14; the fixture declares these.
    assert!(m.min_sdk.is_some());
    assert!(m.target_sdk.is_some());
    // android:icon is a reference into the resource table.
    assert_eq!(m.icon_resource_id, Some(0x7f010000));
}

#[test]
fn cuutruyen_icon_resolution() {
    let mut ctx = open(CUUTRUYEN);
    // Obfuscated build: the icon resource id maps to an opaque file path.
    let path = ctx.resource_path(0x7f010000).unwrap();
    assert_eq!(path, "res/9w.png");
    let bytes = ctx.resource_bytes(&path).unwrap();
    // PNG magic.
    assert_eq!(&bytes[..4], b"\x89PNG");
    // End-to-end: manifest id -> arsc -> entry.
    let icon = ctx.icon_bytes().unwrap();
    assert_eq!(icon, bytes);
}

#[test]
fn akuma_manifest_fields() {
    let mut ctx = open(AKUMA);
    let m = ctx.manifest().unwrap();
    assert_eq!(m.package_id, "eu.kanade.tachiyomi.extension.all.akuma");
    assert_eq!(m.icon_resource_id, Some(0x7f010000));
    let path = ctx.resource_path(0x7f010000).unwrap();
    assert_eq!(path, "res/9w.png");
    let icon = ctx.icon_bytes().unwrap();
    assert_eq!(&icon[..4], b"\x89PNG");
}

#[test]
fn plain_dex_has_no_manifest() {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    assert!(matches!(ctx.manifest(), Err(ManifestError::Missing(_))));
    assert!(ctx.icon_bytes().is_none());
}
