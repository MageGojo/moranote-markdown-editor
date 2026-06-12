mod actions;
mod app;
mod export;
mod model;
mod platform;
mod preview;
mod settings;
mod workspace;

use actions::*;
use app::{AppShell, SharedOpenFiles, claim_system_markdown_association};
use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn main() {
    let pending_open_files: SharedOpenFiles = Arc::new(Mutex::new(initial_open_paths()));
    let open_files = pending_open_files.clone();
    let app = Application::new().with_assets(Assets);
    app.on_open_urls(move |urls| {
        let paths = urls
            .into_iter()
            .filter_map(path_from_open_url)
            .filter(|path| is_markdown_path(path))
            .collect::<Vec<_>>();
        if let Ok(mut pending) = open_files.lock() {
            pending.extend(paths);
        }
    });

    app.run(move |cx: &mut App| {
        // 必须在任何 WKWebView 创建之前执行，消除 macOS 文本拖拽消歧延迟，
        // 修复预览区拖选退化成整行的问题。
        platform::prepare_native_environment();
        gpui_component::init(cx);
        register_morandi_fonts(cx);
        if settings::Settings::load().claim_markdown_association {
            claim_system_markdown_association();
        }
        bind_keys(cx);
        let pending_open_files = pending_open_files.clone();

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1440.0), px(920.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("MoraNote".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |window, cx| {
                let shell = cx.new(|cx| AppShell::new(window, cx, pending_open_files.clone()));
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .expect("failed to open markdown editor window");
    });
}

fn register_morandi_fonts(cx: &mut App) {
    let font_dir = preview::morandi_theme_asset_dir().join("morandigarden");
    let fonts = [
        "AlibabaPuHuiTi-3-65-Medium.ttf",
        "AlibabaPuHuiTi-3-105-Heavy.ttf",
        "JetBrainsMonoNL-Regular.ttf",
        "JetBrainsMono-Bold.ttf",
    ]
    .into_iter()
    .filter_map(|name| std::fs::read(font_dir.join(name)).ok())
    .map(Cow::Owned)
    .collect::<Vec<Cow<'static, [u8]>>>();

    if !fonts.is_empty() {
        let _ = cx.text_system().add_fonts(fonts);
    }
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-o", OpenFile, Some("App")),
        KeyBinding::new("cmd-shift-o", OpenFolder, Some("App")),
        KeyBinding::new("cmd-s", SaveFile, Some("App")),
        KeyBinding::new("cmd-shift-s", SaveFileAs, Some("App")),
        KeyBinding::new("cmd-n", NewFile, Some("App")),
        KeyBinding::new("cmd-b", ToggleSidebar, Some("App")),
        KeyBinding::new("cmd-,", ToggleSettings, Some("App")),
        KeyBinding::new("cmd-e", ToggleExport, Some("App")),
        KeyBinding::new("cmd-p", ToggleQuickOpen, Some("App")),
        KeyBinding::new("cmd-shift-f", ToggleGlobalSearch, Some("App")),
        KeyBinding::new("cmd-1", ModeSource, Some("App")),
        KeyBinding::new("cmd-2", ModePreview, Some("App")),
        KeyBinding::new("cmd-3", ModeSplit, Some("App")),
        KeyBinding::new("cmd-shift-l", ToggleFocusMode, Some("App")),
        KeyBinding::new("cmd-shift-t", ToggleTypewriterMode, Some("App")),
        KeyBinding::new("escape", ClosePanel, Some("App")),
    ]);
}

fn initial_open_paths() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|path| is_markdown_path(path))
        .collect()
}

fn path_from_open_url(url: String) -> Option<PathBuf> {
    let path = if let Some(path) = url.strip_prefix("file://") {
        path.strip_prefix("localhost").unwrap_or(path)
    } else {
        url.as_str()
    };
    let path = percent_decode(path);
    let path = PathBuf::from(path);
    is_markdown_path(&path).then_some(path)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
            continue;
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .map(|extension| extension.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}
