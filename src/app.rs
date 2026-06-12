use crate::actions::*;
use crate::export::{
    ExportFormat, ExportRequest, default_export_path, export_document, normalize_export_path,
};
use crate::model::{DocumentState, FileEntry, SearchResult, WorkspaceState};
use crate::preview::{
    RenderRequest, RenderedDocument, SharedResourceRoot, preview_shell, render_markdown,
    resource_response, scroll_script, update_script,
};
use crate::settings::{AppConfig, EditorMode, Settings, SidebarMode, ThemeMode};
use crate::workspace::{global_search, scan_workspace, toggle_entry};
use gpui::prelude::*;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState, TabSize};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::webview::WebView;
use gpui_component::tooltip::Tooltip;
use gpui_component::{
    Disableable, Icon, IconName, Sizable as _, Theme, ThemeMode as ComponentThemeMode,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// 集中的语义化调色板。所有界面取色都应通过 `Palette`，
/// 以保证 Light / Sepia / Dark 三套主题完全一致、不再出现硬编码颜色。
#[derive(Debug, Clone, Copy)]
struct Palette {
    /// 窗口最底层背景。
    app_bg: u32,
    /// 主体工作区背景（编辑区外圈）。
    workspace: u32,
    /// 侧栏 / 面板背景。
    panel: u32,
    /// 卡片 / 编辑纸张等实心表面。
    surface: u32,
    /// 次级表面（输入框、徽标、分组头底色）。
    surface_soft: u32,
    /// 浮层卡片背景（设置、导出、弹窗）。
    elevated: u32,
    /// 顶部栏 / 底部栏的半透明玻璃背景。
    chrome: u32,
    /// 常规描边。
    border: u32,
    /// 强调描边（聚焦、激活边框）。
    border_strong: u32,
    /// 正文主色。
    text: u32,
    /// 次要文字。
    muted: u32,
    /// 更弱的提示文字。
    subtle: u32,
    /// 强调色（链接、激活图标、主操作）。
    accent: u32,
    /// 强调色的浅底（用于徽标、激活背景）。
    accent_soft: u32,
    /// 强调色文字在 accent_soft 上的可读色。
    accent_text: u32,
    /// 品牌暖色（标题色、文件夹图标），随主题变化。
    warm: u32,
    /// 暖色浅底。
    warm_soft: u32,
    /// 悬停态背景。
    hover: u32,
    /// 选中 / 激活态背景（比 hover 更明显）。
    active: u32,
    /// 阴影色。
    shadow: u32,
}

impl Palette {
    fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::LIGHT,
            ThemeMode::Sepia => Self::SEPIA,
            ThemeMode::Dark => Self::DARK,
            ThemeMode::Typora => Self::TYPORA,
        }
    }

    const LIGHT: Self = Self {
        app_bg: 0xf4f7f4ff,
        workspace: 0xeef3eeff,
        panel: 0xf7faf7ff,
        surface: 0xfdfefdff,
        surface_soft: 0xeef3efff,
        elevated: 0xfcfdfcff,
        chrome: 0xf9fbf9f7,
        border: 0xdbe4ddff,
        border_strong: 0xc2d2c6ff,
        text: 0x3a3633ff,
        muted: 0x807c73ff,
        subtle: 0xa6a59cff,
        accent: 0x55715bff,
        accent_soft: 0xe6eee7ff,
        accent_text: 0x415a47ff,
        warm: 0x8a6a5aff,
        warm_soft: 0xf0e7e0ff,
        hover: 0xe7eee8ff,
        active: 0xdbe7deff,
        shadow: 0x3a363314,
    };

    const SEPIA: Self = Self {
        app_bg: 0xefe7d9ff,
        workspace: 0xeae1d1ff,
        panel: 0xf3ecdfff,
        surface: 0xfaf4e9ff,
        surface_soft: 0xede4d4ff,
        elevated: 0xf8f2e7ff,
        chrome: 0xf4eddff5,
        border: 0xdcd0bcff,
        border_strong: 0xc8b9a1ff,
        text: 0x423d38ff,
        muted: 0x847b6dff,
        subtle: 0xa99f8cff,
        accent: 0x5b6e54ff,
        accent_soft: 0xe7e0cfff,
        accent_text: 0x47593fff,
        warm: 0x8a6c5dff,
        warm_soft: 0xeee2d2ff,
        hover: 0xe6dccaff,
        active: 0xddd0baff,
        shadow: 0x4b3e3314,
    };

    const DARK: Self = Self {
        app_bg: 0x21241fff,
        workspace: 0x1c1f1aff,
        panel: 0x282c25ff,
        surface: 0x2a2e28ff,
        surface_soft: 0x32372fff,
        elevated: 0x2f342dff,
        chrome: 0x252a23f5,
        border: 0x3c423aff,
        border_strong: 0x4c5447ff,
        text: 0xe7e1d7ff,
        muted: 0xa9a698ff,
        subtle: 0x7e7c70ff,
        accent: 0xa8b89eff,
        accent_soft: 0x333a31ff,
        accent_text: 0xbecbb4ff,
        warm: 0xcdb1a2ff,
        warm_soft: 0x3a332eff,
        hover: 0x343a31ff,
        active: 0x3e463bff,
        shadow: 0x00000040,
    };

    /// Typora / GitHub 风格：纯净白底、冷中性灰、GitHub 蓝强调，无暖色调。
    const TYPORA: Self = Self {
        app_bg: 0xf6f8faff,
        workspace: 0xeaeef2ff,
        panel: 0xf6f8faff,
        surface: 0xffffffff,
        surface_soft: 0xf0f3f6ff,
        elevated: 0xffffffff,
        chrome: 0xfbfcfdf7,
        border: 0xd0d7deff,
        border_strong: 0xafb8c1ff,
        text: 0x1f2328ff,
        muted: 0x636c76ff,
        subtle: 0x8c959fff,
        accent: 0x0969daff,
        accent_soft: 0xddf4ffff,
        accent_text: 0x0550aeff,
        warm: 0x0969daff,
        warm_soft: 0xddf4ffff,
        hover: 0xeef1f4ff,
        active: 0xe2e8efff,
        shadow: 0x1f23280f,
    };
}

pub type SharedOpenFiles = Arc<Mutex<Vec<PathBuf>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSnapshot {
    modified: Option<SystemTime>,
    len: u64,
}

pub fn claim_system_markdown_association() {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::{CString, c_char, c_void};

        type CFStringRef = *const c_void;
        const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
        const K_LS_ROLES_EDITOR: u32 = 0x0000_0002;

        #[link(name = "ApplicationServices", kind = "framework")]
        unsafe extern "C" {
            fn CFStringCreateWithCString(
                alloc: *const c_void,
                c_str: *const c_char,
                encoding: u32,
            ) -> CFStringRef;
            fn CFRelease(value: *const c_void);
            fn LSSetDefaultRoleHandlerForContentType(
                content_type: CFStringRef,
                role: u32,
                handler_bundle_id: CFStringRef,
            ) -> i32;
        }

        fn cf_string(value: &str) -> Option<CFStringRef> {
            let value = CString::new(value).ok()?;
            let string = unsafe {
                CFStringCreateWithCString(
                    std::ptr::null(),
                    value.as_ptr(),
                    K_CF_STRING_ENCODING_UTF8,
                )
            };
            (!string.is_null()).then_some(string)
        }

        let Some(markdown_uti) = cf_string("net.daringfireball.markdown") else {
            return;
        };
        let Some(bundle_id) = cf_string("local.moranote") else {
            unsafe {
                CFRelease(markdown_uti);
            }
            return;
        };

        unsafe {
            let _ =
                LSSetDefaultRoleHandlerForContentType(markdown_uti, K_LS_ROLES_EDITOR, bundle_id);
            CFRelease(markdown_uti);
            CFRelease(bundle_id);
        }
    }
}

pub struct AppShell {
    config: AppConfig,
    workspace: WorkspaceState,
    rendered: RenderedDocument,
    editor: Entity<InputState>,
    quick_open_input: Entity<InputState>,
    global_search_input: Entity<InputState>,
    settings_font_input: Entity<InputState>,
    settings_extensions_input: Entity<InputState>,
    theme_select: Entity<SelectState<SearchableVec<&'static str>>>,
    preview: Option<Entity<WebView>>,
    resource_root: SharedResourceRoot,
    pending_open_files: SharedOpenFiles,
    file_snapshots: HashMap<PathBuf, FileSnapshot>,
    show_sidebar: bool,
    show_settings: bool,
    show_export: bool,
    show_quick_open: bool,
    show_recent_open: bool,
    exporting: bool,
    export_generation: u64,
    status: String,
    suppress_editor_event: bool,
    platform_diagnostics: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

impl AppShell {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        pending_open_files: SharedOpenFiles,
    ) -> Self {
        let mut config = Settings::load();
        if matches!(
            config.sidebar_mode,
            SidebarMode::Outline | SidebarMode::Recent
        ) {
            config.sidebar_mode = SidebarMode::Files;
        }
        let mut workspace = WorkspaceState {
            sidebar_mode: config.sidebar_mode,
            editor_mode: config.editor_mode,
            recent_files: config.recent_files.clone(),
            ..WorkspaceState::default()
        };

        let document = DocumentState::scratch();
        let rendered = render_markdown(RenderRequest {
            markdown: document.content.clone(),
            base_dir: None,
        });
        workspace
            .open_documents
            .push(with_rendered(document, &rendered));

        let editor = Self::new_editor(window, cx, &config, &workspace.open_documents[0].content);
        let quick_open_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜索当前文件夹中的 Markdown 文件...")
        });
        let global_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("在工作区搜索..."));
        let settings_font_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入字体名称...")
                .default_value(config.font_family.clone())
        });
        let settings_extensions_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("md")
                .default_value(config.supported_extensions.join(", "))
        });
        let theme_options: Vec<&'static str> =
            ThemeMode::all().iter().map(|mode| mode.label()).collect();
        let current_theme_label = config.theme_mode.label();
        let theme_select = cx.new(|cx| {
            let mut state =
                SelectState::new(SearchableVec::new(theme_options), None, window, cx);
            state.set_selected_value(&current_theme_label, window, cx);
            state
        });
        let resource_root = Arc::new(Mutex::new(None));
        let preview = Self::new_preview(window, cx, resource_root.clone());

        let _subscriptions = vec![
            cx.subscribe_in(
                &editor,
                window,
                |this: &mut AppShell, state, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let content = state.read(cx).value().to_string();
                        this.on_editor_changed(content, window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &quick_open_input,
                window,
                |this: &mut AppShell, state, event: &InputEvent, _window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.workspace.quick_open_query = state.read(cx).value().to_string();
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &global_search_input,
                window,
                |this: &mut AppShell, state, event: &InputEvent, _window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.workspace.global_query = state.read(cx).value().to_string();
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &settings_font_input,
                window,
                |this: &mut AppShell, state, event: &InputEvent, _window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.apply_font_family(state.read(cx).value().to_string(), cx);
                    }
                },
            ),
            cx.subscribe_in(
                &settings_extensions_input,
                window,
                |this: &mut AppShell, state, event: &InputEvent, _window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.apply_supported_extensions(state.read(cx).value().to_string(), cx);
                    }
                },
            ),
            cx.subscribe_in(
                &theme_select,
                window,
                |this: &mut AppShell, _state, event: &SelectEvent<SearchableVec<&'static str>>, window, cx| {
                    let SelectEvent::Confirm(value) = event;
                    if let Some(label) = value {
                        if let Some(mode) = theme_mode_from_label(label) {
                            this.set_theme(mode, window, cx);
                        }
                    }
                },
            ),
            cx.observe_window_activation(window, |this: &mut AppShell, window, cx| {
                if window.is_window_active() {
                    this.sync_external_workspace(window, cx);
                }
            }),
        ];

        let mut this = Self {
            config,
            workspace,
            rendered,
            editor,
            quick_open_input,
            global_search_input,
            settings_font_input,
            settings_extensions_input,
            theme_select,
            preview,
            resource_root,
            pending_open_files,
            file_snapshots: HashMap::new(),
            show_sidebar: false,
            show_settings: false,
            show_export: false,
            show_quick_open: false,
            show_recent_open: false,
            exporting: false,
            export_generation: 0,
            status: "Ready".to_string(),
            suppress_editor_event: false,
            platform_diagnostics: platform_diagnostics(),
            _subscriptions,
        };
        if !this.has_pending_external_files() {
            this.restore_last_session(window, cx);
        }
        this.open_pending_external_files(window, cx);
        this.apply_component_theme(Some(window), cx);
        this.sync_preview(cx);
        this.update_preview_visibility(cx);
        this.start_external_open_watcher(window, cx);
        this.start_workspace_sync_watcher(window, cx);
        this
    }

    fn new_editor(
        window: &mut Window,
        cx: &mut Context<Self>,
        config: &AppConfig,
        content: &str,
    ) -> Entity<InputState> {
        cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("markdown")
                .line_number(config.line_numbers)
                .soft_wrap(config.soft_wrap)
                .tab_size(TabSize {
                    tab_size: config.tab_size as usize,
                    hard_tabs: false,
                })
                .default_value(content.to_string())
        })
    }

    fn new_preview(
        window: &mut Window,
        cx: &mut Context<Self>,
        resource_root: SharedResourceRoot,
    ) -> Option<Entity<WebView>> {
        let resource_root_for_handler = resource_root.clone();
        let webview = gpui_component::wry::WebViewBuilder::new()
            .with_devtools(true)
            .with_initialization_script(crate::preview::renderer::SELECTION_INIT_SCRIPT)
            .with_html(preview_shell())
            .with_custom_protocol("mdres".into(), move |_id, request| {
                resource_response(&resource_root_for_handler, request.uri().path())
            })
            .with_navigation_handler(|url| {
                if url.starts_with("about:")
                    || url.starts_with("mdres://")
                    || url.starts_with("file://")
                {
                    true
                } else {
                    let _ = open::that(url);
                    false
                }
            })
            .build_as_child(window)
            .ok()?;

        Some(cx.new(|cx| WebView::new(webview, window, cx)))
    }

    fn subscribe_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let subscription = cx.subscribe_in(
            &self.editor,
            window,
            |this: &mut AppShell, state, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let content = state.read(cx).value().to_string();
                    this.on_editor_changed(content, window, cx);
                }
            },
        );
        self._subscriptions.push(subscription);
    }

    fn rebuild_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self
            .workspace
            .active_document()
            .map(|document| document.content.clone())
            .unwrap_or_default();
        self.editor = Self::new_editor(window, cx, &self.config, &content);
        self.subscribe_editor(window, cx);
    }

    fn start_external_open_watcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                loop {
                    Timer::after(Duration::from_millis(300)).await;
                    let should_continue = cx
                        .update(|window, cx| {
                            shell
                                .update(cx, |this, cx| {
                                    this.open_pending_external_files(window, cx);
                                })
                                .is_ok()
                        })
                        .unwrap_or(false);
                    if !should_continue {
                        break;
                    }
                }
            })
            .detach();
    }

    fn start_workspace_sync_watcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx| {
                loop {
                    Timer::after(Duration::from_secs(1)).await;
                    let should_continue = cx
                        .update(|window, cx| {
                            shell
                                .update(cx, |this, cx| {
                                    if window.is_window_active() {
                                        this.sync_external_workspace(window, cx);
                                    }
                                })
                                .is_ok()
                        })
                        .unwrap_or(false);
                    if !should_continue {
                        break;
                    }
                }
            })
            .detach();
    }

    fn sync_external_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tree_status = self.sync_workspace_tree(cx);
        let document_status = self.sync_open_documents_from_disk(window, cx);

        if let Some(status) = document_status.or(tree_status) {
            self.status = status;
            cx.notify();
        }
    }

    fn sync_workspace_tree(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let root = self.workspace.root.clone()?;
        let old_files = file_set_from_entries(&self.workspace.file_tree);
        let expanded_dirs = expanded_dir_set(&self.workspace.file_tree);

        let mut entries = match scan_workspace(&root, &self.config.supported_extensions) {
            Ok(entries) => entries,
            Err(error) => {
                return Some(format!("同步文件夹失败: {error}"));
            }
        };
        apply_expanded_dirs(&mut entries, &expanded_dirs);

        let new_files = file_set_from_entries(&entries);
        if old_files == new_files {
            return None;
        }

        let added = new_files.difference(&old_files).count();
        let removed = old_files.difference(&new_files).count();
        self.workspace.file_tree = entries;
        for path in &new_files {
            if let Some(snapshot) = file_snapshot(path) {
                self.file_snapshots.entry(path.clone()).or_insert(snapshot);
            }
        }
        let open_paths = self
            .workspace
            .open_documents
            .iter()
            .filter_map(|document| document.path.clone())
            .collect::<HashSet<_>>();
        self.file_snapshots
            .retain(|path, _| new_files.contains(path) || open_paths.contains(path));
        cx.notify();

        Some(format!(
            "已同步工作区: 新增 {added} 个，移除 {removed} 个 Markdown 文件"
        ))
    }

    fn sync_open_documents_from_disk(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let mut active_reloaded = false;
        let mut reloaded = 0usize;
        let active_document = self.workspace.active_document;

        for (index, document) in self.workspace.open_documents.iter_mut().enumerate() {
            let Some(path) = document.path.clone() else {
                continue;
            };
            let Some(snapshot) = file_snapshot(&path) else {
                continue;
            };
            let previous = self.file_snapshots.get(&path).copied();
            if previous == Some(snapshot) {
                continue;
            }

            self.file_snapshots.insert(path.clone(), snapshot);
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if document.content == content {
                continue;
            }

            document.content = content;
            document.revision += 1;
            document.dirty = false;
            reloaded += 1;
            if index == active_document {
                active_reloaded = true;
            }
        }

        if active_reloaded {
            self.rebuild_editor(window, cx);
            self.render_active_document(cx);
        }

        (reloaded > 0).then(|| {
            if reloaded == 1 {
                "已使用外部修改同步当前 Markdown 内容".to_string()
            } else {
                format!("已使用外部修改同步 {reloaded} 个 Markdown 文档")
            }
        })
    }

    fn open_pending_external_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self
            .pending_open_files
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default();

        if paths.is_empty() {
            return;
        }

        for path in paths {
            self.open_file(path, window, cx);
        }
        self.update_preview_visibility(cx);
    }

    fn has_pending_external_files(&self) -> bool {
        self.pending_open_files
            .lock()
            .map(|pending| !pending.is_empty())
            .unwrap_or(false)
    }

    fn restore_last_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = self
            .config
            .last_opened_folder
            .clone()
            .filter(|folder| folder.is_dir())
        {
            self.open_folder(folder, false, cx);
        }

        let last_file = self
            .config
            .last_active_file
            .clone()
            .or_else(|| self.config.recent_files.first().cloned());
        if let Some(file) = last_file.filter(|file| file.exists() && self.is_supported_path(file)) {
            self.open_file(file, window, cx);
        }
    }

    fn save_config(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = Settings::save(&self.config) {
            self.status = format!("保存设置失败: {error}");
        }
        cx.notify();
    }

    fn apply_font_family(&mut self, value: String, cx: &mut Context<Self>) {
        let value = value.trim();
        if value.is_empty() || self.config.font_family == value {
            return;
        }
        self.config.font_family = value.to_string();
        self.sync_preview(cx);
        self.save_config(cx);
    }

    fn apply_supported_extensions(&mut self, value: String, cx: &mut Context<Self>) {
        let extensions = parse_extensions(&value);
        if extensions.is_empty() || extensions == self.config.supported_extensions {
            return;
        }

        self.config.supported_extensions = extensions;
        let supported_extensions = self.config.supported_extensions.clone();
        self.config
            .recent_files
            .retain(|path| path_has_supported_extension(path, &supported_extensions));
        self.workspace
            .recent_files
            .retain(|path| path_has_supported_extension(path, &supported_extensions));
        self.rescan_current_folder(cx);
        self.status = format!("已更新可打开扩展名: {}", self.supported_extensions_label());
        self.save_config(cx);
    }

    fn rescan_current_folder(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.workspace.root.clone() else {
            return;
        };

        match scan_workspace(&root, &self.config.supported_extensions) {
            Ok(entries) => {
                self.workspace.file_tree = entries;
            }
            Err(error) => {
                self.status = format!("重新扫描文件夹失败: {error}");
            }
        }
        cx.notify();
    }

    fn is_supported_path(&self, path: &Path) -> bool {
        let Some(extension) = path.extension() else {
            return false;
        };
        let extension = extension.to_string_lossy();
        self.config
            .supported_extensions
            .iter()
            .any(|item| extension.eq_ignore_ascii_case(item.trim_start_matches('.')))
    }

    fn supported_extensions_label(&self) -> String {
        self.config
            .supported_extensions
            .iter()
            .map(|extension| format!(".{}", extension.trim_start_matches('.')))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn track_workspace_file_snapshots(&mut self) {
        for path in self.workspace.all_files() {
            self.track_file_snapshot(&path);
        }
    }

    fn track_file_snapshot(&mut self, path: &Path) {
        if let Some(snapshot) = file_snapshot(path) {
            self.file_snapshots.insert(path.to_path_buf(), snapshot);
        }
    }

    fn on_editor_changed(&mut self, content: String, _window: &mut Window, cx: &mut Context<Self>) {
        if self.suppress_editor_event {
            return;
        }

        if let Some(document) = self.workspace.active_document_mut() {
            if document.content != content {
                document.content = content;
                document.revision += 1;
                document.dirty = true;
            }
        }
        self.render_active_document(cx);

        let can_auto_save = self.config.auto_save
            && self
                .workspace
                .active_document()
                .and_then(|document| document.path.as_ref())
                .is_some();
        if can_auto_save {
            let _ = self.save_active_document(false, cx);
        }
        cx.notify();
    }

    fn render_active_document(&mut self, cx: &mut Context<Self>) {
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let base_dir = document
            .path
            .as_ref()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf);
        self.rendered = render_markdown(RenderRequest {
            markdown: document.content.clone(),
            base_dir: base_dir.clone(),
        });
        if let Ok(mut root) = self.resource_root.lock() {
            *root = base_dir;
        }
        if let Some(document) = self.workspace.active_document_mut() {
            document.outline = self.rendered.outline.clone();
            document.stats = self.rendered.stats.clone();
        }
        self.sync_preview(cx);
    }

    fn sync_preview(&self, cx: &mut Context<Self>) {
        if let Some(preview) = &self.preview {
            let theme = serde_json::to_string(preview_theme_name(self.config.theme_mode))
                .unwrap_or_else(|_| "\"light\"".to_string());
            let font_stack = serde_json::to_string(&preview_font_stack(&self.config.font_family))
                .unwrap_or_else(|_| "\"Alibaba PuHuiTi 3.0, sans-serif\"".to_string());
            let font_size = format!("{}px", self.config.font_size);
            let script = format!(
                "document.documentElement.dataset.theme = {theme};\
                 document.documentElement.style.setProperty('--font-main', {font_stack});\
                 document.documentElement.style.setProperty('--font-size-base', '{font_size}');{}",
                update_script(&self.rendered)
            );
            let _ = preview.update(cx, |preview, _cx| {
                let _ = preview.raw().evaluate_script(&script);
            });
        }
    }

    fn floating_panel_visible(&self) -> bool {
        self.show_settings || self.show_export || self.show_quick_open || self.show_recent_open
    }

    fn update_preview_visibility(&self, cx: &mut Context<Self>) {
        if let Some(preview) = &self.preview {
            let visible =
                self.workspace.editor_mode != EditorMode::Source && !self.floating_panel_visible();
            let _ = preview.update(cx, |preview, _cx| {
                if visible {
                    preview.show();
                } else {
                    preview.hide();
                }
            });
        }
    }

    fn hide_preview_for_native_dialog(&self, cx: &mut Context<Self>) {
        if let Some(preview) = &self.preview {
            let _ = preview.update(cx, |preview, _cx| {
                preview.hide();
            });
        }
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_panels(cx);
        self.hide_preview_for_native_dialog(cx);
        self.status = "请选择要打开的 Markdown 文件...".to_string();
        cx.notify();

        let shell = cx.entity().clone();
        let extensions = self.config.supported_extensions.clone();

        window
            .spawn(cx, async move |cx| {
                let filter_extensions = extensions.iter().map(String::as_str).collect::<Vec<_>>();
                let file = rfd::AsyncFileDialog::new()
                    .set_title("打开 Markdown 文件")
                    .add_filter("Markdown", &filter_extensions)
                    .pick_file()
                    .await
                    .map(|file| file.path().to_path_buf());

                cx.update(|window, cx| {
                    let _ = shell.update(cx, |this, cx| {
                        this.update_preview_visibility(cx);
                        match file {
                            Some(file) => {
                                this.open_file(file, window, cx);
                            }
                            None => {
                                this.status = "已取消打开文件".to_string();
                                cx.notify();
                            }
                        }
                        this.update_preview_visibility(cx);
                    });
                })
                .ok();
            })
            .detach();
    }

    fn open_folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_panels(cx);
        self.hide_preview_for_native_dialog(cx);
        self.status = "请选择 Markdown 文件夹...".to_string();
        cx.notify();

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("打开文件夹".into()),
        });
        let shell = cx.entity().clone();

        window
            .spawn(cx, async move |cx| {
                let result = receiver.await;
                cx.update(|_window, cx| {
                    let _ = shell.update(cx, |this, cx| {
                        this.update_preview_visibility(cx);
                        match result {
                            Ok(Ok(Some(paths))) => {
                                if let Some(folder) = paths.into_iter().next() {
                                    this.open_folder(folder, true, cx);
                                }
                            }
                            Ok(Ok(None)) => {
                                this.status = "已取消打开文件夹".to_string();
                                cx.notify();
                            }
                            Ok(Err(error)) => {
                                this.status = format!("打开文件夹选择器失败: {error}");
                                cx.notify();
                            }
                            Err(_) => {
                                this.status = "打开文件夹选择器已关闭".to_string();
                                cx.notify();
                            }
                        }
                        this.update_preview_visibility(cx);
                    });
                })
                .ok();
            })
            .detach();
    }

    fn open_folder(&mut self, folder: PathBuf, reveal_sidebar: bool, cx: &mut Context<Self>) {
        match scan_workspace(&folder, &self.config.supported_extensions) {
            Ok(entries) => {
                self.workspace.root = Some(folder.clone());
                self.workspace.file_tree = entries;
                self.file_snapshots.clear();
                self.track_workspace_file_snapshots();
                if reveal_sidebar {
                    self.show_sidebar = true;
                }
                self.remember_recent_folder(folder.clone());
                self.config.last_opened_folder = Some(folder.clone());
                let _ = Settings::save(&self.config);
                self.status = format!("已打开文件夹: {}", folder.display());
            }
            Err(error) => {
                self.status = format!("打开文件夹失败: {error}");
            }
        }
        cx.notify();
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_supported_path(&path) {
            self.status = format!(
                "只能打开 Markdown 文件 ({}): {}",
                self.supported_extensions_label(),
                path.display()
            );
            cx.notify();
            return;
        }

        if let Some(index) = self
            .workspace
            .open_documents
            .iter()
            .position(|document| document.path.as_ref() == Some(&path))
        {
            self.workspace.active_document = index;
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(document) = self.workspace.open_documents.get_mut(index) {
                    if document.content != content {
                        document.content = content;
                        document.revision += 1;
                        document.dirty = false;
                    }
                }
                self.track_file_snapshot(&path);
            }
            self.rebuild_editor(window, cx);
            self.render_active_document(cx);
            self.remember_recent_file(path.clone());
            self.config.last_active_file = Some(path.clone());
            self.config.recent_files = self.workspace.recent_files.clone();
            let _ = Settings::save(&self.config);
            self.status = format!("已切换到: {}", path.display());
            cx.notify();
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                if self.workspace.root.is_none() {
                    if let Some(parent) = path.parent() {
                        self.open_folder(parent.to_path_buf(), false, cx);
                    }
                }

                let document = DocumentState::from_path(path.clone(), content.clone());
                self.workspace.open_documents.push(document);
                self.workspace.active_document = self.workspace.open_documents.len() - 1;
                self.track_file_snapshot(&path);
                self.remember_recent_file(path.clone());
                self.config.last_active_file = Some(path.clone());
                self.config.recent_files = self.workspace.recent_files.clone();
                let _ = Settings::save(&self.config);
                self.rebuild_editor(window, cx);
                self.render_active_document(cx);
                self.status = format!("已打开: {}", path.display());
            }
            Err(error) => {
                self.status = format!("读取失败: {error}");
            }
        }
        cx.notify();
    }

    fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let document = DocumentState::scratch();
        self.workspace.open_documents.push(document);
        self.workspace.active_document = self.workspace.open_documents.len() - 1;
        self.rebuild_editor(window, cx);
        self.render_active_document(cx);
        self.status = "已新建文档".to_string();
        cx.notify();
    }

    fn save_active_document(
        &mut self,
        force_save_as: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let Some(document) = self.workspace.active_document() else {
            return Ok(());
        };

        if force_save_as || document.path.is_none() {
            let suggested_name = document.title.clone();
            self.hide_preview_for_native_dialog(cx);
            let picked_path = rfd::FileDialog::new()
                .set_file_name(&suggested_name)
                .save_file();
            self.update_preview_visibility(cx);

            let Some(path) = picked_path else {
                return Ok(());
            };

            let Some(document) = self.workspace.active_document_mut() else {
                return Ok(());
            };
            document.path = Some(path.clone());
            document.title = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled.md".to_string());
        }

        let Some(document) = self.workspace.active_document_mut() else {
            return Ok(());
        };
        if let Some(path) = &document.path {
            let path = path.clone();
            std::fs::write(&path, document.content.as_bytes())?;
            document.dirty = false;
            self.track_file_snapshot(&path);
            self.remember_recent_file(path.clone());
            if let Some(parent) = path.parent() {
                let folder = parent.to_path_buf();
                self.remember_recent_folder(folder.clone());
                self.config.last_opened_folder = Some(folder);
            }
            self.config.last_active_file = Some(path.clone());
            self.config.recent_files = self.workspace.recent_files.clone();
            let _ = Settings::save(&self.config);
            self.status = format!("已保存: {}", path.display());
        }
        cx.notify();
        Ok(())
    }

    fn export_current(
        &mut self,
        format: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exporting {
            self.status = "正在导出，请稍候...".to_string();
            cx.notify();
            return;
        }

        self.render_active_document(cx);

        let Some(document) = self.workspace.active_document() else {
            self.status = "没有可导出的文档".to_string();
            cx.notify();
            return;
        };
        let default_path = default_export_path(
            document.path.as_deref(),
            self.config.default_export_dir.as_deref(),
            format,
        );
        let base_dir = document
            .path
            .as_ref()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf);
        let export_dir = default_path
            .parent()
            .filter(|path| path.exists())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let export_name = default_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("untitled.{}", format.extension()));

        self.show_export = false;
        self.exporting = true;
        self.export_generation = self.export_generation.wrapping_add(1);
        let export_generation = self.export_generation;
        self.show_recent_open = false;
        self.status = format!("请选择 {} 的导出位置...", format.label());
        self.hide_preview_for_native_dialog(cx);
        cx.notify();

        let receiver = cx.prompt_for_new_path(&export_dir, Some(&export_name));
        let shell = cx.entity().clone();
        let rendered = self.rendered.clone();
        let pandoc_path = self.config.pandoc_path.clone();
        let chromium_path = self.config.chromium_path.clone();

        window
            .spawn(cx, async move |cx| {
                let selected_path = receiver.await;

                let output_path = match selected_path {
                    Ok(Ok(Some(path))) => normalize_export_path(path, format),
                    Ok(Ok(None)) => {
                        cx.update(|_window, cx| {
                            let _ = shell.update(cx, |this, cx| {
                                if this.export_generation != export_generation {
                                    return;
                                }
                                this.exporting = false;
                                this.status = "已取消导出".to_string();
                                this.update_preview_visibility(cx);
                                cx.notify();
                            });
                        })
                        .ok();
                        return;
                    }
                    Ok(Err(error)) => {
                        cx.update(|_window, cx| {
                            let _ = shell.update(cx, |this, cx| {
                                if this.export_generation != export_generation {
                                    return;
                                }
                                this.exporting = false;
                                this.status = format!("导出位置选择失败: {error}");
                                this.show_export = true;
                                this.update_preview_visibility(cx);
                                cx.notify();
                            });
                        })
                        .ok();
                        return;
                    }
                    Err(_) => {
                        cx.update(|_window, cx| {
                            let _ = shell.update(cx, |this, cx| {
                                if this.export_generation != export_generation {
                                    return;
                                }
                                this.exporting = false;
                                this.status = "导出位置选择器已关闭".to_string();
                                this.update_preview_visibility(cx);
                                cx.notify();
                            });
                        })
                        .ok();
                        return;
                    }
                };

                cx.update(|_window, cx| {
                    let _ = shell.update(cx, |this, cx| {
                        if this.export_generation != export_generation {
                            return;
                        }
                        this.status = format!("正在导出 {}...", format.label());
                        this.update_preview_visibility(cx);
                        cx.notify();
                    });
                })
                .ok();

                let request = ExportRequest {
                    format,
                    rendered,
                    base_dir,
                    output_path,
                    pandoc_path,
                    chromium_path,
                };

                let result = cx
                    .background_spawn(async move {
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            export_document(request)
                        }))
                        .unwrap_or_else(|_| {
                            Err(anyhow::anyhow!("导出过程发生内部错误，已阻止应用退出"))
                        })
                    })
                    .await;

                cx.update(|_window, cx| {
                    let _ = shell.update(cx, |this, cx| {
                        if this.export_generation != export_generation {
                            return;
                        }
                        this.exporting = false;
                        match result {
                            Ok(result) => {
                                this.status = if let Some(command) = result.command {
                                    format!("已导出: {} ({command})", result.path.display())
                                } else {
                                    format!("已导出: {}", result.path.display())
                                };
                                this.show_export = false;
                            }
                            Err(error) => {
                                this.status = format!("导出失败: {error}");
                                this.show_export = true;
                            }
                        }
                        this.update_preview_visibility(cx);
                        cx.notify();
                    });
                })
                .ok();
            })
            .detach();
    }

    fn cancel_export_wait(&mut self, cx: &mut Context<Self>) {
        if !self.exporting {
            return;
        }

        self.export_generation = self.export_generation.wrapping_add(1);
        self.exporting = false;
        self.show_export = false;
        self.show_recent_open = false;
        self.status = "已停止等待导出结果；后台命令会在超时后自动清理。".to_string();
        self.update_preview_visibility(cx);
        cx.notify();
    }

    fn remember_recent_file(&mut self, path: PathBuf) {
        self.workspace.recent_files.retain(|item| item != &path);
        self.workspace.recent_files.insert(0, path);
        self.workspace.recent_files.truncate(12);
    }

    fn remember_recent_folder(&mut self, path: PathBuf) {
        self.config.recent_folders.retain(|item| item != &path);
        self.config.recent_folders.insert(0, path);
        self.config.recent_folders.truncate(12);
    }

    fn set_mode(&mut self, mode: EditorMode, cx: &mut Context<Self>) {
        self.workspace.editor_mode = mode;
        self.config.editor_mode = mode;
        let _ = Settings::save(&self.config);
        self.update_preview_visibility(cx);
        cx.notify();
    }

    fn set_sidebar_mode(&mut self, mode: SidebarMode, cx: &mut Context<Self>) {
        let mode = match mode {
            SidebarMode::Recent | SidebarMode::Outline => SidebarMode::Files,
            mode => mode,
        };
        self.workspace.sidebar_mode = mode;
        self.config.sidebar_mode = mode;
        let _ = Settings::save(&self.config);
        cx.notify();
    }

    /// 直接切换到指定主题（设置面板里手动选择主题时调用）。
    fn set_theme(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.theme_mode == mode {
            return;
        }
        self.config.theme_mode = mode;
        let _ = Settings::save(&self.config);
        self.apply_component_theme(Some(window), cx);
        self.sync_preview(cx);
        cx.notify();
    }

    /// 把当前主题同步到 gpui-component 的全局主题，使 `Input` 等组件
    /// 的底色 / 文字色一并跟随切换（否则组件会一直停留在默认浅色）。
    fn apply_component_theme(&self, window: Option<&mut Window>, cx: &mut App) {
        let mode = match self.config.theme_mode {
            ThemeMode::Dark => ComponentThemeMode::Dark,
            ThemeMode::Light | ThemeMode::Sepia | ThemeMode::Typora => ComponentThemeMode::Light,
        };
        Theme::change(mode, window, cx);
    }

    fn toggle_line_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config.line_numbers = !self.config.line_numbers;
        let _ = Settings::save(&self.config);
        self.editor.update(cx, |editor, cx| {
            editor.set_line_number(self.config.line_numbers, window, cx);
        });
        cx.notify();
    }

    fn toggle_soft_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config.soft_wrap = !self.config.soft_wrap;
        let _ = Settings::save(&self.config);
        self.editor.update(cx, |editor, cx| {
            editor.set_soft_wrap(self.config.soft_wrap, window, cx);
        });
        cx.notify();
    }

    fn adjust_font_size(&mut self, delta: i32, cx: &mut Context<Self>) {
        let next = (self.config.font_size as i32 + delta).clamp(12, 28) as u32;
        if next == self.config.font_size {
            return;
        }
        self.config.font_size = next;
        self.sync_preview(cx);
        self.save_config(cx);
    }

    fn adjust_tab_size(&mut self, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        let next = (self.config.tab_size as i32 + delta).clamp(2, 8) as u32;
        if next == self.config.tab_size {
            return;
        }
        self.config.tab_size = next;
        self.rebuild_editor(window, cx);
        self.save_config(cx);
    }

    fn toggle_auto_save(&mut self, cx: &mut Context<Self>) {
        self.config.auto_save = !self.config.auto_save;
        self.save_config(cx);
    }

    fn toggle_focus_mode_setting(&mut self, cx: &mut Context<Self>) {
        self.config.focus_mode = !self.config.focus_mode;
        self.save_config(cx);
    }

    fn toggle_typewriter_mode_setting(&mut self, cx: &mut Context<Self>) {
        self.config.typewriter_mode = !self.config.typewriter_mode;
        self.save_config(cx);
    }

    fn cycle_editor_mode(&mut self, cx: &mut Context<Self>) {
        let mode = match self.config.editor_mode {
            EditorMode::Source => EditorMode::Preview,
            EditorMode::Preview => EditorMode::Split,
            EditorMode::Split => EditorMode::Source,
        };
        self.set_mode(mode, cx);
    }

    fn cycle_sidebar_mode(&mut self, cx: &mut Context<Self>) {
        let mode = match self.config.sidebar_mode {
            SidebarMode::Files => SidebarMode::FileList,
            SidebarMode::FileList | SidebarMode::Recent | SidebarMode::Outline => {
                SidebarMode::Search
            }
            SidebarMode::Search => SidebarMode::Files,
        };
        self.set_sidebar_mode(mode, cx);
    }

    fn toggle_markdown_association(&mut self, cx: &mut Context<Self>) {
        self.config.claim_markdown_association = !self.config.claim_markdown_association;
        if self.config.claim_markdown_association {
            claim_system_markdown_association();
            self.status = "已启用启动时自动注册 .md 默认打开".to_string();
        } else {
            self.status = "已关闭自动注册；当前系统默认应用不会被立即恢复。".to_string();
        }
        self.save_config(cx);
    }

    fn choose_pandoc(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.config.pandoc_path = Some(path);
            let _ = Settings::save(&self.config);
            self.status = "Pandoc 路径已更新".to_string();
            cx.notify();
        }
    }

    fn clear_pandoc(&mut self, cx: &mut Context<Self>) {
        self.config.pandoc_path = None;
        self.status = "Pandoc 路径已清除".to_string();
        self.save_config(cx);
    }

    fn choose_chromium(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.config.chromium_path = Some(path);
            let _ = Settings::save(&self.config);
            self.status = "Chromium 路径已更新".to_string();
            cx.notify();
        }
    }

    fn clear_chromium(&mut self, cx: &mut Context<Self>) {
        self.config.chromium_path = None;
        self.status = "Chromium 路径已清除".to_string();
        self.save_config(cx);
    }

    fn choose_export_dir(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.config.default_export_dir = Some(path);
            let _ = Settings::save(&self.config);
            self.status = "默认导出目录已更新".to_string();
            cx.notify();
        }
    }

    fn clear_export_dir(&mut self, cx: &mut Context<Self>) {
        self.config.default_export_dir = None;
        self.status = "默认导出目录已清除".to_string();
        self.save_config(cx);
    }

    fn insert_markdown(
        &mut self,
        snippet: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |editor, cx| {
            editor.insert(snippet, window, cx);
        });
    }

    fn scroll_preview_to(&mut self, anchor: String, cx: &mut Context<Self>) {
        if let Some(preview) = &self.preview {
            let script = scroll_script(&anchor);
            let _ = preview.update(cx, |preview, _cx| {
                let _ = preview.raw().evaluate_script(&script);
            });
        }
    }

    fn matching_files(&self) -> Vec<PathBuf> {
        let query = self.workspace.quick_open_query.trim().to_ascii_lowercase();
        self.workspace
            .all_files()
            .into_iter()
            .filter(|path| {
                query.is_empty()
                    || path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_ascii_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .take(24)
            .collect()
    }

    fn current_search_results(&self) -> Vec<SearchResult> {
        global_search(
            &self.workspace.all_files(),
            &self.workspace.global_query,
            80,
        )
    }

    fn close_panels(&mut self, cx: &mut Context<Self>) {
        self.show_settings = false;
        self.show_export = false;
        self.show_quick_open = false;
        self.show_recent_open = false;
        self.update_preview_visibility(cx);
        cx.notify();
    }

    fn handle_open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_dialog(window, cx);
    }

    fn handle_open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folder_dialog(window, cx);
    }

    fn handle_save(&mut self, _: &SaveFile, _window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.save_active_document(false, cx);
    }

    fn handle_save_as(&mut self, _: &SaveFileAs, _window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.save_active_document(true, cx);
    }

    fn handle_new_file(&mut self, _: &NewFile, window: &mut Window, cx: &mut Context<Self>) {
        self.new_file(window, cx);
    }

    fn handle_toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_sidebar = !self.show_sidebar;
        cx.notify();
    }

    fn handle_toggle_settings(
        &mut self,
        _: &ToggleSettings,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_settings = !self.show_settings;
        self.show_export = false;
        self.show_quick_open = false;
        self.show_recent_open = false;
        self.update_preview_visibility(cx);
        cx.notify();
    }

    fn handle_toggle_export(
        &mut self,
        _: &ToggleExport,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_export = !self.show_export;
        self.show_settings = false;
        self.show_quick_open = false;
        self.show_recent_open = false;
        self.update_preview_visibility(cx);
        cx.notify();
    }

    fn handle_toggle_focus(
        &mut self,
        _: &ToggleFocusMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.config.focus_mode = !self.config.focus_mode;
        let _ = Settings::save(&self.config);
        cx.notify();
    }

    fn handle_toggle_typewriter(
        &mut self,
        _: &ToggleTypewriterMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.config.typewriter_mode = !self.config.typewriter_mode;
        let _ = Settings::save(&self.config);
        cx.notify();
    }

    fn handle_quick_open(
        &mut self,
        _: &ToggleQuickOpen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.all_files().is_empty() {
            self.open_file_dialog(window, cx);
            return;
        }
        self.show_quick_open = !self.show_quick_open;
        self.show_settings = false;
        self.show_export = false;
        self.show_recent_open = false;
        self.update_preview_visibility(cx);
        cx.notify();
    }

    fn handle_global_search(
        &mut self,
        _: &ToggleGlobalSearch,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_settings = false;
        self.show_export = false;
        self.show_quick_open = false;
        self.show_recent_open = false;
        self.update_preview_visibility(cx);
        self.show_sidebar = true;
        self.set_sidebar_mode(SidebarMode::Search, cx);
    }

    fn handle_mode_source(&mut self, _: &ModeSource, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(EditorMode::Source, cx);
    }

    fn handle_mode_preview(
        &mut self,
        _: &ModePreview,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_mode(EditorMode::Preview, cx);
    }

    fn handle_mode_split(&mut self, _: &ModeSplit, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(EditorMode::Split, cx);
    }

    fn handle_close_panel(&mut self, _: &ClosePanel, _window: &mut Window, cx: &mut Context<Self>) {
        self.close_panels(cx);
    }
}

impl EventEmitter<()> for AppShell {}

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .key_context("App")
            .bg(rgba(palette.app_bg))
            .text_color(rgba(palette.text))
            .font_family(self.config.font_family.clone())
            .text_size(px(14.0))
            .on_action(cx.listener(Self::handle_open_file))
            .on_action(cx.listener(Self::handle_open_folder))
            .on_action(cx.listener(Self::handle_save))
            .on_action(cx.listener(Self::handle_save_as))
            .on_action(cx.listener(Self::handle_new_file))
            .on_action(cx.listener(Self::handle_toggle_sidebar))
            .on_action(cx.listener(Self::handle_toggle_settings))
            .on_action(cx.listener(Self::handle_toggle_export))
            .on_action(cx.listener(Self::handle_toggle_focus))
            .on_action(cx.listener(Self::handle_toggle_typewriter))
            .on_action(cx.listener(Self::handle_quick_open))
            .on_action(cx.listener(Self::handle_global_search))
            .on_action(cx.listener(Self::handle_mode_source))
            .on_action(cx.listener(Self::handle_mode_preview))
            .on_action(cx.listener(Self::handle_mode_split))
            .on_action(cx.listener(Self::handle_close_panel))
            .child(self.render_toolbar(cx))
            .child(self.render_body(cx))
            .child(self.render_statusbar(cx))
            .when(self.floating_panel_visible(), |this| {
                this.child(self.render_overlay_layer(cx))
            })
    }
}

impl AppShell {
    fn palette(&self) -> Palette {
        Palette::for_mode(self.config.theme_mode)
    }

    fn render_overlay_layer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let scrim = if self.config.theme_mode == ThemeMode::Dark {
            0x00000066
        } else {
            0x2b282440
        };
        div()
            .id("overlay-scrim")
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(rgba(scrim))
            // 作为完整的事件屏障：拦截背后所有鼠标交互（点击 + 滚轮），
            // 否则在浮层上滚动会穿透到下层源码编辑区。
            .occlude()
            .when(self.show_settings || self.show_recent_open, |this| {
                this.flex().justify_center().items_start().pt(px(70.0)).pb(px(40.0))
            })
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.close_panels(cx);
            }))
            .when(self.show_settings, |this| {
                this.child(self.render_settings(cx))
            })
            .when(self.show_export, |this| {
                this.child(self.render_export_panel(cx))
            })
            .when(self.show_quick_open, |this| {
                this.child(self.render_quick_open(cx))
            })
            .when(self.show_recent_open, |this| {
                this.child(self.render_recent_open(cx))
            })
            .text_color(rgba(palette.text))
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().clone();
        let palette = self.palette();
        div()
            .h(px(54.0))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .pl_3()
            .pr_3()
            .border_b_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.chrome))
            .child(self.render_brand(palette))
            .child(toolbar_divider(palette))
            .child(icon_button(
                "open-file",
                "打开文件",
                IconName::File,
                palette,
                &shell,
                |this, window, cx| this.open_file_dialog(window, cx),
            ))
            .child(icon_button(
                "open-folder",
                "打开文件夹",
                IconName::FolderOpen,
                palette,
                &shell,
                |this, window, cx| this.open_folder_dialog(window, cx),
            ))
            .child(icon_button(
                "recent-open",
                "最近打开",
                IconName::Calendar,
                palette,
                &shell,
                |this, _window, cx| {
                    this.show_recent_open = !this.show_recent_open;
                    this.show_settings = false;
                    this.show_export = false;
                    this.show_quick_open = false;
                    this.update_preview_visibility(cx);
                    cx.notify();
                },
            ))
            .child(icon_button(
                "save",
                "保存",
                IconName::Check,
                palette,
                &shell,
                |this, _window, cx| {
                    let _ = this.save_active_document(false, cx);
                },
            ))
            .child(toolbar_divider(palette))
            .child(self.render_format_group(palette, &shell))
            .child(div().flex_1())
            .child(self.render_mode_switcher(cx))
            .child(toolbar_divider(palette))
            .child(icon_button(
                "export",
                "导出",
                IconName::ExternalLink,
                palette,
                &shell,
                |this, _window, cx| {
                    this.show_export = !this.show_export;
                    this.show_settings = false;
                    this.show_quick_open = false;
                    this.show_recent_open = false;
                    this.update_preview_visibility(cx);
                    cx.notify();
                },
            ))
            .child(icon_button(
                "settings",
                "设置",
                IconName::Settings2,
                palette,
                &shell,
                |this, _window, cx| {
                    this.show_settings = !this.show_settings;
                    this.show_export = false;
                    this.show_quick_open = false;
                    this.show_recent_open = false;
                    this.update_preview_visibility(cx);
                    cx.notify();
                },
            ))
    }

    /// 工具栏左侧品牌区：标志方块 + 应用名 + 当前文档状态。
    fn render_brand(&self, palette: Palette) -> impl IntoElement {
        let document = self.workspace.active_document();
        let title = document
            .map(|document| document.title.clone())
            .unwrap_or_else(|| "未命名".to_string());
        let dirty = document.map(|document| document.dirty).unwrap_or(false);

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .pr_1()
            .child(
                div()
                    .w(px(26.0))
                    .h(px(26.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.0))
                    .bg(rgba(palette.warm))
                    .text_color(rgba(0xfdfefdff))
                    .child(Icon::new(IconName::BookOpen).size(px(15.0))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(palette.text))
                            .line_height(px(13.0))
                            .child("MoraNote"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .when(dirty, |this| {
                                this.child(
                                    div()
                                        .w(px(5.0))
                                        .h(px(5.0))
                                        .rounded_full()
                                        .bg(rgba(palette.warm)),
                                )
                            })
                            .child(
                                div()
                                    .max_w(px(150.0))
                                    .truncate()
                                    .text_color(rgba(palette.muted))
                                    .line_height(px(13.0))
                                    .child(title),
                            ),
                    ),
            )
    }

    /// Markdown 格式化按钮组，包裹在一个胶囊容器里成组呈现。
    fn render_format_group(&self, palette: Palette, shell: &Entity<AppShell>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .h(px(30.0))
            .px_1()
            .rounded(px(8.0))
            .bg(rgba(palette.surface_soft))
            .child(format_button(
                "fmt-bold",
                "B",
                "加粗",
                FontWeight::BOLD,
                palette,
                shell,
                |this, window, cx| this.insert_markdown("**bold**", window, cx),
            ))
            .child(format_button(
                "fmt-italic",
                "i",
                "斜体",
                FontWeight::NORMAL,
                palette,
                shell,
                |this, window, cx| this.insert_markdown("*italic*", window, cx),
            ))
            .child(format_button(
                "fmt-code",
                "</>",
                "行内代码",
                FontWeight::MEDIUM,
                palette,
                shell,
                |this, window, cx| this.insert_markdown("`code`", window, cx),
            ))
            .child(format_button(
                "fmt-quote",
                "❝",
                "引用",
                FontWeight::MEDIUM,
                palette,
                shell,
                |this, window, cx| this.insert_markdown("> quote\n", window, cx),
            ))
            .child(format_button(
                "fmt-list",
                "•",
                "无序列表",
                FontWeight::BOLD,
                palette,
                shell,
                |this, window, cx| this.insert_markdown("- item\n", window, cx),
            ))
            .child(format_button(
                "fmt-link",
                "↗",
                "链接",
                FontWeight::MEDIUM,
                palette,
                shell,
                |this, window, cx| this.insert_markdown("[text](url)", window, cx),
            ))
    }

    fn render_mode_switcher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().clone();
        TabBar::new("editor-mode-tabs")
            .segmented()
            .small()
            .selected_index(editor_mode_index(self.workspace.editor_mode))
            .on_click(move |index, _window, cx| {
                let mode = match *index {
                    0 => EditorMode::Source,
                    1 => EditorMode::Preview,
                    _ => EditorMode::Split,
                };
                let _ = shell.update(cx, |this, cx| this.set_mode(mode, cx));
            })
            .child(Tab::new().label(EditorMode::Source.label()))
            .child(Tab::new().label(EditorMode::Preview.label()))
            .child(Tab::new().label(EditorMode::Split.label()))
    }

    fn render_body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let sidebar_visible = self.show_sidebar && !self.config.focus_mode;
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_row()
            .gap_2p5()
            .px_2p5()
            .py_2p5()
            .overflow_hidden()
            .bg(rgba(palette.workspace))
            .when(sidebar_visible, |this| this.child(self.render_sidebar(cx)))
            .when(!sidebar_visible && !self.config.focus_mode, |this| {
                this.child(self.render_sidebar_rail(cx))
            })
            .child(self.render_editor_area(cx))
    }

    /// 侧栏收起后留在左侧的纵向图标栏（类似 VS Code 活动栏）：
    /// 顶部是展开按钮，下面是三种侧栏模式的快捷图标，点任意一个都会
    /// 展开侧栏并切到对应模式，当前模式高亮。整体样式与展开态侧栏统一。
    fn render_sidebar_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let shell = cx.entity().clone();
        let current = self.workspace.sidebar_mode;
        div()
            .id("sidebar-rail")
            .w(px(46.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .px(px(7.0))
            .py(px(8.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.panel))
            .shadow_md()
            .child(
                // 顶部展开按钮：强调色底，醒目且和工具栏的主操作呼应。
                div()
                    .id("sidebar-rail-expand")
                    .w(px(32.0))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(9.0))
                    .bg(rgba(palette.accent_soft))
                    .text_color(rgba(palette.accent_text))
                    .cursor(CursorStyle::PointingHand)
                    .tooltip(|window, cx| Tooltip::new("展开侧栏").build(window, cx))
                    .hover(move |style| style.bg(rgba(palette.accent)))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.show_sidebar = true;
                        cx.notify();
                    }))
                    .child(Icon::new(IconName::PanelLeftOpen).size_4()),
            )
            .child(
                div()
                    .w(px(22.0))
                    .h(px(1.0))
                    .my(px(2.0))
                    .bg(rgba(palette.border)),
            )
            .child(sidebar_rail_button(
                "rail-files",
                IconName::FolderClosed,
                SidebarMode::Files.label(),
                current == SidebarMode::Files,
                palette,
                &shell,
                SidebarMode::Files,
            ))
            .child(sidebar_rail_button(
                "rail-filelist",
                IconName::GalleryVerticalEnd,
                SidebarMode::FileList.label(),
                current == SidebarMode::FileList,
                palette,
                &shell,
                SidebarMode::FileList,
            ))
            .child(sidebar_rail_button(
                "rail-search",
                IconName::Search,
                SidebarMode::Search.label(),
                current == SidebarMode::Search,
                palette,
                &shell,
                SidebarMode::Search,
            ))
    }

    fn render_editor_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let mode = self.workspace.editor_mode;
        let has_preview = mode != EditorMode::Source;
        let has_toc = has_preview && !self.rendered.outline.is_empty();
        let editor_font = editor_font_stack(&self.config.font_family);

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .overflow_hidden()
            .rounded(px(12.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.surface))
            .shadow_md()
            .when(has_preview, |this| this.flex_row())
            .when(!has_preview, |this| this.flex_col())
            .when(mode != EditorMode::Preview, |this| {
                this.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .bg(rgba(palette.workspace))
                        .child(
                            div()
                                .size_full()
                                .mx_auto()
                                .max_w(px(860.0))
                                .px(px(56.0))
                                .py(px(40.0))
                                .bg(rgba(palette.surface))
                                .border_l_1()
                                .border_r_1()
                                .border_color(rgba(palette.border))
                                .font_family(editor_font)
                                .child(WithRemSize::new(
                                    // Input 字号 = 0.875rem、行高 = 1.25rem。把 rem 设为
                                    // font_size / 0.875，使显示字号恰为 font_size，且行高随之缩放。
                                    px(self.config.font_size as f32 / 0.875),
                                    Input::new(&self.editor).size_full().appearance(false),
                                )),
                        ),
                )
            })
            .when(mode == EditorMode::Split, |this| {
                this.child(
                    div()
                        .w(px(1.0))
                        .h_full()
                        .flex_shrink_0()
                        .bg(rgba(palette.border)),
                )
            })
            .when(mode != EditorMode::Source, |this| {
                this.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .bg(rgba(palette.surface))
                        .when_some(self.preview.clone(), |this, preview| {
                            // 按下鼠标时立刻让 WebView 成为第一响应者，
                            // 否则 macOS WKWebView 未取得焦点时拖选会整行整段选中，无法逐字选择。
                            this.on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    if let Some(preview) = &this.preview {
                                        let _ = preview.read(cx).focus();
                                    }
                                }),
                            )
                            .child(preview)
                        })
                        .when(self.preview.is_none(), |this| {
                            this.child(
                                div()
                                    .size_full()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .text_color(rgba(palette.muted))
                                    .child(
                                        Icon::new(IconName::TriangleAlert)
                                            .size_6()
                                            .text_color(rgba(palette.warm)),
                                    )
                                    .child("WebView 初始化失败，请检查平台 WebView 依赖。"),
                            )
                        }),
                )
            })
            .when(has_toc, |this| {
                this.child(
                    div()
                        .w(px(1.0))
                        .h_full()
                        .flex_shrink_0()
                        .bg(rgba(palette.border)),
                )
                .child(self.render_content_toc(cx))
            })
    }

    fn render_content_toc(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        div()
            .w(px(248.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgba(palette.panel))
            .child(
                div()
                    .h(px(48.0))
                    .flex_shrink_0()
                    .px_3()
                    .border_b_1()
                    .border_color(rgba(palette.border))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.0))
                                    .bg(rgba(palette.accent_soft))
                                    .child(
                                        Icon::new(IconName::BookOpen)
                                            .size(px(13.0))
                                            .text_color(rgba(palette.accent_text)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgba(palette.text))
                                    .child("目录"),
                            ),
                    )
                    .child(
                        div()
                            .rounded(px(999.0))
                            .px_2()
                            .py(px(2.0))
                            .text_xs()
                            .text_color(rgba(palette.muted))
                            .bg(rgba(palette.surface_soft))
                            .child(format!("{}", self.rendered.outline.len())),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_2()
                    .children(self.render_outline(cx)),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let shell = cx.entity().clone();
        div()
            .w(px(280.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .rounded(px(12.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.panel))
            .shadow_md()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_2()
                    .border_b_1()
                    .border_color(rgba(palette.border))
                    .child(div().flex_1().min_w_0().child(self.render_sidebar_tabs(cx)))
                    .child(icon_button(
                        "sidebar-collapse",
                        "收起目录",
                        IconName::PanelLeftClose,
                        palette,
                        &shell,
                        |this, _window, cx| {
                            this.show_sidebar = false;
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_2()
                    .when(self.workspace.sidebar_mode == SidebarMode::Files, |this| {
                        this.children(self.render_file_tree(cx, &self.workspace.file_tree, 0))
                    })
                    .when(
                        self.workspace.sidebar_mode == SidebarMode::FileList,
                        |this| this.children(self.render_file_list(cx)),
                    )
                    .when(self.workspace.sidebar_mode == SidebarMode::Search, |this| {
                        this.child(self.render_search(cx))
                    })
                    .when(
                        matches!(
                            self.workspace.sidebar_mode,
                            SidebarMode::Outline | SidebarMode::Recent
                        ),
                        |this| {
                            this.children(self.render_file_tree(cx, &self.workspace.file_tree, 0))
                        },
                    ),
            )
    }

    fn render_sidebar_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().clone();
        TabBar::new("sidebar-tabs")
            .segmented()
            .xsmall()
            .selected_index(sidebar_mode_index(self.workspace.sidebar_mode))
            .on_click(move |index, _window, cx| {
                let mode = match *index {
                    0 => SidebarMode::Files,
                    1 => SidebarMode::FileList,
                    _ => SidebarMode::Search,
                };
                let _ = shell.update(cx, |this, cx| this.set_sidebar_mode(mode, cx));
            })
            .child(Tab::new().label(SidebarMode::Files.label()))
            .child(Tab::new().label(SidebarMode::FileList.label()))
            .child(Tab::new().label(SidebarMode::Search.label()))
    }

    fn render_file_tree(
        &self,
        cx: &mut Context<Self>,
        entries: &[FileEntry],
        depth: usize,
    ) -> Vec<AnyElement> {
        let palette = self.palette();
        let mut elements = Vec::new();
        if entries.is_empty() && depth == 0 {
            elements.push(self.render_sidebar_empty(
                cx,
                "还没有打开文件夹",
                "打开一个文件夹即可浏览其中的 Markdown 文件。",
            ));
        }

        for entry in entries {
            let path = entry.path.clone();
            let is_dir = entry.is_dir;
            let is_active = self
                .workspace
                .active_document()
                .and_then(|document| document.path.as_ref())
                .map(|active| active == &path)
                .unwrap_or(false);
            elements.push(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "tree-row:{}",
                        path.display()
                    ))))
                    .h(px(30.0))
                    .pl(px((depth * 14 + 6) as f32))
                    .pr_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .rounded(px(7.0))
                    .text_sm()
                    .text_color(if is_dir {
                        rgba(palette.text)
                    } else if is_active {
                        rgba(palette.accent_text)
                    } else {
                        rgba(palette.text)
                    })
                    .when(is_active, |this| {
                        this.bg(rgba(palette.active))
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |style| style.bg(rgba(palette.hover)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        if is_dir {
                            toggle_entry(&mut this.workspace.file_tree, &path);
                        } else {
                            this.open_file(path.clone(), window, cx);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(14.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_dir, |this| {
                                this.child(
                                    Icon::new(if entry.expanded {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .size_3()
                                    .text_color(rgba(palette.subtle)),
                                )
                            }),
                    )
                    .when(is_dir, |this| {
                        this.child(
                            Icon::new(if entry.expanded {
                                IconName::FolderOpen
                            } else {
                                IconName::FolderClosed
                            })
                            .size_4()
                            .text_color(rgba(palette.warm)),
                        )
                    })
                    .when(!is_dir, |this| this.child(md_file_icon(palette)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(entry.name.clone()),
                    )
                    .into_any_element(),
            );

            if entry.is_dir && entry.expanded {
                elements.extend(self.render_file_tree(cx, &entry.children, depth + 1));
            }
        }
        elements
    }

    /// 侧栏统一的空状态卡片（图标 + 标题 + 说明 + 打开按钮）。
    fn render_sidebar_empty(
        &self,
        cx: &mut Context<Self>,
        title: &'static str,
        description: &'static str,
    ) -> AnyElement {
        let palette = self.palette();
        let shell = cx.entity().clone();
        div()
            .mt_2()
            .p_4()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .rounded(px(10.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.surface_soft))
            .child(
                div()
                    .w(px(40.0))
                    .h(px(40.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.0))
                    .bg(rgba(palette.accent_soft))
                    .child(
                        Icon::new(IconName::FolderOpen)
                            .size(px(20.0))
                            .text_color(rgba(palette.accent_text)),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba(palette.text))
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_center()
                    .text_color(rgba(palette.muted))
                    .child(description),
            )
            .child(
                Button::new("empty-open-folder")
                    .label("打开文件夹")
                    .icon(IconName::FolderOpen)
                    .small()
                    .outline()
                    .mt_1()
                    .on_click(move |_event, window, cx| {
                        let _ = shell.update(cx, |this, cx| this.open_folder_dialog(window, cx));
                    }),
            )
            .into_any_element()
    }

    fn render_file_list(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = self.palette();
        let files = self.workspace.all_files();
        if files.is_empty() {
            return vec![self.render_sidebar_empty(
                cx,
                "暂无文件",
                "打开一个文件夹后，这里会列出全部 Markdown 文件。",
            )];
        }

        files
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                let label = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                let p = path.clone();
                let is_active = self
                    .workspace
                    .active_document()
                    .and_then(|document| document.path.as_ref())
                    .map(|active| active == &path)
                    .unwrap_or(false);
                div()
                    .id(("file-list-row", index))
                    .h(px(30.0))
                    .px_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .rounded(px(7.0))
                    .text_sm()
                    .text_color(if is_active {
                        rgba(palette.accent_text)
                    } else {
                        rgba(palette.text)
                    })
                    .when(is_active, |this| {
                        this.bg(rgba(palette.active))
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |style| style.bg(rgba(palette.hover)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.open_file(p.clone(), window, cx);
                    }))
                    .child(md_file_icon(palette))
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .into_any_element()
            })
            .collect()
    }

    fn render_outline(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = self.palette();
        let outline = self
            .workspace
            .active_document()
            .map(|document| document.outline.clone())
            .unwrap_or_default();

        if outline.is_empty() {
            return vec![
                div()
                    .p_3()
                    .text_sm()
                    .text_color(rgba(palette.muted))
                    .child("当前文档还没有标题。")
                    .into_any_element(),
            ];
        }

        outline
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let anchor = item.anchor.clone();
                let level = item.level;
                div()
                    .id(("outline-row", index))
                    .min_h(px(28.0))
                    .pl(px(10.0 + (level.saturating_sub(1) as f32 * 12.0)))
                    .pr_2()
                    .py_1()
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .line_height(px(17.0))
                    .text_color(if level <= 2 {
                        rgba(palette.text)
                    } else {
                        rgba(palette.muted)
                    })
                    .when(level <= 2, |this| this.font_weight(FontWeight::MEDIUM))
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |style| style.bg(rgba(palette.hover)))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.scroll_preview_to(anchor.clone(), cx);
                    }))
                    .when(level <= 2, |this| {
                        this.child(
                            div()
                                .w(px(3.0))
                                .h(px(12.0))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(rgba(palette.accent)),
                        )
                    })
                    .child(div().flex_1().min_w_0().truncate().child(item.title))
                    .into_any_element()
            })
            .collect()
    }

    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let results = self.current_search_results();
        let has_query = !self.workspace.global_query.trim().is_empty();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Input::new(&self.global_search_input).prefix(IconName::Search))
            .when(has_query && results.is_empty(), |this| {
                this.child(
                    div()
                        .mt_1()
                        .p_3()
                        .text_sm()
                        .text_center()
                        .text_color(rgba(palette.muted))
                        .child("没有匹配的结果。"),
                )
            })
            .children(results.into_iter().enumerate().map(|(index, result)| {
                let p = result.path.clone();
                let name = result
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| result.path.display().to_string());
                div()
                    .id(("search-result", index))
                    .p_2()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgba(palette.border))
                    .bg(rgba(palette.surface))
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |style| {
                        style
                            .bg(rgba(palette.hover))
                            .border_color(rgba(palette.border_strong))
                    })
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.open_file(p.clone(), window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgba(palette.accent_text))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(palette.subtle))
                                    .flex_shrink_0()
                                    .child(format!("第 {} 行", result.line)),
                            ),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_xs()
                            .text_color(rgba(palette.muted))
                            .truncate()
                            .child(result.preview),
                    )
                    .into_any_element()
            }))
    }

    fn render_statusbar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let document = self.workspace.active_document();
        let stats = document
            .map(|document| document.stats.clone())
            .unwrap_or_default();
        let dirty = document.map(|document| document.dirty).unwrap_or(false);

        div()
            .h(px(26.0))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .border_t_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.chrome))
            .text_xs()
            .text_color(rgba(palette.muted))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(rgba(if dirty { palette.warm } else { palette.accent })),
                    )
                    .child(
                        div()
                            .text_color(rgba(palette.text))
                            .child(if dirty { "未保存" } else { "已保存" }),
                    ),
            )
            .child(status_divider(palette))
            .child(status_metric(format!("{} 字", stats.words)))
            .child(status_metric(format!("{} 字符", stats.chars)))
            .child(status_metric(format!("{} 行", stats.lines)))
            .child(status_metric(format!(
                "阅读约 {} 分钟",
                stats.reading_time_minutes
            )))
            .child(div().flex_1())
            .when(!self.status.is_empty(), |this| {
                this.child(
                    div()
                        .max_w(px(360.0))
                        .truncate()
                        .text_color(rgba(palette.subtle))
                        .child(self.status.clone()),
                )
                .child(status_divider(palette))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        Icon::new(theme_mode_icon(self.config.theme_mode))
                            .size_3()
                            .text_color(rgba(palette.accent)),
                    )
                    .child(self.config.theme_mode.label()),
            )
            .child(status_divider(palette))
            .child(self.workspace.editor_mode.label())
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().clone();
        let palette = self.palette();
        div()
            .id("settings-panel")
            .w(px(560.0))
            .max_w(relative(0.94))
            .max_h(relative(1.0))
            .flex_shrink_0()
            .overflow_hidden()
            .rounded(px(16.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.elevated))
            .shadow(elevated_shadow(palette))
            .flex()
            .flex_col()
            .on_click(cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(9.0))
                                    .bg(rgba(palette.accent_soft))
                                    .child(
                                        Icon::new(IconName::Settings2)
                                            .size(px(18.0))
                                            .text_color(rgba(palette.accent_text)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgba(palette.text))
                                            .child("设置"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(palette.muted))
                                            .child("调整写作体验、编辑行为和导出环境"),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("settings-close")
                            .icon(IconName::Close)
                            .ghost()
                            .small()
                            .text_color(rgba(palette.muted))
                            .on_click({
                                let shell = shell.clone();
                                move |_event, _window, cx| {
                                    let _ = shell.update(cx, |this, cx| this.close_panels(cx));
                                }
                            }),
                    ),
            )
            .child(settings_section(
                self.config.theme_mode,
                "外观",
                "主题和字体会同时影响编辑区与预览页。",
                vec![
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::Palette,
                        "主题",
                        theme_mode_description(self.config.theme_mode),
                        div()
                            .w(px(150.0))
                            .child(
                                Select::new(&self.theme_select)
                                    .small()
                                    .icon(theme_mode_icon(self.config.theme_mode))
                                    .menu_width(px(150.0)),
                            )
                            .into_any_element(),
                    ),
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::ALargeSmall,
                        "字体",
                        "正文、编辑区与预览页字体",
                        div()
                            .w_full()
                            .child(
                                Input::new(&self.settings_font_input).prefix(IconName::ALargeSmall),
                            )
                            .into_any_element(),
                    ),
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::ALargeSmall,
                        "字号",
                        "编辑器和预览页基础字号",
                        settings_stepper(
                            self.config.theme_mode,
                            format!("{}px", self.config.font_size),
                            "font-size-decrease",
                            "font-size-increase",
                            &shell,
                            |this, _window, cx| this.adjust_font_size(-1, cx),
                            |this, _window, cx| this.adjust_font_size(1, cx),
                        ),
                    ),
                ],
            ))
            .child(settings_section(
                self.config.theme_mode,
                "布局",
                "控制启动后的默认视图和写作模式。",
                vec![
                    settings_item(
                        self.config.theme_mode,
                        IconName::PanelRight,
                        "默认视图",
                        "Source、Preview、Split 三种模式",
                        self.workspace.editor_mode.label(),
                        Some(action_button(
                            "editor-mode-cycle",
                            "切换",
                            IconName::ChevronsUpDown,
                            &shell,
                            |this, _window, cx| this.cycle_editor_mode(cx),
                        )),
                    ),
                    settings_item(
                        self.config.theme_mode,
                        IconName::PanelLeft,
                        "默认侧栏",
                        "文件树、文件列表或搜索",
                        self.workspace.sidebar_mode.label(),
                        Some(action_button(
                            "sidebar-mode-cycle",
                            "切换",
                            IconName::ChevronsUpDown,
                            &shell,
                            |this, _window, cx| this.cycle_sidebar_mode(cx),
                        )),
                    ),
                    settings_item(
                        self.config.theme_mode,
                        IconName::Eye,
                        "专注模式",
                        "隐藏侧栏，让编辑区成为主视觉",
                        bool_label(self.config.focus_mode),
                        Some(action_button(
                            "focus-mode-toggle",
                            "切换",
                            IconName::Eye,
                            &shell,
                            |this, _window, cx| this.toggle_focus_mode_setting(cx),
                        )),
                    ),
                    settings_item(
                        self.config.theme_mode,
                        IconName::Frame,
                        "打字机模式",
                        "保留配置项，后续可接滚动定位行为",
                        bool_label(self.config.typewriter_mode),
                        Some(action_button(
                            "typewriter-mode-toggle",
                            "切换",
                            IconName::Frame,
                            &shell,
                            |this, _window, cx| this.toggle_typewriter_mode_setting(cx),
                        )),
                    ),
                ],
            ))
            .child(settings_section(
                self.config.theme_mode,
                "编辑器",
                "控制源码编辑页的显示和保存行为。",
                vec![
                    settings_item(
                        self.config.theme_mode,
                        IconName::ALargeSmall,
                        "行号",
                        "在源码编辑器左侧显示行号",
                        bool_label(self.config.line_numbers),
                        Some(action_button(
                            "line-toggle",
                            "切换",
                            IconName::ALargeSmall,
                            &shell,
                            |this, window, cx| {
                                this.toggle_line_numbers(window, cx);
                            },
                        )),
                    ),
                    settings_item(
                        self.config.theme_mode,
                        IconName::PanelRight,
                        "软换行",
                        "长段落自动换行，减少横向滚动",
                        bool_label(self.config.soft_wrap),
                        Some(action_button(
                            "wrap-toggle",
                            "切换",
                            IconName::PanelRight,
                            &shell,
                            |this, window, cx| {
                                this.toggle_soft_wrap(window, cx);
                            },
                        )),
                    ),
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::CaseSensitive,
                        "Tab 宽度",
                        "代码块和缩进使用的空格宽度",
                        settings_stepper(
                            self.config.theme_mode,
                            format!("{}", self.config.tab_size),
                            "tab-size-decrease",
                            "tab-size-increase",
                            &shell,
                            |this, window, cx| this.adjust_tab_size(-1, window, cx),
                            |this, window, cx| this.adjust_tab_size(1, window, cx),
                        ),
                    ),
                    settings_item(
                        self.config.theme_mode,
                        IconName::Check,
                        "自动保存",
                        "编辑后自动写回当前文件",
                        bool_label(self.config.auto_save),
                        Some(action_button(
                            "auto-save-toggle",
                            "切换",
                            IconName::Check,
                            &shell,
                            |this, _window, cx| this.toggle_auto_save(cx),
                        )),
                    ),
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::File,
                        "支持扩展名",
                        "默认只打开 .md，可用逗号分隔多个扩展名",
                        div()
                            .w_full()
                            .child(
                                Input::new(&self.settings_extensions_input).prefix(IconName::File),
                            )
                            .into_any_element(),
                    ),
                ],
            ))
            .child(settings_section(
                self.config.theme_mode,
                "导出",
                "Word 已内置可用；其他高级格式可配置外部工具作为增强。",
                vec![
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::ExternalLink,
                        "Pandoc",
                        "用于 EPUB、LaTeX、RevealJS 等高级格式",
                        settings_path_control(
                            self.config.theme_mode,
                            tool_status(self.config.pandoc_path.as_deref(), "可选"),
                            self.config.pandoc_path.is_some(),
                            "pandoc-path",
                            "pandoc-clear",
                            &shell,
                            |this, _window, cx| this.choose_pandoc(cx),
                            |this, _window, cx| this.clear_pandoc(cx),
                        ),
                    ),
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::ExternalLink,
                        "Chromium",
                        "用于 PDF、PNG、JPEG 预览效果导出",
                        settings_path_control(
                            self.config.theme_mode,
                            tool_status(self.config.chromium_path.as_deref(), "自动检测"),
                            self.config.chromium_path.is_some(),
                            "chromium-path",
                            "chromium-clear",
                            &shell,
                            |this, _window, cx| this.choose_chromium(cx),
                            |this, _window, cx| this.clear_chromium(cx),
                        ),
                    ),
                    settings_control_item(
                        self.config.theme_mode,
                        IconName::FolderOpen,
                        "默认导出目录",
                        "未配置时使用当前文档所在目录",
                        settings_path_control(
                            self.config.theme_mode,
                            path_label(self.config.default_export_dir.as_deref()),
                            self.config.default_export_dir.is_some(),
                            "export-dir",
                            "export-dir-clear",
                            &shell,
                            |this, _window, cx| this.choose_export_dir(cx),
                            |this, _window, cx| this.clear_export_dir(cx),
                        ),
                    ),
                ],
            ))
            .child(settings_section(
                self.config.theme_mode,
                "系统",
                "系统集成和文件关联。",
                vec![settings_item(
                    self.config.theme_mode,
                    IconName::File,
                    "Markdown 双击打开",
                    "启动时自动把 .md 默认编辑器注册为 MoraNote",
                    bool_label(self.config.claim_markdown_association),
                    Some(action_button(
                        "claim-markdown-association",
                        "切换",
                        IconName::File,
                        &shell,
                        |this, _window, cx| this.toggle_markdown_association(cx),
                    )),
                )],
            ))
            .when(!self.platform_diagnostics.is_empty(), |this| {
                this.child(settings_section(
                    self.config.theme_mode,
                    "运行环境",
                    "当前平台运行环境诊断。",
                    self.platform_diagnostics
                        .iter()
                        .enumerate()
                        .map(|(index, message)| {
                            settings_item(
                                self.config.theme_mode,
                                IconName::Check,
                                format!("诊断 {}", index + 1),
                                message.clone(),
                                "已检测",
                                None,
                            )
                        })
                        .collect(),
                ))
            }),
            )
    }

    fn render_export_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().clone();
        let palette = self.palette();
        div()
            .id("export-panel")
            .absolute()
            .top(px(70.0))
            .right(px(16.0))
            .w(px(340.0))
            .max_w(relative(0.92))
            .rounded(px(14.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.elevated))
            .shadow(elevated_shadow(palette))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .on_click(cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .pb_1()
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.0))
                            .bg(rgba(palette.accent_soft))
                            .child(
                                Icon::new(IconName::ExternalLink)
                                    .size(px(15.0))
                                    .text_color(rgba(palette.accent_text)),
                            ),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(palette.text))
                            .child("导出"),
                    ),
            )
            .when(self.exporting, |this| {
                let shell = shell.clone();
                this.child(
                    div()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(rgba(palette.border))
                        .bg(rgba(palette.warm_soft))
                        .p_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .text_sm()
                        .text_color(rgba(palette.warm))
                        .child("正在导出当前预览，请稍候...")
                        .child(
                            Button::new("cancel-export-wait")
                                .label("停止等待")
                                .icon(IconName::CircleX)
                                .small()
                                .ghost()
                                .on_click(move |_event, _window, cx| {
                                    let _ = shell.update(cx, |this, cx| {
                                        this.cancel_export_wait(cx);
                                    });
                                }),
                        ),
                )
            })
            .children(
                ExportFormat::all()
                    .into_iter()
                    .enumerate()
                    .map(|(index, format)| {
                        let label = format.label();
                        let shell = shell.clone();
                        Button::new(("export-format", index))
                            .label(label)
                            .icon(IconName::ExternalLink)
                            .ghost()
                            .text_color(rgba(palette.text))
                            .disabled(self.exporting)
                            .on_click(move |_event, window, cx| {
                                let _ = shell.update(cx, |this, cx| {
                                    this.export_current(format, window, cx);
                                });
                            })
                            .into_any_element()
                    }),
            )
    }

    fn render_quick_open(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let matches = self.matching_files();
        let has_query = !self.workspace.quick_open_query.trim().is_empty();
        div()
            .id("quick-open-panel")
            .absolute()
            .top(px(80.0))
            .left_0()
            .right_0()
            .mx_auto()
            .w(px(620.0))
            .max_w(relative(0.9))
            .rounded(px(14.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.elevated))
            .shadow(elevated_shadow(palette))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .on_click(cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }))
            .child(Input::new(&self.quick_open_input).prefix(IconName::Search))
            .when(has_query && matches.is_empty(), |this| {
                this.child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_center()
                        .text_color(rgba(palette.muted))
                        .child("没有匹配的文件。"),
                )
            })
            .children(matches.into_iter().enumerate().map(|(index, path)| {
                let p = path.clone();
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                let full = path.display().to_string();
                div()
                    .id(("quick-open-row", index))
                    .px_2()
                    .py_1p5()
                    .rounded(px(8.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |style| style.bg(rgba(palette.hover)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.open_file(p.clone(), window, cx);
                        this.show_quick_open = false;
                        this.update_preview_visibility(cx);
                        cx.notify();
                    }))
                    .child(md_file_icon(palette))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgba(palette.text))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(palette.subtle))
                                    .truncate()
                                    .child(full),
                            ),
                    )
                    .into_any_element()
            }))
    }

    fn render_recent_open(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let folder_rows = self.render_recent_folder_rows(cx);
        let file_rows = self.render_recent_file_rows(cx);

        let palette = self.palette();
        div()
            .id("recent-open-panel")
            .w(px(780.0))
            .max_w(relative(0.94))
            .flex_shrink_0()
            .rounded(px(16.0))
            .border_1()
            .border_color(rgba(palette.border))
            .bg(rgba(palette.elevated))
            .shadow(elevated_shadow(palette))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .on_click(cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(9.0))
                                    .bg(rgba(palette.accent_soft))
                                    .child(
                                        Icon::new(IconName::Calendar)
                                            .size(px(18.0))
                                            .text_color(rgba(palette.accent_text)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgba(palette.text))
                                            .child("最近打开"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(palette.muted))
                                            .child("启动时会自动恢复上次打开的文件夹和文档"),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("close-recent-open")
                            .icon(IconName::Close)
                            .ghost()
                            .small()
                            .text_color(rgba(palette.muted))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.show_recent_open = false;
                                this.update_preview_visibility(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .h(px(430.0))
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .rounded(px(12.0))
                            .border_1()
                            .border_color(rgba(palette.border))
                            .bg(rgba(palette.surface))
                            .overflow_hidden()
                            .child(recent_column_header(
                                self.config.theme_mode,
                                IconName::FolderOpen,
                                "文件夹",
                                self.config.recent_folders.len(),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scrollbar()
                                    .p_2()
                                    .children(folder_rows),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .rounded(px(12.0))
                            .border_1()
                            .border_color(rgba(palette.border))
                            .bg(rgba(palette.surface))
                            .overflow_hidden()
                            .child(recent_column_header(
                                self.config.theme_mode,
                                IconName::File,
                                "Markdown 文件",
                                self.workspace.recent_files.len(),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scrollbar()
                                    .p_2()
                                    .children(file_rows),
                            ),
                    ),
            )
    }

    fn render_recent_folder_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let folders = self
            .config
            .recent_folders
            .iter()
            .filter(|path| !path.as_os_str().is_empty())
            .cloned()
            .collect::<Vec<_>>();

        if folders.is_empty() {
            return vec![self.render_recent_empty("打开文件夹后会记录在这里。")];
        }

        folders
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                let path_text = path.display().to_string();
                let exists = path.is_dir();
                let p = path.clone();

                div()
                    .id(("recent-folder-row", index))
                    .p_2()
                    .mb_1()
                    .rounded_md()
                    .cursor(CursorStyle::PointingHand)
                    .hover({
                        let hover = theme_hover(self.config.theme_mode);
                        move |style| style.bg(rgba(hover))
                    })
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if p.is_dir() {
                            this.open_folder(p.clone(), true, cx);
                            this.show_recent_open = false;
                            this.update_preview_visibility(cx);
                        } else {
                            this.status = format!("历史文件夹不存在: {}", p.display());
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::FolderOpen)
                                    .size_4()
                                    .text_color(if exists {
                                        rgba(theme_warm(self.config.theme_mode))
                                    } else {
                                        rgba(theme_muted(self.config.theme_mode))
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if exists {
                                        rgba(theme_text(self.config.theme_mode))
                                    } else {
                                        rgba(theme_muted(self.config.theme_mode))
                                    })
                                    .child(name),
                            )
                            .when(!exists, |this| {
                                this.child(
                                    div()
                                        .rounded(px(999.0))
                                        .px_2()
                                        .py(px(2.0))
                                        .text_xs()
                                        .text_color(rgba(theme_warm(self.config.theme_mode)))
                                        .bg(rgba(theme_warm_soft(self.config.theme_mode)))
                                        .child("缺失"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .mt_1()
                            .pl(px(24.0))
                            .truncate()
                            .text_xs()
                            .text_color(rgba(theme_muted(self.config.theme_mode)))
                            .child(path_text),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_recent_file_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let files = self
            .workspace
            .recent_files
            .iter()
            .filter(|path| self.is_supported_path(path))
            .cloned()
            .collect::<Vec<_>>();

        if files.is_empty() {
            return vec![self.render_recent_empty("打开 Markdown 文件后会记录在这里。")];
        }

        files
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                let parent = path
                    .parent()
                    .map(|parent| parent.display().to_string())
                    .unwrap_or_default();
                let exists = path.exists();
                let p = path.clone();

                div()
                    .id(("recent-file-row", index))
                    .p_2()
                    .mb_1()
                    .rounded_md()
                    .cursor(CursorStyle::PointingHand)
                    .hover({
                        let hover = theme_hover(self.config.theme_mode);
                        move |style| style.bg(rgba(hover))
                    })
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        if p.exists() && this.is_supported_path(&p) {
                            this.open_file(p.clone(), window, cx);
                            this.show_recent_open = false;
                            this.update_preview_visibility(cx);
                        } else {
                            this.status = format!("历史文件不可用: {}", p.display());
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(md_file_icon(self.palette()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if exists {
                                        rgba(theme_text(self.config.theme_mode))
                                    } else {
                                        rgba(theme_muted(self.config.theme_mode))
                                    })
                                    .child(file_name),
                            )
                            .when(!exists, |this| {
                                this.child(
                                    div()
                                        .rounded(px(999.0))
                                        .px_2()
                                        .py(px(2.0))
                                        .text_xs()
                                        .text_color(rgba(theme_warm(self.config.theme_mode)))
                                        .bg(rgba(theme_warm_soft(self.config.theme_mode)))
                                        .child("缺失"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .mt_1()
                            .pl(px(30.0))
                            .truncate()
                            .text_xs()
                            .text_color(rgba(theme_muted(self.config.theme_mode)))
                            .child(parent),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_recent_empty(&self, message: &'static str) -> AnyElement {
        div()
            .p_3()
            .text_sm()
            .text_color(rgba(theme_muted(self.config.theme_mode)))
            .child(message)
            .into_any_element()
    }
}

fn action_button<F>(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    shell: &Entity<AppShell>,
    f: F,
) -> Button
where
    F: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
{
    let shell = shell.clone();
    Button::new(id)
        .label(label)
        .icon(icon)
        .ghost()
        .small()
        .on_click(move |_event, window, cx| {
            let _ = shell.update(cx, |this, cx| f(this, window, cx));
        })
}

/// 仅图标的工具栏按钮，文字颜色跟随主题。
fn icon_button<F>(
    id: &'static str,
    tooltip: &'static str,
    icon: IconName,
    palette: Palette,
    shell: &Entity<AppShell>,
    f: F,
) -> Button
where
    F: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
{
    let shell = shell.clone();
    Button::new(id)
        .icon(icon)
        .ghost()
        .small()
        .tooltip(tooltip)
        .text_color(rgba(palette.text))
        .on_click(move |_event, window, cx| {
            let _ = shell.update(cx, |this, cx| f(this, window, cx));
        })
}

/// 收起侧栏的纵向图标栏里的单个模式按钮：当前模式高亮（强调色底 +
/// 左侧指示条），其余为静默图标带 hover。点击会展开侧栏并切到该模式。
fn sidebar_rail_button(
    id: &'static str,
    icon: IconName,
    tooltip: &'static str,
    active: bool,
    palette: Palette,
    shell: &Entity<AppShell>,
    mode: SidebarMode,
) -> impl IntoElement {
    let shell = shell.clone();
    let hover = palette.hover;
    div()
        .id(id)
        .relative()
        .w(px(32.0))
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(9.0))
        .cursor(CursorStyle::PointingHand)
        .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
        .when(active, |this| {
            this.bg(rgba(palette.accent_soft))
                .text_color(rgba(palette.accent_text))
                // 左侧的小指示条，强化“当前选中”的状态。
                .child(
                    div()
                        .absolute()
                        .left(px(-7.0))
                        .top(px(8.0))
                        .w(px(3.0))
                        .h(px(16.0))
                        .rounded_r(px(3.0))
                        .bg(rgba(palette.accent)),
                )
        })
        .when(!active, |this| {
            this.text_color(rgba(palette.subtle))
                .hover(move |style| style.bg(rgba(hover)).text_color(rgba(palette.text)))
        })
        .on_click(move |_event, _window, cx| {
            let _ = shell.update(cx, |this, cx| {
                this.show_sidebar = true;
                this.set_sidebar_mode(mode, cx);
                cx.notify();
            });
        })
        .child(Icon::new(icon).size_4())
}

/// 紧凑的格式化按钮（方形、文字符号、跟随主题、带 hover/tooltip）。
fn format_button<F>(
    id: &'static str,
    glyph: &'static str,
    _tooltip: &'static str,
    weight: FontWeight,
    palette: Palette,
    shell: &Entity<AppShell>,
    f: F,
) -> impl IntoElement
where
    F: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
{
    let shell = shell.clone();
    let hover = palette.hover;
    div()
        .id(id)
        .w(px(26.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .text_sm()
        .font_weight(weight)
        .text_color(rgba(palette.muted))
        .cursor(CursorStyle::PointingHand)
        .hover(move |style| style.bg(rgba(hover)).text_color(rgba(palette.text)))
        .on_click(move |_event, window, cx| {
            let _ = shell.update(cx, |this, cx| f(this, window, cx));
        })
        .child(glyph)
}

/// 工具栏中的竖向分隔条，颜色跟随主题。
fn toolbar_divider(palette: Palette) -> Div {
    div()
        .w(px(1.0))
        .h(px(18.0))
        .flex_shrink_0()
        .bg(rgba(palette.border))
        .mx_1()
}

/// 浮层卡片的柔和投影，颜色来自调色板，保证三主题协调。
fn elevated_shadow(palette: Palette) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: rgba(palette.shadow).into(),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(48.0),
            spread_radius: px(0.0),
        },
        BoxShadow {
            color: rgba(palette.shadow).into(),
            offset: point(px(0.0), px(2.0)),
            blur_radius: px(8.0),
            spread_radius: px(0.0),
        },
    ]
}

/// 状态栏中的细分隔条。
fn status_divider(palette: Palette) -> Div {
    div()
        .w(px(1.0))
        .h(px(11.0))
        .flex_shrink_0()
        .bg(rgba(palette.border))
}

/// 状态栏中的统计文字项。
fn status_metric(text: String) -> Div {
    div().flex_shrink_0().child(text)
}

/// 当前主题对应的图标。
fn theme_mode_icon(mode: ThemeMode) -> IconName {
    match mode {
        ThemeMode::Light => IconName::Sun,
        ThemeMode::Sepia => IconName::BookOpen,
        ThemeMode::Dark => IconName::Moon,
        ThemeMode::Typora => IconName::File,
    }
}

/// 把主题下拉里选中的标签文字映射回 `ThemeMode`。
fn theme_mode_from_label(label: &str) -> Option<ThemeMode> {
    ThemeMode::all().into_iter().find(|mode| mode.label() == label)
}

/// 设置面板中每个主题的简短说明。
fn theme_mode_description(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "Morandi 浅色配色",
        ThemeMode::Sepia => "Morandi 护眼暖色",
        ThemeMode::Dark => "Morandi 深色配色",
        ThemeMode::Typora => "Typora / GitHub 简洁风格",
    }
}

fn recent_column_header(
    theme: ThemeMode,
    icon: IconName,
    title: &'static str,
    count: usize,
) -> impl IntoElement {
    div()
        .h(px(44.0))
        .flex_shrink_0()
        .px_3()
        .border_b_1()
        .border_color(rgba(theme_border(theme)))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgba(theme_text(theme)))
                .child(
                    Icon::new(icon)
                        .size_4()
                        .text_color(rgba(theme_accent(theme))),
                )
                .child(title),
        )
        .child(
            div()
                .rounded_md()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(rgba(theme_muted(theme)))
                .bg(rgba(theme_surface_soft(theme)))
                .child(format!("{} 项", count)),
        )
}

fn settings_section(
    theme: ThemeMode,
    title: impl Into<String>,
    description: impl Into<String>,
    items: Vec<AnyElement>,
) -> impl IntoElement {
    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(rgba(theme_border(theme)))
        .bg(rgba(theme_surface(theme)))
        .shadow_sm()
        .overflow_hidden()
        .child(
            div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgba(theme_border(theme)))
                .bg(rgba(theme_surface_soft(theme)))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgba(theme_text(theme)))
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgba(theme_muted(theme)))
                        .child(description.into()),
                ),
        )
        .child(div().flex().flex_col().children(items))
}

fn settings_item(
    theme: ThemeMode,
    icon: IconName,
    title: impl Into<String>,
    description: impl Into<String>,
    value: impl Into<String>,
    action: Option<Button>,
) -> AnyElement {
    let palette = Palette::for_mode(theme);
    div()
        .min_h(px(60.0))
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgba(palette.border))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(30.0))
                .h(px(30.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .bg(rgba(palette.accent_soft))
                .text_color(rgba(palette.accent_text))
                .child(Icon::new(icon).size(px(15.0))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgba(palette.text))
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgba(palette.muted))
                        .truncate()
                        .child(description.into()),
                ),
        )
        .child(
            div()
                .max_w(px(170.0))
                .min_w(px(64.0))
                .text_sm()
                .text_color(rgba(palette.text))
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(value.into()),
        )
        .when_some(action, |this, action| {
            this.child(div().min_w(px(70.0)).flex().justify_end().child(action))
        })
        .into_any_element()
}

fn settings_control_item(
    theme: ThemeMode,
    icon: IconName,
    title: impl Into<String>,
    description: impl Into<String>,
    control: AnyElement,
) -> AnyElement {
    let palette = Palette::for_mode(theme);
    div()
        .min_h(px(66.0))
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgba(palette.border))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(30.0))
                .h(px(30.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .bg(rgba(palette.accent_soft))
                .text_color(rgba(palette.accent_text))
                .child(Icon::new(icon).size(px(15.0))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgba(palette.text))
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgba(palette.muted))
                        .truncate()
                        .child(description.into()),
                ),
        )
        .child(
            div()
                .w(px(238.0))
                .flex_shrink_0()
                .flex()
                .justify_end()
                .child(control),
        )
        .into_any_element()
}

fn settings_stepper<FDec, FInc>(
    theme: ThemeMode,
    value: impl Into<String>,
    decrement_id: &'static str,
    increment_id: &'static str,
    shell: &Entity<AppShell>,
    decrement: FDec,
    increment: FInc,
) -> AnyElement
where
    FDec: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
    FInc: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
{
    let decrement_shell = shell.clone();
    let increment_shell = shell.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            Button::new(decrement_id)
                .icon(IconName::Minus)
                .ghost()
                .small()
                .text_color(rgba(theme_text(theme)))
                .on_click(move |_event, window, cx| {
                    let _ = decrement_shell.update(cx, |this, cx| decrement(this, window, cx));
                }),
        )
        .child(
            div()
                .w(px(72.0))
                .h(px(30.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(rgba(theme_border(theme)))
                .bg(rgba(theme_surface_soft(theme)))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba(theme_text(theme)))
                .child(value.into()),
        )
        .child(
            Button::new(increment_id)
                .icon(IconName::Plus)
                .ghost()
                .small()
                .text_color(rgba(theme_text(theme)))
                .on_click(move |_event, window, cx| {
                    let _ = increment_shell.update(cx, |this, cx| increment(this, window, cx));
                }),
        )
        .into_any_element()
}

fn settings_path_control<FChoose, FClear>(
    theme: ThemeMode,
    value: impl Into<String>,
    has_value: bool,
    choose_id: &'static str,
    clear_id: &'static str,
    shell: &Entity<AppShell>,
    choose: FChoose,
    clear: FClear,
) -> AnyElement
where
    FChoose: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
    FClear: Fn(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
{
    let choose_shell = shell.clone();
    let clear_shell = shell.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap_1()
        .child(
            div()
                .w(px(112.0))
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(rgba(theme_muted(theme)))
                .child(value.into()),
        )
        .child(
            Button::new(choose_id)
                .icon(IconName::FolderOpen)
                .label("选择")
                .ghost()
                .small()
                .text_color(rgba(theme_text(theme)))
                .on_click(move |_event, window, cx| {
                    let _ = choose_shell.update(cx, |this, cx| choose(this, window, cx));
                }),
        )
        .child(
            Button::new(clear_id)
                .icon(IconName::Delete)
                .ghost()
                .small()
                .disabled(!has_value)
                .text_color(rgba(theme_text(theme)))
                .on_click(move |_event, window, cx| {
                    let _ = clear_shell.update(cx, |this, cx| clear(this, window, cx));
                }),
        )
        .into_any_element()
}

fn md_file_icon(palette: Palette) -> impl IntoElement {
    div()
        .w(px(22.0))
        .h(px(16.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .bg(rgba(palette.accent_soft))
        .text_color(rgba(palette.accent_text))
        .text_size(px(9.0))
        .font_weight(FontWeight::BOLD)
        .child("MD")
}

fn editor_mode_index(mode: EditorMode) -> usize {
    match mode {
        EditorMode::Source => 0,
        EditorMode::Preview => 1,
        EditorMode::Split => 2,
    }
}

fn sidebar_mode_index(mode: SidebarMode) -> usize {
    match mode {
        SidebarMode::Files => 0,
        SidebarMode::FileList => 1,
        SidebarMode::Recent => 0,
        SidebarMode::Outline => 0,
        SidebarMode::Search => 2,
    }
}

fn preview_theme_name(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
        ThemeMode::Sepia => "sepia",
        ThemeMode::Typora => "typora",
    }
}

/// 源码编辑区字体：优先使用用户配置字体，回退到等宽字体，保证代码对齐。
fn editor_font_stack(font_family: &str) -> String {
    let primary = font_family.replace(['"', '\''], "").trim().to_string();
    if primary.is_empty() {
        "JetBrains Mono NL".to_string()
    } else {
        primary
    }
}

fn preview_font_stack(font_family: &str) -> String {
    let primary = font_family.replace(['"', '\''], "").trim().to_string();
    if primary.is_empty() {
        "\"Alibaba PuHuiTi 3.0\", \"PingFang SC\", sans-serif".to_string()
    } else {
        format!("\"{primary}\", \"Alibaba PuHuiTi 3.0\", \"PingFang SC\", sans-serif")
    }
}

fn parse_extensions(value: &str) -> Vec<String> {
    let mut extensions = Vec::new();
    for extension in value.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace()) {
        let extension = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if extension.is_empty() || extensions.contains(&extension) {
            continue;
        }
        extensions.push(extension);
    }
    extensions
}

fn path_has_supported_extension(path: &Path, extensions: &[String]) -> bool {
    let Some(extension) = path.extension() else {
        return false;
    };
    let extension = extension.to_string_lossy();
    extensions
        .iter()
        .any(|item| extension.eq_ignore_ascii_case(item.trim_start_matches('.')))
}

fn file_snapshot(path: &Path) -> Option<FileSnapshot> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(FileSnapshot {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

fn file_set_from_entries(entries: &[FileEntry]) -> HashSet<PathBuf> {
    let mut files = HashSet::new();
    collect_files(entries, &mut files);
    files
}

fn collect_files(entries: &[FileEntry], files: &mut HashSet<PathBuf>) {
    for entry in entries {
        if entry.is_dir {
            collect_files(&entry.children, files);
        } else {
            files.insert(entry.path.clone());
        }
    }
}

fn expanded_dir_set(entries: &[FileEntry]) -> HashSet<PathBuf> {
    let mut expanded = HashSet::new();
    collect_expanded_dirs(entries, &mut expanded);
    expanded
}

fn collect_expanded_dirs(entries: &[FileEntry], expanded: &mut HashSet<PathBuf>) {
    for entry in entries {
        if entry.is_dir {
            if entry.expanded {
                expanded.insert(entry.path.clone());
            }
            collect_expanded_dirs(&entry.children, expanded);
        }
    }
}

fn apply_expanded_dirs(entries: &mut [FileEntry], expanded: &HashSet<PathBuf>) {
    for entry in entries {
        if entry.is_dir {
            entry.expanded = expanded.contains(&entry.path);
            apply_expanded_dirs(&mut entry.children, expanded);
        }
    }
}

fn theme_text(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).text
}

fn theme_muted(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).muted
}

fn theme_accent(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).accent
}

fn theme_surface(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).surface
}

fn theme_surface_soft(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).surface_soft
}

fn theme_hover(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).hover
}

fn theme_border(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).border
}

fn theme_warm(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).warm
}

fn theme_warm_soft(theme: ThemeMode) -> u32 {
    Palette::for_mode(theme).warm_soft
}

fn bool_label(value: bool) -> &'static str {
    if value { "开启" } else { "关闭" }
}

fn tool_status(path: Option<&Path>, fallback: &'static str) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| fallback.to_string())
}

fn path_label(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "未配置".to_string())
}

fn with_rendered(mut document: DocumentState, rendered: &RenderedDocument) -> DocumentState {
    document.outline = rendered.outline.clone();
    document.stats = rendered.stats.clone();
    document
}

fn platform_diagnostics() -> Vec<String> {
    let mut messages = Vec::new();
    #[cfg(target_os = "linux")]
    {
        messages.push("Linux 需要系统安装 WebKitGTK/GTK 运行时以启用 WebView。".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        messages.push("Windows 需要 WebView2 Runtime。".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        messages.push("macOS 使用系统 WKWebView。".to_string());
    }
    messages
}

/// 把子元素的布局与绘制都置于指定 `rem_size` 上下文中执行的包装元素。
///
/// `gpui_component` 的 `Input` 把行高写死为 `Rems(1.25)`（相对根字号 rem），
/// 因此仅调整字号会导致行高不跟随、文字挤在一起。用这个包装把源码编辑区的
/// rem_size 设为期望字号，字号与行高便会一起等比缩放，且不影响外部 UI。
struct WithRemSize {
    rem_size: Pixels,
    child: AnyElement,
}

impl WithRemSize {
    fn new(rem_size: Pixels, child: impl IntoElement) -> Self {
        Self {
            rem_size,
            child: child.into_any_element(),
        }
    }
}

impl IntoElement for WithRemSize {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WithRemSize {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id =
            window.with_rem_size(Some(self.rem_size), |window| self.child.request_layout(window, cx));
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        window.with_rem_size(Some(self.rem_size), |window| self.child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_rem_size(Some(self.rem_size), |window| self.child.paint(window, cx));
    }
}
