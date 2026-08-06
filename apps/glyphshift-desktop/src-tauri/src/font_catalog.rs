use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const WINDOWS_FONT_REGISTRY_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";
const FONT_CACHE_SCHEMA: &str = "glyphshift.font-cache/1";
const FONT_CACHE_FILE: &str = "font-families.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontFamilyCache {
    schema: Box<str>,
    families: Vec<Box<str>>,
}

pub(super) fn load(data_root: &Path) -> Vec<Box<str>> {
    load_cached_font_families(data_root, system_font_families)
}

pub(super) fn refresh(data_root: &Path) -> Result<Vec<Box<str>>, std::io::Error> {
    refresh_cached_font_families(data_root, system_font_families)
}

fn load_cached_font_families(
    data_root: &Path,
    scan: impl FnOnce() -> Vec<Box<str>>,
) -> Vec<Box<str>> {
    if let Some(families) = read_cached_font_families(data_root) {
        return families;
    }
    let families = normalize_font_families(scan());
    let _ = write_cached_font_families(data_root, &families);
    families
}

fn refresh_cached_font_families(
    data_root: &Path,
    scan: impl FnOnce() -> Vec<Box<str>>,
) -> Result<Vec<Box<str>>, std::io::Error> {
    let families = normalize_font_families(scan());
    write_cached_font_families(data_root, &families)?;
    Ok(families)
}

fn read_cached_font_families(data_root: &Path) -> Option<Vec<Box<str>>> {
    let source = fs::read_to_string(data_root.join(FONT_CACHE_FILE)).ok()?;
    let artifact = serde_json::from_str::<FontFamilyCache>(&source).ok()?;
    if artifact.schema.as_ref() != FONT_CACHE_SCHEMA {
        return None;
    }
    let families = normalize_font_families(artifact.families);
    (!families.is_empty()).then_some(families)
}

fn write_cached_font_families(
    data_root: &Path,
    families: &[Box<str>],
) -> Result<(), std::io::Error> {
    if families.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "font catalog is empty",
        ));
    }
    fs::create_dir_all(data_root)?;
    let payload = serde_json::to_vec_pretty(&FontFamilyCache {
        schema: FONT_CACHE_SCHEMA.into(),
        families: families.to_vec(),
    })
    .map_err(std::io::Error::other)?;
    fs::write(data_root.join(FONT_CACHE_FILE), payload)
}

fn normalize_font_families(
    families: impl IntoIterator<Item = impl Into<Box<str>>>,
) -> Vec<Box<str>> {
    families
        .into_iter()
        .map(Into::into)
        .filter_map(|family| {
            let family = family.trim();
            (!family.is_empty()).then(|| Box::<str>::from(family))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(target_os = "windows")]
fn system_font_families() -> Vec<Box<str>> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let mut families = BTreeSet::new();
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let Ok(fonts) =
            RegKey::predef(hive).open_subkey_with_flags(WINDOWS_FONT_REGISTRY_KEY, KEY_READ)
        else {
            continue;
        };
        for (label, _) in fonts.enum_values().flatten() {
            if let Some(family) = normalize_font_registry_label(&label) {
                families.insert(Box::<str>::from(family));
            }
        }
    }
    families.into_iter().collect()
}

#[cfg(not(target_os = "windows"))]
fn system_font_families() -> Vec<Box<str>> {
    Vec::new()
}

fn normalize_font_registry_label(label: &str) -> Option<&str> {
    let label = label.trim().trim_start_matches('@').trim();
    let family = label
        .rfind(" (")
        .filter(|_| label.ends_with(')'))
        .map_or(label, |suffix| &label[..suffix]);
    (!family.is_empty()).then_some(family)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn registry_labels_become_user_facing_family_options() {
        assert_eq!(
            normalize_font_registry_label("Synthetic Sans (TrueType)"),
            Some("Synthetic Sans")
        );
        assert_eq!(
            normalize_font_registry_label("Synthetic Variable Font"),
            Some("Synthetic Variable Font")
        );
        assert_eq!(normalize_font_registry_label("  "), None);
    }

    #[test]
    fn cache_skips_system_scan_until_explicit_refresh() {
        let root = tempdir().expect("font cache root");
        let mut scans = 0;
        let first = load_cached_font_families(root.path(), || {
            scans += 1;
            vec![
                Box::<str>::from("Zulu Sans"),
                Box::<str>::from("Alpha Sans"),
            ]
        });
        assert_eq!(scans, 1);
        assert_eq!(
            first,
            vec![
                Box::<str>::from("Alpha Sans"),
                Box::<str>::from("Zulu Sans")
            ]
        );

        let second = load_cached_font_families(root.path(), || {
            scans += 1;
            vec![Box::<str>::from("must not scan")]
        });
        assert_eq!(scans, 1);
        assert_eq!(second, first);

        let refreshed = refresh_cached_font_families(root.path(), || {
            scans += 1;
            vec![Box::<str>::from("Refreshed Sans")]
        })
        .expect("refresh font cache");
        assert_eq!(scans, 2);
        assert_eq!(refreshed, vec![Box::<str>::from("Refreshed Sans")]);

        let reopened = load_cached_font_families(root.path(), || {
            scans += 1;
            vec![Box::<str>::from("must not rescan")]
        });
        assert_eq!(scans, 2);
        assert_eq!(reopened, refreshed);
    }
}
