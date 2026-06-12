use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    Sepia,
    Typora,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "浅色",
            ThemeMode::Dark => "深色",
            ThemeMode::Sepia => "护眼",
            ThemeMode::Typora => "Typora",
        }
    }

    /// 主题选择器中展示的顺序（手动选择用）。
    pub fn all() -> [ThemeMode; 4] {
        [
            ThemeMode::Light,
            ThemeMode::Typora,
            ThemeMode::Sepia,
            ThemeMode::Dark,
        ]
    }
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Light
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorMode {
    Source,
    Preview,
    Split,
}

impl EditorMode {
    pub fn label(self) -> &'static str {
        match self {
            EditorMode::Source => "源码",
            EditorMode::Preview => "预览",
            EditorMode::Split => "分屏",
        }
    }
}

impl Default for EditorMode {
    fn default() -> Self {
        EditorMode::Split
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarMode {
    Files,
    FileList,
    Recent,
    Outline,
    Search,
}

impl SidebarMode {
    pub fn label(self) -> &'static str {
        match self {
            SidebarMode::Files => "文件树",
            SidebarMode::FileList => "文件列表",
            SidebarMode::Recent => "历史",
            SidebarMode::Outline => "大纲",
            SidebarMode::Search => "搜索",
        }
    }
}

impl Default for SidebarMode {
    fn default() -> Self {
        SidebarMode::Files
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub editor_mode: EditorMode,
    #[serde(default)]
    pub sidebar_mode: SidebarMode,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default)]
    pub line_numbers: bool,
    #[serde(default = "default_true")]
    pub soft_wrap: bool,
    #[serde(default = "default_tab_size")]
    pub tab_size: u32,
    #[serde(default)]
    pub auto_save: bool,
    #[serde(default)]
    pub focus_mode: bool,
    #[serde(default)]
    pub typewriter_mode: bool,
    #[serde(default)]
    pub pandoc_path: Option<PathBuf>,
    #[serde(default)]
    pub chromium_path: Option<PathBuf>,
    #[serde(default)]
    pub default_export_dir: Option<PathBuf>,
    #[serde(default = "default_supported_extensions")]
    pub supported_extensions: Vec<String>,
    #[serde(default = "default_true")]
    pub claim_markdown_association: bool,
    #[serde(default)]
    pub last_opened_folder: Option<PathBuf>,
    #[serde(default)]
    pub last_active_file: Option<PathBuf>,
    #[serde(default)]
    pub recent_files: Vec<PathBuf>,
    #[serde(default)]
    pub recent_folders: Vec<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Light,
            editor_mode: EditorMode::Split,
            sidebar_mode: SidebarMode::Files,
            font_family: default_font_family(),
            font_size: default_font_size(),
            line_numbers: false,
            soft_wrap: true,
            tab_size: default_tab_size(),
            auto_save: false,
            focus_mode: false,
            typewriter_mode: false,
            pandoc_path: None,
            chromium_path: None,
            default_export_dir: None,
            supported_extensions: default_supported_extensions(),
            claim_markdown_association: true,
            last_opened_folder: None,
            last_active_file: None,
            recent_files: Vec::new(),
            recent_folders: Vec::new(),
        }
    }
}

pub struct Settings;

impl Settings {
    fn config_path() -> PathBuf {
        let mut path = dirs_next().unwrap_or_else(|| PathBuf::from("."));
        path.push(".markdown-editor-config.toml");
        path
    }

    pub fn load() -> AppConfig {
        let path = Self::config_path();
        let mut config = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|content| toml::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            AppConfig::default()
        };

        if config.supported_extensions.is_empty() {
            config.supported_extensions = default_supported_extensions();
        }
        config.supported_extensions = normalize_extensions(&config.supported_extensions);
        if config.font_family.trim().is_empty() {
            config.font_family = default_font_family();
        }
        config.font_size = config.font_size.clamp(12, 28);
        config.tab_size = config.tab_size.clamp(2, 8);
        let supported_extensions = config.supported_extensions.clone();
        config
            .recent_files
            .retain(|path| is_supported_path(path, &supported_extensions));
        if config
            .last_active_file
            .as_ref()
            .is_some_and(|path| !is_supported_path(path, &supported_extensions))
        {
            config.last_active_file = None;
        }
        config
            .recent_folders
            .retain(|path| !path.as_os_str().is_empty());
        config
    }

    pub fn save(config: &AppConfig) -> anyhow::Result<()> {
        let path = Self::config_path();
        std::fs::write(path, toml::to_string_pretty(config)?)?;
        Ok(())
    }
}

fn default_true() -> bool {
    true
}

fn default_font_family() -> String {
    "Alibaba PuHuiTi 3.0".to_string()
}

fn default_font_size() -> u32 {
    16
}

fn default_tab_size() -> u32 {
    4
}

fn default_supported_extensions() -> Vec<String> {
    vec!["md".to_string()]
}

fn normalize_extensions(extensions: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for extension in extensions {
        let extension = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if extension.is_empty() || normalized.contains(&extension) {
            continue;
        }
        normalized.push(extension);
    }
    if normalized.is_empty() {
        normalized.push("md".to_string());
    }
    normalized
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .ok()
}

fn is_supported_path(path: &std::path::Path, supported_extensions: &[String]) -> bool {
    let Some(extension) = path.extension() else {
        return false;
    };
    let extension = extension.to_string_lossy();
    supported_extensions
        .iter()
        .any(|supported| extension.eq_ignore_ascii_case(supported.trim_start_matches('.')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_typora_like_extensions() {
        let config = AppConfig::default();
        assert_eq!(config.supported_extensions, vec!["md".to_string()]);
        assert_eq!(config.editor_mode, EditorMode::Split);
        assert!(!config.line_numbers);
        assert!(!config.auto_save);
        assert!(config.claim_markdown_association);
    }
}
