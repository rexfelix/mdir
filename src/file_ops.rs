use std::fs;
use std::path::Path;

/// 파일/디렉토리를 대상 경로로 복사한다.
/// 디렉토리인 경우 재귀적으로 복사한다.
pub fn copy_entries(sources: &[&Path], dest_dir: &Path) -> Result<(), String> {
    if !dest_dir.is_dir() {
        return Err(format!(
            "대상 경로가 디렉토리가 아닙니다: {}",
            dest_dir.display()
        ));
    }

    for source in sources {
        let file_name = source
            .file_name()
            .ok_or_else(|| format!("파일명을 가져올 수 없습니다: {}", source.display()))?;
        let target = dest_dir.join(file_name);

        if target.exists() {
            return Err(format!("대상이 이미 존재합니다: {}", target.display()));
        }

        if source.is_dir() {
            copy_dir_recursive(source, &target)?;
        } else {
            fs::copy(source, &target)
                .map_err(|e| format!("복사 실패 {}: {}", source.display(), e))?;
        }
    }
    Ok(())
}

/// 파일/디렉토리를 대상 경로로 이동한다.
pub fn move_entries(sources: &[&Path], dest_dir: &Path) -> Result<(), String> {
    if !dest_dir.is_dir() {
        return Err(format!(
            "대상 경로가 디렉토리가 아닙니다: {}",
            dest_dir.display()
        ));
    }

    for source in sources {
        let file_name = source
            .file_name()
            .ok_or_else(|| format!("파일명을 가져올 수 없습니다: {}", source.display()))?;
        let target = dest_dir.join(file_name);

        if target.exists() {
            return Err(format!("대상이 이미 존재합니다: {}", target.display()));
        }

        // rename 시도, 실패 시 copy + delete
        if fs::rename(source, &target).is_err() {
            if source.is_dir() {
                copy_dir_recursive(source, &target)?;
                fs::remove_dir_all(source)
                    .map_err(|e| format!("원본 삭제 실패 {}: {}", source.display(), e))?;
            } else {
                fs::copy(source, &target)
                    .map_err(|e| format!("복사 실패 {}: {}", source.display(), e))?;
                fs::remove_file(source)
                    .map_err(|e| format!("원본 삭제 실패 {}: {}", source.display(), e))?;
            }
        }
    }
    Ok(())
}

/// 파일/디렉토리를 삭제한다.
pub fn delete_entries(targets: &[&Path]) -> Result<(), String> {
    for target in targets {
        if target.is_dir() {
            fs::remove_dir_all(target)
                .map_err(|e| format!("삭제 실패 {}: {}", target.display(), e))?;
        } else {
            fs::remove_file(target)
                .map_err(|e| format!("삭제 실패 {}: {}", target.display(), e))?;
        }
    }
    Ok(())
}

/// 파일/디렉토리 이름을 변경한다.
pub fn rename_entry(source: &Path, new_name: &str) -> Result<(), String> {
    if new_name.is_empty() {
        return Err("새 이름이 비어있습니다".to_string());
    }
    if new_name.contains('/') || new_name.contains('\\') {
        return Err("이름에 경로 구분자를 포함할 수 없습니다".to_string());
    }

    let parent = source
        .parent()
        .ok_or_else(|| "상위 디렉토리를 찾을 수 없습니다".to_string())?;
    let target = parent.join(new_name);

    if target.exists() {
        return Err(format!("이미 존재하는 이름입니다: {}", new_name));
    }

    fs::rename(source, &target)
        .map_err(|e| format!("이름 변경 실패: {}", e))?;
    Ok(())
}

/// 새 디렉토리를 생성한다.
pub fn create_directory(parent: &Path, name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("디렉토리명이 비어있습니다".to_string());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("이름에 경로 구분자를 포함할 수 없습니다".to_string());
    }

    let target = parent.join(name);
    if target.exists() {
        return Err(format!("이미 존재합니다: {}", name));
    }

    fs::create_dir(&target)
        .map_err(|e| format!("디렉토리 생성 실패: {}", e))?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir(dst)
        .map_err(|e| format!("디렉토리 생성 실패 {}: {}", dst.display(), e))?;

    let entries = fs::read_dir(src)
        .map_err(|e| format!("디렉토리 읽기 실패 {}: {}", src.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("엔트리 읽기 실패: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("복사 실패 {}: {}", src_path.display(), e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file_a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("file_b.txt"), "bbb").unwrap();
        fs::create_dir(dir.path().join("sub_dir")).unwrap();
        fs::write(dir.path().join("sub_dir/inner.txt"), "inner").unwrap();
        dir
    }

    // --- copy ---

    #[test]
    fn test_copy_file() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();

        let sources = vec![src_dir.path().join("file_a.txt")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        copy_entries(&source_refs, dest_dir.path()).unwrap();

        assert!(dest_dir.path().join("file_a.txt").exists());
        assert_eq!(
            fs::read_to_string(dest_dir.path().join("file_a.txt")).unwrap(),
            "aaa"
        );
        // 원본 유지
        assert!(src_dir.path().join("file_a.txt").exists());
    }

    #[test]
    fn test_copy_directory_recursive() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();

        let sources = vec![src_dir.path().join("sub_dir")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        copy_entries(&source_refs, dest_dir.path()).unwrap();

        assert!(dest_dir.path().join("sub_dir").is_dir());
        assert!(dest_dir.path().join("sub_dir/inner.txt").exists());
    }

    #[test]
    fn test_copy_duplicate_error() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();
        fs::write(dest_dir.path().join("file_a.txt"), "existing").unwrap();

        let sources = vec![src_dir.path().join("file_a.txt")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        let result = copy_entries(&source_refs, dest_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("이미 존재"));
    }

    #[test]
    fn test_copy_invalid_dest() {
        let src_dir = setup_test_dir();
        let bad_dest = src_dir.path().join("nonexistent");

        let sources = vec![src_dir.path().join("file_a.txt")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        let result = copy_entries(&source_refs, &bad_dest);
        assert!(result.is_err());
    }

    // --- move ---

    #[test]
    fn test_move_file() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();

        let sources = vec![src_dir.path().join("file_a.txt")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        move_entries(&source_refs, dest_dir.path()).unwrap();

        assert!(dest_dir.path().join("file_a.txt").exists());
        assert!(!src_dir.path().join("file_a.txt").exists()); // 원본 삭제됨
    }

    #[test]
    fn test_move_directory() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();

        let sources = vec![src_dir.path().join("sub_dir")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        move_entries(&source_refs, dest_dir.path()).unwrap();

        assert!(dest_dir.path().join("sub_dir/inner.txt").exists());
        assert!(!src_dir.path().join("sub_dir").exists());
    }

    #[test]
    fn test_move_duplicate_error() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();
        fs::write(dest_dir.path().join("file_a.txt"), "existing").unwrap();

        let sources = vec![src_dir.path().join("file_a.txt")];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        let result = move_entries(&source_refs, dest_dir.path());
        assert!(result.is_err());
    }

    // --- delete ---

    #[test]
    fn test_delete_file() {
        let dir = setup_test_dir();
        let targets = vec![dir.path().join("file_a.txt")];
        let target_refs: Vec<&Path> = targets.iter().map(|p| p.as_path()).collect();
        delete_entries(&target_refs).unwrap();
        assert!(!dir.path().join("file_a.txt").exists());
    }

    #[test]
    fn test_delete_directory() {
        let dir = setup_test_dir();
        let targets = vec![dir.path().join("sub_dir")];
        let target_refs: Vec<&Path> = targets.iter().map(|p| p.as_path()).collect();
        delete_entries(&target_refs).unwrap();
        assert!(!dir.path().join("sub_dir").exists());
    }

    #[test]
    fn test_delete_multiple() {
        let dir = setup_test_dir();
        let targets = vec![
            dir.path().join("file_a.txt"),
            dir.path().join("file_b.txt"),
        ];
        let target_refs: Vec<&Path> = targets.iter().map(|p| p.as_path()).collect();
        delete_entries(&target_refs).unwrap();
        assert!(!dir.path().join("file_a.txt").exists());
        assert!(!dir.path().join("file_b.txt").exists());
    }

    // --- rename ---

    #[test]
    fn test_rename_file() {
        let dir = setup_test_dir();
        let source = dir.path().join("file_a.txt");
        rename_entry(&source, "renamed.txt").unwrap();
        assert!(!dir.path().join("file_a.txt").exists());
        assert!(dir.path().join("renamed.txt").exists());
    }

    #[test]
    fn test_rename_empty_name_error() {
        let dir = setup_test_dir();
        let source = dir.path().join("file_a.txt");
        let result = rename_entry(&source, "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("비어있습니다"));
    }

    #[test]
    fn test_rename_duplicate_error() {
        let dir = setup_test_dir();
        let source = dir.path().join("file_a.txt");
        let result = rename_entry(&source, "file_b.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("이미 존재"));
    }

    #[test]
    fn test_rename_path_separator_error() {
        let dir = setup_test_dir();
        let source = dir.path().join("file_a.txt");
        let result = rename_entry(&source, "sub/name.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("경로 구분자"));
    }

    // --- mkdir ---

    #[test]
    fn test_create_directory() {
        let dir = setup_test_dir();
        create_directory(dir.path(), "new_dir").unwrap();
        assert!(dir.path().join("new_dir").is_dir());
    }

    #[test]
    fn test_create_directory_empty_name_error() {
        let dir = setup_test_dir();
        let result = create_directory(dir.path(), "");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_directory_duplicate_error() {
        let dir = setup_test_dir();
        let result = create_directory(dir.path(), "sub_dir");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("이미 존재"));
    }

    #[test]
    fn test_copy_multiple_files() {
        let src_dir = setup_test_dir();
        let dest_dir = tempfile::tempdir().unwrap();

        let sources = vec![
            src_dir.path().join("file_a.txt"),
            src_dir.path().join("file_b.txt"),
        ];
        let source_refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        copy_entries(&source_refs, dest_dir.path()).unwrap();

        assert!(dest_dir.path().join("file_a.txt").exists());
        assert!(dest_dir.path().join("file_b.txt").exists());
    }
}
