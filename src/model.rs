use crate::settings::{EditorMode, SidebarMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorInfo {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionInfo {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineItem {
    pub level: u8,
    pub title: String,
    pub anchor: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentStats {
    pub words: usize,
    pub chars: usize,
    pub lines: usize,
    pub reading_time_minutes: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DocumentState {
    pub path: Option<PathBuf>,
    pub title: String,
    pub content: String,
    pub revision: u64,
    pub dirty: bool,
    pub cursor: CursorInfo,
    pub selection: SelectionInfo,
    pub outline: Vec<OutlineItem>,
    pub stats: DocumentStats,
}

impl DocumentState {
    pub fn scratch() -> Self {
        Self {
            path: None,
            title: "Untitled.md".to_string(),
            content: "# Untitled\n\nStart writing...".to_string(),
            revision: 0,
            dirty: false,
            cursor: CursorInfo::default(),
            selection: SelectionInfo::default(),
            outline: Vec::new(),
            stats: DocumentStats::default(),
        }
    }

    pub fn from_path(path: PathBuf, content: String) -> Self {
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled.md".to_string());

        Self {
            path: Some(path),
            title,
            content,
            revision: 0,
            dirty: false,
            cursor: CursorInfo::default(),
            selection: SelectionInfo::default(),
            outline: Vec::new(),
            stats: DocumentStats::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<FileEntry>,
}

impl FileEntry {
    pub fn flatten_files(&self, output: &mut Vec<PathBuf>) {
        if self.is_dir {
            for child in &self.children {
                child.flatten_files(output);
            }
        } else {
            output.push(self.path.clone());
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceState {
    pub root: Option<PathBuf>,
    pub file_tree: Vec<FileEntry>,
    pub open_documents: Vec<DocumentState>,
    pub active_document: usize,
    pub recent_files: Vec<PathBuf>,
    pub sidebar_mode: SidebarMode,
    pub editor_mode: EditorMode,
    pub global_query: String,
    pub quick_open_query: String,
}

impl WorkspaceState {
    pub fn active_document(&self) -> Option<&DocumentState> {
        self.open_documents.get(self.active_document)
    }

    pub fn active_document_mut(&mut self) -> Option<&mut DocumentState> {
        self.open_documents.get_mut(self.active_document)
    }

    pub fn all_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in &self.file_tree {
            entry.flatten_files(&mut files);
        }
        files
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line: usize,
    pub preview: String,
}
