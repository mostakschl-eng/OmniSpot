use crate::configuration::configuration::Configuration;
use crate::configuration::configuration_validator::ConfigurationValidator;
use crate::configuration::packaged_app_path_sanitizer::{
    IPackagedAppsService, PackagedAppInfo, PackagedAppPathSanitizer,
};
use crate::stats::session_stats::SessionStats;
use crate::stats::storage::DailyBucketDocument;
use crate::window_discovery::packaged_apps_service::PackagedAppsService;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::Value;
use std::{error::Error, path::PathBuf, time::Duration};

#[path = "legacy_import.rs"]
mod legacy_import;
use legacy_import::LegacyDatabase;

#[derive(Deserialize)]
struct Row {
    collection: String,
    key: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase")]
enum Operation {
    Ready,
    Initialize {
        rows: Vec<Row>,
    },
    Read {
        collection: String,
        key: Option<String>,
    },
    Write {
        rows: Vec<Row>,
        overwrite: bool,
    },
    Clear {
        collection: String,
    },
    LoadSettings {
        sanitize: bool,
    },
    SaveSettings {
        configuration: Configuration,
    },
    StatsHistory {
        today: String,
    },
    FlushStats,
    RestoreStats {
        today: String,
    },
    ResetStats,
}

#[derive(Deserialize)]
struct Request {
    path: PathBuf,
    #[serde(flatten)]
    operation: Operation,
}

pub(crate) fn execute(request: &[u8]) -> Result<Value, Box<dyn Error>> {
    let Request { path, operation } = serde_json::from_slice(request)?;
    execute_operation(path, operation)
}

fn execute_operation(path: PathBuf, operation: Operation) -> Result<Value, Box<dyn Error>> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
    match &operation {
        Operation::Ready if !path.try_exists()? => return Ok(Value::Bool(false)),
        Operation::Initialize { .. } => {
            if let Some(directory) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(directory)?;
            }
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        _ => {}
    }
    let mut db = Connection::open_with_flags(path, flags)?;
    db.busy_timeout(Duration::from_secs(5))?;
    if let Operation::Initialize { rows } = operation {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        match version(&tx)? {
            0 => {
                tx.execute_batch(
                    "CREATE TABLE documents (
                        collection TEXT NOT NULL,
                        key TEXT NOT NULL COLLATE NOCASE,
                        value TEXT NOT NULL,
                        PRIMARY KEY (collection, key)
                    ) WITHOUT ROWID;",
                )?;
                write(&tx, rows, None)?;
                tx.pragma_update(None, "user_version", 1)?;
            }
            1 => {}
            _ => return Err("Unsupported OmniSpot database schema".into()),
        }
        tx.commit()?;
        return Ok(Value::Null);
    }
    let version = version(&db)?;
    if matches!(operation, Operation::Ready) && version == 0 {
        return Ok(Value::Bool(false));
    }
    if version != 1 {
        return Err("Unsupported or uninitialized OmniSpot database schema".into());
    }
    match operation {
        Operation::Ready => return Ok(Value::Bool(true)),
        Operation::Read { collection, key } => {
            let mut query = db.prepare(
                "SELECT value FROM documents WHERE collection = ?1 AND (?2 IS NULL OR key = ?2)
                 ORDER BY key",
            )?;
            let rows = query.query_map(params![collection, key], |row| row.get::<_, String>(0))?;
            let values = rows
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect::<Result<Vec<Value>, Box<dyn Error>>>()?;
            return Ok(Value::Array(values));
        }
        Operation::Write { rows, overwrite } => {
            let tx = db.transaction()?;
            write(&tx, rows, Some(overwrite))?;
            tx.commit()?;
        }
        Operation::Clear { collection } => {
            db.execute("DELETE FROM documents WHERE collection = ?1", [collection])?;
        }
        Operation::LoadSettings { sanitize } => {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let stored: Option<String> = tx
                .query_row(
                    "SELECT value FROM documents WHERE collection = 'settings' AND key = '1'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            let mut config = stored
                .as_deref()
                .map(decode_settings)
                .transpose()?
                .unwrap_or_default();
            let corrected = sanitize
                .then(|| {
                    PackagedAppPathSanitizer::new(InstalledPackages::default()).sanitize(&config)
                })
                .flatten();
            if stored.is_none() || corrected.is_some() {
                config = corrected.unwrap_or(config);
                save_settings(&tx, config.clone())?;
            }
            tx.commit()?;
            let validation = ConfigurationValidator::validate(&config);
            return Ok(serde_json::json!({
                "configuration": config,
                "validationMessage": validation.message.map(|message| {
                    validation.process.map_or_else(|| message.clone(), |process| format!("[{process}] {message}"))
                })
            }));
        }
        Operation::SaveSettings { configuration } => save_settings(&db, configuration)?,
        Operation::StatsHistory { today } => {
            let mut query = db.prepare(
                "SELECT value FROM documents WHERE collection = 'daily_buckets' AND key < ?1
                 ORDER BY key DESC",
            )?;
            let rows = query.query_map([today], |row| row.get::<_, String>(0))?;
            let values = rows
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect::<Result<Vec<Value>, Box<dyn Error>>>()?;
            return Ok(Value::Array(values));
        }
        Operation::FlushStats => {
            let session = crate::ffi::STATS_SESSION
                .get_or_init(|| std::sync::Mutex::new(SessionStats::new()));
            let session = session.lock().map_err(|_| "Stats session is unavailable")?;
            let snapshots = session.get_snapshots();
            let has_rollover = snapshots.len() > 1;
            let rows = snapshots
                .into_iter()
                .filter(|bucket| !bucket.date.is_empty() && bucket.date != "0001-01-01")
                .map(|bucket| {
                    Ok(Row {
                        collection: "daily_buckets".into(),
                        key: bucket.date.clone(),
                        value: serde_json::to_value(bucket)?,
                    })
                })
                .collect::<serde_json::Result<Vec<_>>>()?;
            let count = rows.len();
            let tx = db.transaction()?;
            write(&tx, rows, Some(true))?;
            tx.commit()?;
            if has_rollover {
                session.clear_rollover();
            }
            return Ok(Value::from(count));
        }
        Operation::RestoreStats { today } => {
            let stored: Option<String> = db
                .query_row(
                    "SELECT value FROM documents WHERE collection = 'daily_buckets' AND key = ?1",
                    [today],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(json) = stored {
                let bucket: DailyBucketDocument = serde_json::from_str(&json)?;
                let session = crate::ffi::STATS_SESSION
                    .get_or_init(|| std::sync::Mutex::new(SessionStats::new()));
                session
                    .lock()
                    .map_err(|_| "Stats session is unavailable")?
                    .load_from(&bucket);
            }
        }
        Operation::ResetStats => {
            let session = crate::ffi::STATS_SESSION
                .get_or_init(|| std::sync::Mutex::new(SessionStats::new()));
            let session = session.lock().map_err(|_| "Stats session is unavailable")?;
            db.execute(
                "DELETE FROM documents WHERE collection = 'daily_buckets'",
                [],
            )?;
            session.clear();
        }
        Operation::Initialize { .. } => unreachable!(),
    }
    Ok(Value::Null)
}

pub(crate) fn load_desktop_settings(path: PathBuf) -> Result<Configuration, Box<dyn Error>> {
    initialize_desktop(path.clone(), LegacyDatabase::Settings, true)?;
    let result = execute_operation(path, Operation::LoadSettings { sanitize: false })?;
    Ok(serde_json::from_value(result["configuration"].clone())?)
}

pub(crate) fn desktop_stats_buckets(
    path: PathBuf,
) -> Result<Vec<DailyBucketDocument>, Box<dyn Error>> {
    if !initialize_desktop(path.clone(), LegacyDatabase::Stats, false)? {
        return Ok(Vec::new());
    }
    let buckets = execute_operation(
        path,
        Operation::StatsHistory {
            today: "9999-12-31".into(),
        },
    )?;
    Ok(serde_json::from_value(buckets)?)
}

pub(crate) fn clear_desktop_stats(path: PathBuf) -> Result<(), Box<dyn Error>> {
    if initialize_desktop(path.clone(), LegacyDatabase::Stats, false)? {
        execute_operation(
            path,
            Operation::Clear {
                collection: "daily_buckets".into(),
            },
        )?;
    }
    Ok(())
}

pub(crate) fn prepare_desktop_stats(path: PathBuf) -> Result<(), Box<dyn Error>> {
    initialize_desktop(path.clone(), LegacyDatabase::Stats, true)?;
    execute_operation(
        path,
        Operation::RestoreStats {
            today: crate::stats::session_stats::system_today(),
        },
    )?;
    Ok(())
}

pub(crate) fn import_desktop_stats(path: PathBuf) -> Result<(), Box<dyn Error>> {
    initialize_desktop(path, LegacyDatabase::Stats, false)?;
    Ok(())
}

fn initialize_desktop(
    path: PathBuf,
    database: LegacyDatabase,
    create: bool,
) -> Result<bool, Box<dyn Error>> {
    if execute_operation(path.clone(), Operation::Ready)?.as_bool() == Some(true) {
        return Ok(true);
    }
    let legacy_path = path.with_file_name(database.file_name());
    let rows = match legacy_import::read(&legacy_path, database)? {
        Some(rows) => rows,
        None if create => Vec::new(),
        None => return Ok(false),
    };
    execute_operation(path, Operation::Initialize { rows })?;
    Ok(true)
}

pub(crate) fn flush_desktop_stats(path: PathBuf) -> Result<(), Box<dyn Error>> {
    execute_operation(path, Operation::FlushStats)?;
    Ok(())
}

pub(crate) fn read_desktop_registry(
    path: PathBuf,
) -> Result<std::collections::HashMap<String, String>, Box<dyn Error>> {
    let value = execute_operation(path, Operation::Read {
        collection: "app_registry".into(),
        key: None,
    })?;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Entry { process_name: String, display_name: String }
    Ok(serde_json::from_value::<Vec<Entry>>(value)?.into_iter()
        .map(|entry| (entry.process_name.to_lowercase(), entry.display_name)).collect())
}

pub(crate) fn persist_desktop_registry(path: PathBuf) -> Result<(), Box<dyn Error>> {
    let rows: Vec<_> = crate::ffi::desktop_registry().into_iter().map(|(key, name)| Row {
        collection: "app_registry".into(),
        value: serde_json::json!({ "processName": key, "displayName": name }),
        key,
    }).collect();
    if !rows.is_empty() {
        execute_operation(path, Operation::Write { rows, overwrite: false })?;
    }
    Ok(())
}

pub(crate) fn reset_running_desktop_stats(path: PathBuf) -> Result<(), Box<dyn Error>> {
    execute_operation(path, Operation::ResetStats)?;
    Ok(())
}

pub(crate) fn save_desktop_settings(
    path: PathBuf,
    configuration: Configuration,
) -> Result<(), Box<dyn Error>> {
    use crate::configuration::configuration::CycleMode;

    let result = ConfigurationValidator::validate(&configuration);
    if let Some(message) = result.message {
        return Err(message.into());
    }
    let mut keys = std::collections::HashMap::new();
    for app in &configuration.applications {
        let next_app = app.cycle_mode == CycleMode::NextApp;
        if let Some(previous) = keys.insert(app.key, next_app) {
            if !previous || !next_app {
                return Err("Multiple apps with the same key are only allowed if they all are in Next App mode".into());
            }
        }
    }
    execute_operation(path, Operation::SaveSettings { configuration })?;
    Ok(())
}

#[derive(Default)]
struct InstalledPackages(std::cell::RefCell<PackagedAppsService>);

impl IPackagedAppsService for InstalledPackages {
    fn get_by_package_family_name(&self, family_name: &str) -> Option<PackagedAppInfo> {
        self.0
            .borrow_mut()
            .get_by_package_family_name(family_name)
            .ok()
            .flatten()
            .map(|package| PackagedAppInfo {
                aumid: package.aumid,
                icon_path: Some(package.icon_path),
            })
    }

    fn get_installed_path_by_aumid(&self, aumid: &str) -> Option<String> {
        self.0
            .borrow()
            .get_installed_path_by_aumid(Some(aumid))
            .ok()
            .flatten()
    }
}

fn save_settings(db: &Connection, mut configuration: Configuration) -> Result<(), Box<dyn Error>> {
    if configuration
        .spotlight_custom_directories
        .as_ref()
        .is_some_and(Vec::is_empty)
    {
        configuration.spotlight_custom_directories = None;
    }
    let mut value = serde_json::to_value(&configuration)?;
    value["id"] = Value::from(1);
    value["theme"] = Value::from(configuration.theme as u32);
    for (index, app) in configuration.applications.iter().enumerate() {
        let fields = value["applications"][index]
            .as_object_mut()
            .ok_or("Invalid serialized application settings")?;
        fields.remove("applicationType");
        fields.insert("cycleMode".into(), Value::from(app.cycle_mode as u32));
        fields.insert("type".into(), Value::from(app.application_type as u32));
    }
    write(
        db,
        vec![Row {
            collection: "settings".into(),
            key: "1".into(),
            value,
        }],
        Some(true),
    )?;
    Ok(())
}

fn decode_settings(json: &str) -> Result<Configuration, Box<dyn Error>> {
    let mut value: Value = serde_json::from_str(json)?;
    // The first SQLite release and the LiteDB importer use numeric WPF enums.
    if let Some(theme) = value.get_mut("theme") {
        decode_enum(theme, &["System", "Dark", "Light"]);
    }
    if let Some(applications) = value.get_mut("applications").and_then(Value::as_array_mut) {
        for app in applications {
            let fields = app
                .as_object_mut()
                .ok_or("Invalid stored application settings")?;
            if let Some(mode) = fields.get_mut("cycleMode") {
                decode_enum(mode, &["NextApp", "Hide", "NextWindow"]);
            }
            if !fields.contains_key("applicationType") {
                let mut kind = fields
                    .remove("type")
                    .unwrap_or_else(|| Value::from("Win32"));
                decode_enum(&mut kind, &["Win32", "Packaged"]);
                fields.insert("applicationType".into(), kind);
            }
        }
    }
    Ok(serde_json::from_value(value)?)
}

fn decode_enum(value: &mut Value, names: &[&str]) {
    if let Some(name) = value.as_u64().and_then(|index| names.get(index as usize)) {
        *value = Value::from(*name);
    }
}

fn version(db: &Connection) -> rusqlite::Result<i64> {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn write(db: &Connection, rows: Vec<Row>, overwrite: Option<bool>) -> rusqlite::Result<()> {
    let sql = match overwrite {
        None => "INSERT INTO documents VALUES (?1, ?2, ?3)",
        Some(true) => {
            "INSERT INTO documents VALUES (?1, ?2, ?3)
                       ON CONFLICT (collection, key) DO UPDATE SET value = excluded.value"
        }
        Some(false) => {
            "INSERT INTO documents VALUES (?1, ?2, ?3)
                        ON CONFLICT (collection, key) DO NOTHING"
        }
    };
    let mut statement = db.prepare(sql)?;
    for row in rows {
        statement.execute(params![row.collection, row.key, row.value.to_string()])?;
    }
    Ok(())
}
