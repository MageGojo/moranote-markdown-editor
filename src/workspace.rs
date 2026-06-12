use crate::model::{FileEntry, SearchResult};
use anyhow::Context as _;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn scan_workspace(
    root: &Path,
    supported_extensions: &[String],
) -> anyhow::Result<Vec<FileEntry>> {
    let supported = supported_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .collect::<HashSet<_>>();

    scan_dir(root, &supported)
}

fn scan_dir(root: &Path, supported: &HashSet<String>) -> anyhow::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    let read_dir =
        std::fs::read_dir(root).with_context(|| format!("读取目录失败: {}", root.display()))?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if should_skip(&name) {
            continue;
        }

        if path.is_dir() {
            let children = scan_dir(&path, supported).unwrap_or_default();
            if !children.is_empty() {
                entries.push(FileEntry {
                    name,
                    path,
                    is_dir: true,
                    expanded: false,
                    children,
                });
            }
            continue;
        }

        if is_supported_file(&path, supported) {
            entries.push(FileEntry {
                name,
                path,
                is_dir: false,
                expanded: false,
                children: Vec::new(),
            });
        }
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn is_supported_file(path: &Path, supported_extensions: &HashSet<String>) -> bool {
    path.extension()
        .map(|extension| {
            supported_extensions.contains(&extension.to_string_lossy().to_ascii_lowercase())
        })
        .unwrap_or(false)
}

pub fn toggle_entry(entries: &mut [FileEntry], target: &Path) -> bool {
    for entry in entries {
        if entry.path == target {
            entry.expanded = !entry.expanded;
            return true;
        }
        if toggle_entry(&mut entry.children, target) {
            return true;
        }
    }
    false
}

pub fn global_search(files: &[PathBuf], query: &str, limit: usize) -> Vec<SearchResult> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for path in files {
        if results.len() >= limit {
            break;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        for (index, line) in content.lines().enumerate() {
            if line.to_ascii_lowercase().contains(&needle) {
                results.push(SearchResult {
                    path: path.clone(),
                    line: index + 1,
                    preview: line.trim().to_string(),
                });
                if results.len() >= limit {
                    break;
                }
            }
        }
    }

    results
}

fn should_skip(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "node_modules" | ".git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_file_filter_matches_extension_without_dot() {
        let supported = ["md".to_string()].into_iter().collect::<HashSet<_>>();

        assert!(is_supported_file(Path::new("note.md"), &supported));
        assert!(is_supported_file(Path::new("note.MD"), &supported));
        assert!(!is_supported_file(Path::new("note.txt"), &supported));
        assert!(!is_supported_file(Path::new("note.rs"), &supported));
    }
}
