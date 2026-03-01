use chrono::{DateTime, Local};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryType {
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub entry_type: EntryType,
    pub size: u64,
    pub modified: Option<DateTime<Local>>,
    pub is_parent: bool,
}

impl FileEntry {
    pub fn parent_entry(path: &Path) -> Self {
        Self {
            name: "..".to_string(),
            path: path.parent().unwrap_or(path).to_path_buf(),
            entry_type: EntryType::Directory,
            size: 0,
            modified: None,
            is_parent: true,
        }
    }

    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        let entry_type = if metadata.is_symlink() {
            EntryType::Symlink
        } else if metadata.is_dir() {
            EntryType::Directory
        } else {
            EntryType::File
        };

        let modified = metadata
            .modified()
            .ok()
            .map(|t| DateTime::<Local>::from(t));

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(Self {
            name,
            path: path.to_path_buf(),
            entry_type,
            size: metadata.len(),
            modified,
            is_parent: false,
        })
    }

    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
    }

    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    pub fn is_executable(&self) -> bool {
        if self.is_dir() || self.is_parent {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&self.path) {
                return meta.permissions().mode() & 0o111 != 0;
            }
        }
        false
    }

    pub fn is_archive(&self) -> bool {
        const ARCHIVE_EXTS: &[&str] = &[
            ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".rar", ".tgz",
            ".tar.gz", ".tar.bz2", ".tar.xz", ".zst", ".lz4",
        ];
        let lower = self.name.to_lowercase();
        ARCHIVE_EXTS.iter().any(|ext| lower.ends_with(ext))
    }

    pub fn is_symlink(&self) -> bool {
        self.entry_type == EntryType::Symlink
    }

    pub fn display_permissions(&self) -> String {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::symlink_metadata(&self.path) {
                let mode = meta.permissions().mode();
                let mut s = String::with_capacity(9);
                s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
                s.push(if mode & 0o200 != 0 { 'w' } else { '-' });
                s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
                s.push(if mode & 0o040 != 0 { 'r' } else { '-' });
                s.push(if mode & 0o020 != 0 { 'w' } else { '-' });
                s.push(if mode & 0o010 != 0 { 'x' } else { '-' });
                s.push(if mode & 0o004 != 0 { 'r' } else { '-' });
                s.push(if mode & 0o002 != 0 { 'w' } else { '-' });
                s.push(if mode & 0o001 != 0 { 'x' } else { '-' });
                return s;
            }
        }
        "---------".to_string()
    }

    pub fn display_size(&self) -> String {
        if self.is_dir() {
            "<DIR>".to_string()
        } else if self.size >= 1_073_741_824 {
            format!("{:.1}G", self.size as f64 / 1_073_741_824.0)
        } else if self.size >= 1_048_576 {
            format!("{:.1}M", self.size as f64 / 1_048_576.0)
        } else if self.size >= 1024 {
            format!("{}K", self.size / 1024)
        } else {
            format!("{}", self.size)
        }
    }

    pub fn display_date(&self) -> String {
        match &self.modified {
            Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
            None => String::new(),
        }
    }
}

pub fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| {
        if a.is_parent {
            return Ordering::Less;
        }
        if b.is_parent {
            return Ordering::Greater;
        }
        match (a.is_dir(), b.is_dir()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
}

pub fn read_directory(path: &Path, show_hidden: bool) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    if path.parent().is_some() {
        entries.push(FileEntry::parent_entry(path));
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_path = entry.path();
        match FileEntry::from_path(&file_path) {
            Ok(fe) => {
                if !show_hidden && fe.is_hidden() {
                    continue;
                }
                entries.push(fe);
            }
            Err(_) => continue,
        }
    }

    sort_entries(&mut entries);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("alpha_dir")).unwrap();
        fs::create_dir(dir.path().join("beta_dir")).unwrap();
        fs::write(dir.path().join("charlie.txt"), "hello").unwrap();
        fs::write(dir.path().join("able.rs"), "fn main(){}").unwrap();
        fs::write(dir.path().join(".hidden"), "secret").unwrap();
        dir
    }

    #[test]
    fn test_sort_directories_first() {
        let dir = create_test_dir();
        let entries = read_directory(dir.path(), false).unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names[0], "..");
        assert!(entries[1].is_dir());
        assert!(entries[2].is_dir());
    }

    #[test]
    fn test_hidden_files_filtered() {
        let dir = create_test_dir();
        let entries = read_directory(dir.path(), false).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&".hidden"));
    }

    #[test]
    fn test_hidden_files_shown() {
        let dir = create_test_dir();
        let entries = read_directory(dir.path(), true).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".hidden"));
    }

    #[test]
    fn test_parent_entry_first() {
        let dir = create_test_dir();
        let entries = read_directory(dir.path(), false).unwrap();
        assert!(entries[0].is_parent);
        assert_eq!(entries[0].name, "..");
    }

    #[test]
    fn test_alphabetical_sort_within_type() {
        let dir = create_test_dir();
        let entries = read_directory(dir.path(), false).unwrap();
        let dirs: Vec<&str> = entries
            .iter()
            .filter(|e| e.is_dir() && !e.is_parent)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(dirs, vec!["alpha_dir", "beta_dir"]);

        let files: Vec<&str> = entries
            .iter()
            .filter(|e| !e.is_dir())
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(files, vec!["able.rs", "charlie.txt"]);
    }

    #[test]
    fn test_display_size() {
        let entry = FileEntry {
            name: "test".to_string(),
            path: PathBuf::from("test"),
            entry_type: EntryType::File,
            size: 2048,
            modified: None,
            is_parent: false,
        };
        assert_eq!(entry.display_size(), "2K");

        let dir_entry = FileEntry {
            name: "dir".to_string(),
            path: PathBuf::from("dir"),
            entry_type: EntryType::Directory,
            size: 0,
            modified: None,
            is_parent: false,
        };
        assert_eq!(dir_entry.display_size(), "<DIR>");
    }

    #[test]
    fn test_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let entries = read_directory(dir.path(), false).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_parent);
    }

    // --- Phase 3 테스트 ---

    #[test]
    fn test_is_archive() {
        let archives = vec!["file.zip", "data.tar.gz", "img.7z", "backup.rar", "src.tar.xz"];
        for name in archives {
            let entry = FileEntry {
                name: name.to_string(),
                path: PathBuf::from(name),
                entry_type: EntryType::File,
                size: 100,
                modified: None,
                is_parent: false,
            };
            assert!(entry.is_archive(), "{} should be archive", name);
        }

        let non_archives = vec!["readme.txt", "main.rs", "photo.jpg"];
        for name in non_archives {
            let entry = FileEntry {
                name: name.to_string(),
                path: PathBuf::from(name),
                entry_type: EntryType::File,
                size: 100,
                modified: None,
                is_parent: false,
            };
            assert!(!entry.is_archive(), "{} should not be archive", name);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_is_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let exec_path = dir.path().join("run.sh");
        fs::write(&exec_path, "#!/bin/sh").unwrap();
        fs::set_permissions(&exec_path, fs::Permissions::from_mode(0o755)).unwrap();

        let entry = FileEntry::from_path(&exec_path).unwrap();
        assert!(entry.is_executable());

        let normal_path = dir.path().join("data.txt");
        fs::write(&normal_path, "hello").unwrap();
        fs::set_permissions(&normal_path, fs::Permissions::from_mode(0o644)).unwrap();

        let entry2 = FileEntry::from_path(&normal_path).unwrap();
        assert!(!entry2.is_executable());
    }

    #[cfg(unix)]
    #[test]
    fn test_display_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let entry = FileEntry::from_path(&path).unwrap();
        assert_eq!(entry.display_permissions(), "rw-r--r--");

        let exec_path = dir.path().join("run.sh");
        fs::write(&exec_path, "#!/bin/sh").unwrap();
        fs::set_permissions(&exec_path, fs::Permissions::from_mode(0o755)).unwrap();

        let entry2 = FileEntry::from_path(&exec_path).unwrap();
        assert_eq!(entry2.display_permissions(), "rwxr-xr-x");
    }

    #[test]
    fn test_is_symlink() {
        let entry = FileEntry {
            name: "link".to_string(),
            path: PathBuf::from("link"),
            entry_type: EntryType::Symlink,
            size: 0,
            modified: None,
            is_parent: false,
        };
        assert!(entry.is_symlink());

        let entry2 = FileEntry {
            name: "file.txt".to_string(),
            path: PathBuf::from("file.txt"),
            entry_type: EntryType::File,
            size: 0,
            modified: None,
            is_parent: false,
        };
        assert!(!entry2.is_symlink());
    }
}
