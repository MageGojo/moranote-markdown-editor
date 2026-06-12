//! 平台相关的底层适配。
//!
//! 目前只在 macOS 上做一件事：消除 WKWebView 预览区「拖选退化成整行」的
//! 根因——WebKit 的文本拖拽消歧延迟。

#[cfg(target_os = "macos")]
mod macos;

/// 在应用启动早期、任何 WKWebView 创建之前调用。
///
/// 在非 macOS 平台上是空操作。
pub fn prepare_native_environment() {
    #[cfg(target_os = "macos")]
    macos::tune_webkit_text_selection();
}
