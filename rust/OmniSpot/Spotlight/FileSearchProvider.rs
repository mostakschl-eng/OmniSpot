use super::{SearchResult, SearchResultType};
use crate::window_discovery::IconExtractor;
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        Search::{
            DBBINDING, DBMEMOWNER_CLIENTOWNED, DBPARAMIO_NOTPARAM, DBPART_LENGTH, DBPART_STATUS,
            DBPART_VALUE, DBSTATUS_S_OK, DBTYPE_WSTR, HACCESSOR, IAccessor, ICommandText,
            IDBCreateCommand, IDBCreateSession, IDBInitialize, IDataInitialize, IRowset,
            MSDAINITIALIZE,
        },
    },
    core::{Interface, PCWSTR},
};

pub struct FileSearchProvider {
    roots: Vec<PathBuf>,
    icons: IconExtractor,
}
impl Default for FileSearchProvider {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            icons: IconExtractor,
        }
    }
}
impl FileSearchProvider {
    #[cfg(test)]
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            icons: IconExtractor,
        }
    }
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        if self.roots.is_empty() {
            let worker = std::thread::Builder::new()
                .name("omnispot-windows-search".into())
                .spawn(move || {
                    let icons = IconExtractor;
                    search_indexed_files(&query, max_results, &icons)
                });
            return worker
                .map_err(|error| error.to_string())
                .and_then(|worker| {
                    worker
                        .join()
                        .map_err(|_| "Windows Search worker panicked".to_string())?
                })
                .unwrap_or_else(|error| {
                    crate::logging::debug(format!("Windows Search query failed: {error}"));
                    Vec::new()
                });
        }
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let mut results = Vec::new();
        for root in &self.roots {
            self.visit(root, &terms, &mut results, max_results);
            if results.len() >= max_results {
                break;
            }
        }
        results.sort_by_key(|result| std::cmp::Reverse(result.score));
        results.truncate(max_results);
        results
    }
    fn visit(
        &self,
        path: &Path,
        terms: &[&str],
        results: &mut Vec<SearchResult>,
        max_results: usize,
    ) {
        if results.len() >= max_results {
            return;
        }
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            if results.len() >= max_results {
                return;
            }
            let path = entry.path();
            let name = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => continue,
            };
            if path.is_dir() {
                if !is_hidden_directory(name) {
                    self.visit(&path, terms, results, max_results);
                }
                continue;
            }
            if is_excluded_file(name) {
                continue;
            }
            let normalized = name.to_ascii_lowercase();
            if terms.iter().all(|term| normalized.contains(term)) {
                let score = if normalized.starts_with(terms[0]) {
                    90
                } else {
                    50
                };
                results.push(SearchResult {
                    title: name.to_string(),
                    subtitle: path.to_string_lossy().into_owned(),
                    target: path.to_string_lossy().into_owned(),
                    result_type: SearchResultType::File,
                    score,
                    icon_path: self.icons.get_by_path(&path),
                });
            }
        }
    }
}

const COLUMN_LIMIT: usize = 4096;
const ROW_BYTES: usize = 3 * (16 + COLUMN_LIMIT * 2);
const SEARCH_DIALECT: windows::core::GUID =
    windows::core::GUID::from_u128(0xc8b522bc_5cf3_11ce_ade5_00aa0044773d);

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn search_indexed_files(
    query: &str,
    max_results: usize,
    icons: &IconExtractor,
) -> Result<Vec<SearchResult>, String> {
    if max_results == 0 {
        return Ok(Vec::new());
    }
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if initialized.is_err() {
        return Err(format!("COM initialization failed: {initialized}"));
    }
    let _apartment = ComApartment;
    let query = indexed_query(query, max_results);
    let query = query.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let results = unsafe {
        let data_initializer: IDataInitialize =
            CoCreateInstance(&MSDAINITIALIZE, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("Windows Search provider unavailable: {error}"))?;
        let connection = "Provider=Search.CollatorDSO;Extended Properties='Application=Windows';";
        let connection = connection.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut data_source = None;
        data_initializer
            .GetDataSource(
                None,
                CLSCTX_INPROC_SERVER.0,
                PCWSTR(connection.as_ptr()),
                &IDBInitialize::IID,
                &mut data_source,
            )
            .map_err(|error| format!("Windows Search connection failed: {error}"))?;
        let data_source =
            data_source.ok_or_else(|| "Windows Search returned no data source".to_string())?;
        let initializer: IDBInitialize = data_source
            .cast()
            .map_err(|error| format!("Windows Search initializer unavailable: {error}"))?;
        initializer
            .Initialize()
            .map_err(|error| format!("Windows Search initialization failed: {error}"))?;
        let create_session: IDBCreateSession = data_source
            .cast()
            .map_err(|error| format!("Windows Search session unavailable: {error}"))?;
        let session = create_session
            .CreateSession(None, &IDBCreateCommand::IID)
            .map_err(|error| format!("Windows Search session failed: {error}"))?;
        let create_command: IDBCreateCommand = session
            .cast()
            .map_err(|error| format!("Windows Search command unavailable: {error}"))?;
        let command = create_command
            .CreateCommand(None, &ICommandText::IID)
            .map_err(|error| format!("Windows Search command creation failed: {error}"))?;
        let command: ICommandText = command
            .cast()
            .map_err(|error| format!("Windows Search SQL interface unavailable: {error}"))?;
        command
            .SetCommandText(&SEARCH_DIALECT, PCWSTR(query.as_ptr()))
            .map_err(|error| format!("Windows Search query setup failed: {error}"))?;
        let mut rowset = None;
        command
            .Execute(None, &IRowset::IID, None, None, Some(&mut rowset))
            .map_err(|error| format!("Windows Search query failed: {error}"))?;
        let rowset: IRowset = rowset
            .ok_or_else(|| "Windows Search returned no results interface".to_string())?
            .cast()
            .map_err(|error| format!("Windows Search result interface unavailable: {error}"))?;
        let accessor: IAccessor = rowset
            .cast()
            .map_err(|error| format!("Windows Search row accessor unavailable: {error}"))?;
        let bindings = (0..3)
            .map(|index| DBBINDING {
                iOrdinal: index + 1,
                obStatus: index * (16 + COLUMN_LIMIT * 2),
                obLength: index * (16 + COLUMN_LIMIT * 2) + 8,
                obValue: index * (16 + COLUMN_LIMIT * 2) + 16,
                dwPart: (DBPART_STATUS.0 | DBPART_LENGTH.0 | DBPART_VALUE.0) as u32,
                dwMemOwner: DBMEMOWNER_CLIENTOWNED.0 as u32,
                eParamIO: DBPARAMIO_NOTPARAM.0 as u32,
                cbMaxLen: COLUMN_LIMIT * 2,
                wType: DBTYPE_WSTR.0 as u16,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let mut binding_status = [0u32; 3];
        let mut accessor_handle = HACCESSOR::default();
        accessor
            .CreateAccessor(
                0,
                bindings.len(),
                bindings.as_ptr(),
                ROW_BYTES,
                &mut accessor_handle,
                Some(binding_status.as_mut_ptr()),
            )
            .map_err(|error| format!("Windows Search binding failed: {error}"))?;
        if binding_status.iter().any(|status| *status != 0) {
            let _ = accessor.ReleaseAccessor(accessor_handle, None);
            return Err("Windows Search rejected a result column binding".into());
        }
        let mut results = Vec::new();
        while results.len() < max_results {
            let mut row_handles = [std::ptr::null_mut(); 1];
            let mut fetched = 0usize;
            if rowset
                .GetNextRows(0, 0, &mut fetched, &mut row_handles)
                .is_err()
                || fetched == 0
            {
                break;
            }
            let mut row = vec![0u8; ROW_BYTES];
            let read_result = rowset.GetData(
                row_handles[0] as usize,
                accessor_handle,
                row.as_mut_ptr().cast(),
            );
            let _ = rowset.ReleaseRows(
                1,
                row_handles.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if read_result.is_err() {
                continue;
            }
            let Some(name) = read_bound_string(&row, 0) else {
                continue;
            };
            let path = read_bound_string(&row, 2)
                .or_else(|| read_bound_string(&row, 1).map(|url| decode_file_url(&url)));
            let Some(path) = path.filter(|path| !path.is_empty()) else {
                continue;
            };
            results.push(SearchResult::file(
                name,
                path.clone(),
                path.clone(),
                50,
                icons.get_by_path(Path::new(&path)),
            ));
        }
        let _ = accessor.ReleaseAccessor(accessor_handle, None);
        results
    };
    Ok(results)
}

fn indexed_query(query: &str, max_results: usize) -> String {
    let terms = query
        .split([' ', '\t'])
        .filter(|term| !term.is_empty())
        .map(|term| term.replace('\'', "''"))
        .map(|term| {
            format!("(System.ItemName LIKE '%{term}%' OR CONTAINS(System.ItemName, '\"{term}*\"'))")
        })
        .collect::<Vec<_>>();
    format!(
        "SELECT TOP {max_results} System.ItemName, System.ItemUrl, System.ItemPathDisplay FROM SystemIndex WHERE SCOPE='file:' AND {} AND System.ItemName NOT LIKE '%.dll' AND System.ItemName NOT LIKE '%.exe' AND System.ItemName NOT LIKE '%.sys' AND System.ItemName NOT LIKE '%.pdb' ORDER BY System.Search.Rank DESC",
        terms.join(" AND ")
    )
}

fn read_bound_string(row: &[u8], column: usize) -> Option<String> {
    let base = column * (16 + COLUMN_LIMIT * 2);
    let status = u32::from_ne_bytes(row.get(base..base + 4)?.try_into().ok()?);
    let byte_len = u64::from_ne_bytes(row.get(base + 8..base + 16)?.try_into().ok()?) as usize;
    if status != DBSTATUS_S_OK.0 as u32
        || byte_len > COLUMN_LIMIT * 2
        || !byte_len.is_multiple_of(2)
    {
        return None;
    }
    let units = byte_len / 2;
    let bytes = row.get(base + 16..base + 16 + units * 2)?;
    let wide = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Some(String::from_utf16_lossy(&wide))
}

fn decode_file_url(url: &str) -> String {
    let url = url.strip_prefix("file:").unwrap_or(url);
    let mut bytes = Vec::with_capacity(url.len());
    let mut index = 0;
    let encoded = url.as_bytes();
    while index < url.len() {
        if encoded[index] == b'%'
            && index + 2 < encoded.len()
            && let (Some(high), Some(low)) =
                (hex_digit(encoded[index + 1]), hex_digit(encoded[index + 2]))
        {
            let value = high * 16 + low;
            bytes.push(value);
            index += 3;
            continue;
        }
        let Some(character) = url[index..].chars().next() else {
            break;
        };
        let mut encoded = [0; 4];
        bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
        index += character.len_utf8();
    }
    let path = String::from_utf8_lossy(&bytes).replace('/', "\\");
    path.strip_prefix("\\\\\\")
        .map(str::to_string)
        .unwrap_or(path)
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
fn is_hidden_directory(name: &str) -> bool {
    matches!(name, "AppData" | "node_modules" | ".git" | "target") || name.starts_with('.')
}
fn is_excluded_file(name: &str) -> bool {
    [".dll", ".exe", ".sys", ".pdb"]
        .iter()
        .any(|extension| name.to_ascii_lowercase().ends_with(extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searches_files_and_ignores_executable_artifacts() {
        let root = std::env::temp_dir().join(format!("omnispot-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("notes.txt"), "test").unwrap();
        fs::write(root.join("notes.exe"), "test").unwrap();
        let results = FileSearchProvider::with_roots(vec![root.clone()]).search("notes", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result_type, SearchResultType::File);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn windows_search_query_matches_the_csharp_index_query_shape() {
        let sql = indexed_query("O'Brien report", 15);
        assert!(sql.starts_with(
            "SELECT TOP 15 System.ItemName, System.ItemUrl, System.ItemPathDisplay FROM SystemIndex"
        ));
        assert!(sql.contains("SCOPE='file:'"));
        assert!(sql.contains("O''Brien"));
        assert!(sql.contains("System.ItemName NOT LIKE '%.exe'"));
        assert!(sql.ends_with("ORDER BY System.Search.Rank DESC"));
    }

    #[test]
    fn windows_search_query_escapes_each_term_and_honors_result_limit() {
        let sql = indexed_query("one two'", 7);
        assert_eq!(sql.matches("System.ItemName LIKE").count(), 2);
        assert!(sql.contains("two''*"));
        assert!(sql.contains("SELECT TOP 7"));
    }

    #[test]
    fn file_urls_decode_spaces_unicode_and_local_drive_prefix() {
        assert_eq!(
            decode_file_url("file:///C:/My%20Files/%E4%B8%AD.txt"),
            "C:\\My Files\\中.txt"
        );
    }

    #[test]
    fn malformed_escapes_are_preserved_without_panicking() {
        assert_eq!(decode_file_url("file:/a%a中"), "\\a%a中");
    }
}
