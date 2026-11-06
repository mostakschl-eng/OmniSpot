use std::{
    fs::{self, File},
    io::{self, Write},
    os::windows::ffi::OsStrExt,
    path::Path,
};
use windows::Win32::Storage::FileSystem::{
    MOVE_FILE_FLAGS, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows::core::PCWSTR;

pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file path has no name"))?;
    let temp = path.with_file_name(format!(
        "{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        let source = temp
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(destination.as_ptr()),
                MOVE_FILE_FLAGS(MOVEFILE_REPLACE_EXISTING.0 | MOVEFILE_WRITE_THROUGH.0),
            )
        }
        .map_err(io::Error::other)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::write;
    use std::fs;

    #[test]
    fn replaces_a_file_and_preserves_the_old_destination_on_failure() {
        let base = std::env::temp_dir().join(format!("omnispot-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let target = base.join("settings.json");
        fs::write(&target, "old").unwrap();
        write(&target, b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");

        let blocked = base.join("blocked.json");
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("keep"), "preserved").unwrap();
        assert!(write(&blocked, b"replacement").is_err());
        assert_eq!(
            fs::read_to_string(blocked.join("keep")).unwrap(),
            "preserved"
        );
        let _ = fs::remove_dir_all(base);
    }
}
