//! macOS 专属：从根上修复 WKWebView 预览区「拖选退化成整行」。
//!
//! # 根因
//! macOS 的 AppKit / WebKit 有一个系统级的「文本拖拽消歧延迟」，由全局
//! `NSUserDefaults` 键 `NSDragAndDropTextDelay` 控制（单位毫秒，默认约 150ms）。
//! 在这个延迟窗口内，WebKit 的 `EventHandler::handleDrag` 会**丢弃前若干个
//! `mouseDragged` 事件**，用来判断用户是想「拖动已选中的文字」还是「按住拖动
//! 来扩展选区」。
//!
//! 当 WKWebView 通过 `build_as_child` 被塞进 gpui 自绘的 NSView 层级里时，
//! 这套消歧逻辑工作异常：被丢弃的拖动事件连 DOM 都收不到，拖选最终退化成
//! 整行 / 整段。这也是为什么单纯改 CSS、调 `user-select`、甚至在 JS 里监听
//! `pointermove` 都救不回来——事件在 WebKit 引擎层就被吞掉了。
//!
//! # 方案
//! 在应用进程内、任何 WKWebView 创建之前，把 `NSDragAndDropTextDelay` 设为
//! `0`。这样 WebKit 不再有消歧延迟，第一下拖动就被当成扩展选区，得到精确的
//! 逐字符选择。
//!
//! 该键写入的是**本应用自己的** preferences domain（`com.<bundle>.moranote`），
//! 不会污染系统全局，也不影响其他应用。

use objc2_foundation::{NSUserDefaults, ns_string};

/// 在 WKWebView 创建之前调用一次，消除文本拖拽消歧延迟。
///
/// 直接写入 `0`：`integerForKey` 在键缺失时同样返回 `0`，无法区分「未设置」与
/// 「显式为 0」，所以这里不做提前返回，无条件落定目标值最稳妥。
pub fn tune_webkit_text_selection() {
    let defaults = NSUserDefaults::standardUserDefaults();
    defaults.setInteger_forKey(0, ns_string!("NSDragAndDropTextDelay"));
}
