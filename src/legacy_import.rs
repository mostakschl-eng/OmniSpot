use super::{decode_settings, Row};
use crate::stats::storage::DailyBucketDocument;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use vrc_get_litedb::{bson, file_io::LiteDBFile};

const PAGE_SIZE: usize = 8192;
const TRANSACTION_ID: usize = 14;
const CONFIRMED: usize = 18;

#[derive(Clone, Copy)]
pub(super) enum LegacyDatabase {
    Settings,
    Stats,
}

impl LegacyDatabase {
    pub(super) fn file_name(self) -> &'static str {
        match self {
            Self::Settings => "settings.db",
            Self::Stats => "stats.db",
        }
    }
}

pub(super) fn read(
    path: &Path,
    database: LegacyDatabase,
) -> Result<Option<Vec<Row>>, Box<dyn Error>> {
    let Some(mut source) = open_read_only(path)? else {
        return Ok(None);
    };
    let mut data = Vec::new();
    source.read_to_end(&mut data)?;
    if data.len() < PAGE_SIZE || data.len() % PAGE_SIZE != 0 {
        return Err("The legacy OmniSpot database has incomplete pages.".into());
    }
    let log = path.with_file_name(database.file_name().replace(".db", "-log.db"));
    if let Some(mut log) = open_read_only(&log)? {
        let mut journal = Vec::new();
        log.read_to_end(&mut journal)?;
        restore_committed(&mut data, &journal)?;
    }
    // The upstream parser has internal assertions for corrupt pages.
    let rows = std::panic::catch_unwind(|| {
        let file = LiteDBFile::parse(&data)?;
        match database {
            LegacyDatabase::Settings => read_settings(&file),
            LegacyDatabase::Stats => read_stats(&file),
        }
    })
    .map_err(|_| "The legacy OmniSpot database could not be decoded.")??;
    Ok(Some(rows))
}

fn open_read_only(path: &Path) -> std::io::Result<Option<File>> {
    // Hold the source handle while reading its journal so another process cannot checkpoint it.
    match OpenOptions::new().read(true).share_mode(1).open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn restore_committed(data: &mut Vec<u8>, journal: &[u8]) -> Result<(), Box<dyn Error>> {
    let page_limit = data.len() / PAGE_SIZE + journal.len() / PAGE_SIZE;
    let mut transactions: HashMap<u32, Vec<&[u8]>> = HashMap::new();
    for page in journal.chunks_exact(PAGE_SIZE) {
        if page.iter().all(|&byte| byte == 0) {
            continue;
        }
        let id = u32::from_le_bytes(page[TRANSACTION_ID..CONFIRMED].try_into().unwrap());
        transactions.entry(id).or_default().push(page);
        if page[CONFIRMED] != 1 {
            continue;
        }
        for image in transactions.remove(&id).unwrap() {
            let index = u32::from_le_bytes(image[..4].try_into().unwrap()) as usize;
            if index >= page_limit || image[4] > 4 {
                return Err(
                    "The legacy OmniSpot database journal contains an invalid page.".into(),
                );
            }
            let start = index * PAGE_SIZE;
            let end = start + PAGE_SIZE;
            if end > data.len() {
                data.resize(end, 0);
            }
            data[start..end].copy_from_slice(image);
        }
    }
    Ok(())
}

fn read_settings(file: &LiteDBFile) -> Result<Vec<Row>, Box<dyn Error>> {
    let mut rows = Vec::new();
    for document in file.get_all("settings") {
        if !matches!(
            document.get("_id"),
            bson::Value::Int32(1) | bson::Value::Int64(1)
        ) {
            continue;
        }
        let mut settings = json_document(document)?;
        settings.remove("_id");
        settings.insert("id".into(), 1.into());
        for (key, value) in [
            ("applications", Value::Array(Vec::new())),
            ("pulseBorderEnabled", false.into()),
            ("theme", 0.into()),
            ("overlayEnabled", false.into()),
            ("overlayShowDelayMs", 1000.into()),
            ("overlayKeepOpenWhileModifierHeld", false.into()),
            ("peekEnabled", false.into()),
            ("dynamicModeEnabled", true.into()),
            ("statsEnabled", false.into()),
            ("spotlightEnabled", true.into()),
            (
                "spotlightModifier",
                crate::input::keyboard_input::LEFT_ALT.into(),
            ),
            ("spotlightCustomDirectories", Value::Null),
        ] {
            settings.entry(key).or_insert(value);
        }
        if let Some(apps) = settings
            .get_mut("applications")
            .and_then(Value::as_array_mut)
        {
            let mut packages =
                crate::window_discovery::packaged_apps_service::PackagedAppsService::default();
            for app in apps {
                let app = app
                    .as_object_mut()
                    .ok_or("Invalid legacy application settings.")?;
                app.entry("startIfNotRunning").or_insert(false.into());
                app.entry("cycleMode").or_insert(0.into());
                app.entry("type").or_insert(0.into());
                let path = app
                    .get("processPath")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let win32 = app
                    .get("type")
                    .is_some_and(|kind| kind == 0 || kind == "Win32");
                if win32
                    && crate::configuration::packaged_app_path_sanitizer::get_package_family_name(
                        path,
                    )
                    .is_some()
                {
                    if let Ok(Some(info)) = packages.get_by_installed_path(path, None) {
                        app.insert("type".into(), 1.into());
                        app.insert("aumid".into(), info.aumid.into());
                    }
                }
            }
        }
        let value = Value::Object(settings);
        decode_settings(&value.to_string())?;
        rows.push(Row {
            collection: "settings".into(),
            key: "1".into(),
            value,
        });
    }
    for document in file.get_all("app_registry") {
        let bson::Value::String(key) = document.get("_id") else {
            return Err("Invalid legacy app registry key.".into());
        };
        let mut fields = json_document(document)?;
        fields.remove("_id");
        fields.insert("processName".into(), key.clone().into());
        rows.push(Row {
            collection: "app_registry".into(),
            key: key.clone(),
            value: fields.into(),
        });
    }
    Ok(rows)
}

fn read_stats(file: &LiteDBFile) -> Result<Vec<Row>, Box<dyn Error>> {
    let mut rows = Vec::new();
    for document in file.get_all("daily_buckets") {
        let key = match document.get("_id") {
            bson::Value::String(key) => key.clone(),
            bson::Value::DateTime(utc) => local_date(utc)?,
            _ => return Err("Invalid legacy usage history date.".into()),
        };
        if key == "0001-01-01" {
            continue;
        }
        use crate::stats::stats_calculator::{days_to_ymd, parse_ymd, ymd_to_days};
        let valid = parse_ymd(&key)
            .filter(|&(_, month, day)| (1..=12).contains(&month) && (1..=31).contains(&day))
            .is_some_and(|(year, month, day)| {
                (1..=9999).contains(&year) && days_to_ymd(ymd_to_days(year, month, day)) == key
            });
        if !valid {
            return Err("Invalid legacy usage history date.".into());
        }
        let mut bucket = json_document(document)?;
        bucket.remove("_id");
        bucket.insert("date".into(), key.clone().into());
        let value = Value::Object(bucket);
        serde_json::from_value::<DailyBucketDocument>(value.clone())?;
        rows.push(Row {
            collection: "daily_buckets".into(),
            key,
            value,
        });
    }
    Ok(rows)
}

fn json_document(document: &bson::Document) -> Result<Map<String, Value>, Box<dyn Error>> {
    document
        .iter()
        .map(|(key, value)| {
            let mut name = key.to_owned();
            if let Some(first) = name.get_mut(..1) {
                first.make_ascii_lowercase();
            }
            let value = match (name.as_str(), value) {
                ("staticAppUsage" | "dynamicAppUsage", bson::Value::Document(apps)) => {
                    let apps = apps
                        .iter()
                        .map(|(path, value)| Ok((path.to_owned(), json_value(value)?)))
                        .collect::<Result<Map<String, Value>, Box<dyn Error>>>()?;
                    Value::Object(apps)
                }
                _ => json_value(value)?,
            };
            Ok((name, value))
        })
        .collect()
}

fn json_value(value: &bson::Value) -> Result<Value, Box<dyn Error>> {
    Ok(match value {
        bson::Value::Null => Value::Null,
        bson::Value::Int32(value) => (*value).into(),
        bson::Value::Int64(value) => (*value).into(),
        bson::Value::String(value) => value.clone().into(),
        bson::Value::Boolean(value) => (*value).into(),
        bson::Value::Array(values) => {
            Value::Array(values.iter().map(json_value).collect::<Result<_, _>>()?)
        }
        bson::Value::Document(document) => Value::Object(json_document(document)?),
        bson::Value::DateTime(utc) => format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            utc.year(),
            utc.month(),
            utc.day(),
            utc.hour(),
            utc.minute(),
            utc.second(),
            utc.ticks() / 10_000 % 1000,
        )
        .into(),
        _ => return Err("The legacy OmniSpot database contains an unsupported BSON value.".into()),
    })
}

fn local_date(utc: &bson::DateTime) -> Result<String, Box<dyn Error>> {
    use windows::Win32::Foundation::SYSTEMTIME;
    use windows::Win32::System::Time::SystemTimeToTzSpecificLocalTimeEx;
    if utc.year() == 1 && utc.month() == 1 && utc.day() == 1 {
        return Ok("0001-01-01".into());
    }
    let time = SYSTEMTIME {
        wYear: utc.year() as u16,
        wMonth: utc.month() as u16,
        wDay: utc.day() as u16,
        wHour: utc.hour() as u16,
        wMinute: utc.minute() as u16,
        wSecond: utc.second() as u16,
        ..Default::default()
    };
    let mut local = SYSTEMTIME::default();
    unsafe { SystemTimeToTzSpecificLocalTimeEx(None, &time, &mut local)? };
    Ok(format!(
        "{:04}-{:02}-{:02}",
        local.wYear, local.wMonth, local.wDay
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commits_replace_data_pages_and_uncommitted_pages_are_ignored() {
        let mut data = vec![0; PAGE_SIZE];
        let mut journal = vec![0; PAGE_SIZE * 3];
        for (index, page) in journal.chunks_exact_mut(PAGE_SIZE).enumerate() {
            page[4] = 1;
            page[TRANSACTION_ID] = (index + 1) as u8;
            page[32] = (index + 1) as u8;
        }
        journal[CONFIRMED] = 1;
        journal[PAGE_SIZE + CONFIRMED] = 1;
        restore_committed(&mut data, &journal).unwrap();
        assert_eq!(data[32], 2);
    }
}
