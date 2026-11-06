use crate::{
    configuration::{model::Key, storage::SettingsDocument},
    stats::session::DailyBucket,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

const MAX_EXPORT_BYTES: u64 = 256 * 1024 * 1024;
const BUNDLE_NAME: &str = "legacy-import.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct LegacyBundle {
    format_version: u32,
    source_app_version: String,
    settings_migration_version: u32,
    stats_migration_version: u32,
    source_profile_id: String,
    exported_at: String,
    pub settings: SettingsDocument,
    pub stats: LegacyStats,
    pub app_registry: BTreeMap<String, String>,
    pub spotlight_usage: LegacyUsage,
    integrity_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct LegacyStats {
    pub buckets: Vec<DailyBucket>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct LegacyUsage {
    pub counts: BTreeMap<String, i32>,
}

static PROFILE_BUNDLE_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<LegacyBundle>>>> = OnceLock::new();

pub(crate) fn import(export_path: &Path, data_dir: &Path) -> Result<bool, String> {
    let bytes = read_bounded(export_path)?;
    let bundle = parse_and_validate(&bytes)?;
    fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
    let marker = data_dir.join(BUNDLE_NAME);

    if marker.exists() {
        let existing = read_bounded(&marker)?;
        let prior = parse_and_validate(&existing)?;
        if prior
            .integrity_sha256
            .eq_ignore_ascii_case(&bundle.integrity_sha256)
        {
            return Ok(false);
        }
        return Err("A different legacy import is already present in this Rust profile.".into());
    }

    for name in [
        "settings.json",
        "stats.json",
        "app_registry.json",
        "spotlight_usage.json",
    ] {
        if data_dir.join(name).exists() {
            return Err(format!(
                "Rust profile already contains {name}; export or back up this profile before importing."
            ));
        }
    }

    crate::atomic_file::write(&marker, &bytes).map_err(|error| error.to_string())?;
    PROFILE_BUNDLE_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "legacy import cache lock poisoned".to_string())?
        .insert(marker, Arc::new(bundle));
    Ok(true)
}

pub(crate) fn profile_bundle(data_dir: &Path) -> Result<Option<Arc<LegacyBundle>>, String> {
    let path = data_dir.join(BUNDLE_NAME);
    if !path.exists() {
        return Ok(None);
    }
    let cache = PROFILE_BUNDLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(bundle) = cache
        .lock()
        .map_err(|_| "legacy import cache lock poisoned".to_string())?
        .get(&path)
        .cloned()
    {
        return Ok(Some(bundle));
    }
    let bundle = Arc::new(parse_and_validate(&read_bounded(&path)?)?);
    cache
        .lock()
        .map_err(|_| "legacy import cache lock poisoned".to_string())?
        .insert(path, bundle.clone());
    Ok(Some(bundle))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_EXPORT_BYTES {
        return Err("Legacy export exceeds the 256 MiB import limit.".into());
    }
    fs::read(path).map_err(|error| error.to_string())
}

fn parse_and_validate(bytes: &[u8]) -> Result<LegacyBundle, String> {
    let source: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("Invalid legacy export JSON: {error}"))?;
    let mut canonical = source.clone();
    let object = canonical
        .as_object_mut()
        .ok_or_else(|| "Legacy export must contain a JSON object.".to_string())?;
    let expected = object
        .remove("integrity_sha256")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| "Legacy export has no SHA-256 integrity digest.".to_string())?;
    sort_json_keys(&mut canonical);
    let canonical_bytes = serde_json::to_vec(&canonical)
        .map_err(|error| format!("Could not canonicalize legacy export: {error}"))?;
    let actual = format!("{:X}", Sha256::digest(&canonical_bytes));
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err("Legacy export integrity check failed.".into());
    }

    let mut bundle: LegacyBundle = serde_json::from_slice(bytes)
        .map_err(|error| format!("Unsupported legacy export fields or enum values: {error}"))?;
    validate_legacy_key_pairs(&source)?;
    if bundle.format_version != 1 || bundle.settings.schema_version != 2 {
        return Err("Unsupported legacy export or settings schema version.".into());
    }
    if bundle.source_profile_id.is_empty()
        || bundle.source_app_version.is_empty()
        || bundle.exported_at.is_empty()
        || bundle.settings_migration_version > 1
        || bundle.stats_migration_version > 2
    {
        return Err("Legacy export provenance or storage version is invalid.".into());
    }
    crate::stats::migrations::remove_default_date_bucket(&mut bundle.stats.buckets);
    let mut dates = std::collections::HashSet::new();
    for bucket in &bundle.stats.buckets {
        if !valid_date(&bucket.date)
            || !dates.insert(bucket.date.as_str())
            || [
                bucket.total_switches,
                bucket.total_time_saved_ms,
                bucket.total_peeks,
                bucket.alt_tab_switches,
                bucket.alt_tab_keystrokes,
            ]
            .iter()
            .any(|value| *value < 0)
        {
            return Err("Legacy export contains invalid or duplicate daily statistics.".into());
        }
        for stats in bucket
            .static_app_usage
            .values()
            .chain(bucket.dynamic_app_usage.values())
        {
            if [
                stats.switches,
                stats.peeks,
                stats.total_peek_time_ms,
                stats.total_switch_time_ms,
            ]
            .iter()
            .any(|value| *value < 0)
            {
                return Err("Legacy export contains negative per-app statistics.".into());
            }
        }
        let app_switches = bucket
            .static_app_usage
            .values()
            .chain(bucket.dynamic_app_usage.values())
            .map(|stats| i64::from(stats.switches))
            .sum::<i64>();
        let app_peeks = bucket
            .static_app_usage
            .values()
            .chain(bucket.dynamic_app_usage.values())
            .map(|stats| i64::from(stats.peeks))
            .sum::<i64>();
        if app_switches != i64::from(bucket.total_switches)
            || app_peeks != i64::from(bucket.total_peeks)
        {
            return Err("Legacy daily totals do not match the per-app records.".into());
        }
        if let Some(record) = &bucket.fastest_switch
            && (record.duration_ms <= 0 || !supported_legacy_key(record.letter))
        {
            return Err("Legacy export contains an invalid fastest-switch record.".into());
        }
    }
    if bundle
        .spotlight_usage
        .counts
        .values()
        .any(|count| *count < 0)
    {
        return Err("Legacy export contains a negative Spotlight usage count.".into());
    }
    for app in &bundle.settings.applications {
        if !supported_legacy_key(app.key) {
            return Err("Legacy export contains an unsupported application key.".into());
        }
    }
    if !supported_legacy_key(bundle.settings.modifier)
        || !supported_legacy_key(bundle.settings.spotlight_modifier)
    {
        return Err("Legacy export contains an unsupported modifier key.".into());
    }
    Ok(bundle)
}

fn sort_json_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            let mut entries = object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.encode_utf16().cmp(right.0.encode_utf16()));
            object.clear();
            for (key, mut value) in entries {
                sort_json_keys(&mut value);
                object.insert(key, value);
            }
        }
        serde_json::Value::Array(values) => values.iter_mut().for_each(sort_json_keys),
        _ => {}
    }
}

fn validate_legacy_key_pairs(source: &serde_json::Value) -> Result<(), String> {
    let settings = &source["settings"];
    validate_key_pair(settings, "modifier", "modifier_legacy_value")?;
    validate_key_pair(
        settings,
        "spotlight_modifier",
        "spotlight_modifier_legacy_value",
    )?;
    let apps = settings["applications"]
        .as_array()
        .ok_or_else(|| "Legacy export has no ordered application list.".to_string())?;
    for app in apps {
        validate_key_pair(app, "key", "key_legacy_value")?;
    }
    Ok(())
}

fn validate_key_pair(
    document: &serde_json::Value,
    name_field: &str,
    value_field: &str,
) -> Result<(), String> {
    let name = document[name_field]
        .as_str()
        .ok_or_else(|| format!("Legacy {name_field} has no WPF key name."))?;
    let value = document[value_field]
        .as_i64()
        .ok_or_else(|| format!("Legacy {name_field} has no WPF key value."))?;
    let mapped = legacy_key(value).ok_or_else(|| format!("Unsupported WPF key value {value}."))?;
    if mapped != name {
        return Err(format!("WPF key mapping mismatch for {name_field}."));
    }
    Ok(())
}

fn legacy_key(value: i64) -> Option<&'static str> {
    Some(match value {
        0 => "None",
        3 => "Tab",
        6 => "Return",
        8 => "Capital",
        13 => "Escape",
        18 => "Space",
        23 => "Left",
        24 => "Up",
        25 => "Right",
        26 => "Down",
        70 => "LWin",
        71 => "RWin",
        72 => "Apps",
        116 => "LeftShift",
        117 => "RightShift",
        118 => "LeftCtrl",
        119 => "RightCtrl",
        120 => "LeftAlt",
        121 => "RightAlt",
        34..=43 => {
            return Some(
                ["D0", "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9"][(value - 34) as usize],
            );
        }
        44..=69 => {
            return Some(
                [
                    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P",
                    "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
                ][(value - 44) as usize],
            );
        }
        _ => return None,
    })
}

fn supported_legacy_key(key: Key) -> bool {
    use Key::*;
    matches!(
        key,
        None | A
            | B
            | C
            | D
            | E
            | F
            | G
            | H
            | I
            | J
            | K
            | L
            | M
            | N
            | O
            | P
            | Q
            | R
            | S
            | T
            | U
            | V
            | W
            | X
            | Y
            | Z
            | D0
            | D1
            | D2
            | D3
            | D4
            | D5
            | D6
            | D7
            | D8
            | D9
            | Tab
            | Left
            | Right
            | Up
            | Down
            | Return
            | Escape
            | Space
            | LeftCtrl
            | LeftAlt
            | LeftShift
            | LWin
            | RightCtrl
            | RightAlt
            | RightShift
            | RWin
            | Apps
            | Capital
    )
}

fn valid_date(value: &str) -> bool {
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<i32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    day > 0 && day <= days && value == format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs;

    fn sample_bundle() -> serde_json::Value {
        serde_json::json!({
            "format_version": 1,
            "source_app_version": "1.0.2",
            "settings_migration_version": 1,
            "stats_migration_version": 2,
            "source_profile_id": "synthetic-profile",
            "exported_at": "2026-11-06T12:00:00+00:00",
            "settings": {
                "schema_version": 2, "modifier": "RightCtrl", "modifier_legacy_value": 119,
                "applications": [], "pulse_border_enabled": true, "theme": "Dark",
                "overlay_enabled": false, "overlay_show_delay_ms": 1000,
                "overlay_keep_open_while_modifier_held": true, "peek_enabled": false,
                "dynamic_mode_enabled": true, "stats_enabled": true, "spotlight_enabled": true,
                "spotlight_modifier": "LeftAlt", "spotlight_modifier_legacy_value": 120
            },
            "stats": {"buckets": [{"date": "2026-11-05", "total_switches": 9,
                "total_time_saved_ms": 1000, "total_peeks": 2, "static_app_usage": {
                    "editor.exe": {"switches": 9, "peeks": 2,
                        "total_peek_time_ms": 500, "total_switch_time_ms": 800}
                },
                "dynamic_app_usage": {}, "alt_tab_switches": 1, "alt_tab_keystrokes": 2,
                "fastest_switch": null}]},
            "app_registry": {"editor.exe": "Editor"},
            "spotlight_usage": {"counts": {"EDITOR.EXE": 4}}
        })
    }

    fn signed_sample() -> Vec<u8> {
        sign(sample_bundle())
    }

    fn sign(mut value: serde_json::Value) -> Vec<u8> {
        sort_json_keys(&mut value);
        let digest = format!("{:X}", Sha256::digest(serde_json::to_vec(&value).unwrap()));
        value["integrity_sha256"] = digest.into();
        serde_json::to_vec_pretty(&value).unwrap()
    }

    #[test]
    fn validates_digest_is_idempotent_and_rejects_a_different_import() {
        let root = std::env::temp_dir().join(format!("omnispot-import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("export.json");
        fs::write(&source, signed_sample()).unwrap();
        let data = root.join("profile");
        assert!(import(&source, &data).unwrap());
        assert!(!import(&source, &data).unwrap());
        assert!(profile_bundle(&data).unwrap().is_some());
        let config = crate::configuration::ConfigurationService::new(data.join("settings.json"))
            .read()
            .unwrap();
        assert_eq!(config.theme, crate::configuration::AppThemeSetting::Dark);
        assert!(config.stats_enabled);
        let mut registry =
            crate::window_discovery::AppRegistryCache::new(data.join("app_registry.json"));
        assert_eq!(registry.get_display_name("editor.exe", None), "Editor");
        let usage = crate::spotlight::SpotlightUsageStore::new(data.join("spotlight_usage.json"));
        assert_eq!(usage.get_boost("editor.exe"), 8);
        let mut stats = crate::stats::StatsService::new(data.join("stats.json"));
        stats.set_enabled(true);
        assert_eq!(stats.snapshots()[0].total_switches, 9);
        fs::write(data.join("settings.json"), "user config").unwrap();
        let mut different = sample_bundle();
        different["source_profile_id"] = "different-profile".into();
        fs::write(&source, sign(different)).unwrap();
        assert!(
            import(&source, &data)
                .unwrap_err()
                .contains("different legacy import")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_tampered_exports_before_writing_profile_files() {
        let root = std::env::temp_dir().join(format!("omnispot-import-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("export.json");
        let mut value: serde_json::Value = serde_json::from_slice(&signed_sample()).unwrap();
        value["source_app_version"] = "tampered".into();
        fs::write(&source, serde_json::to_vec(&value).unwrap()).unwrap();
        let data = root.join("profile");
        assert!(import(&source, &data).unwrap_err().contains("integrity"));
        assert!(!data.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn applies_csharp_default_date_bucket_migration_on_import() {
        let mut value = sample_bundle();
        value["stats"]["buckets"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "date": "0001-01-01", "total_switches": 0, "total_time_saved_ms": 0,
                "total_peeks": 0, "static_app_usage": {}, "dynamic_app_usage": {},
                "alt_tab_switches": 0, "alt_tab_keystrokes": 0, "fastest_switch": null
            }));
        let bundle = parse_and_validate(&sign(value)).unwrap();
        assert_eq!(bundle.stats.buckets.len(), 1);
        assert_eq!(bundle.stats.buckets[0].date, "2026-11-05");
    }
}
