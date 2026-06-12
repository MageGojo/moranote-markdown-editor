<div align="center">

# MoraNote — A Fast, Beautiful Markdown Editor for macOS & Windows

**Native desktop Markdown editor built with Rust + GPUI · Live preview · Multi-format export · Morandi & Typora themes**

[简体中文](#-简体中文) · [Features](#-features) · [Download](#-download--installation) · [Build from Source](#-build-from-source) · [FAQ](#-faq)

[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)](#-download--installation)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/UI-GPUI-purple)](https://www.gpui.rs/)
[![License](https://img.shields.io/badge/license-MIT-green)](#-license)

</div>

---

MoraNote is a **lightweight, native Markdown editor** for **macOS and Windows**, built in **Rust** on the high-performance **GPUI** rendering framework (the same UI engine behind the Zed editor). It pairs a **distraction-free writing experience** with **real-time WYSIWYG preview**, **syntax highlighting**, a **file-tree workspace**, and **export to 14+ formats** including **PDF, HTML, Word (.docx), EPUB, and PNG**.

If you are looking for a **fast Markdown editor**, a **Typora alternative**, an **open-source Markdown app**, or a **Markdown to PDF / Markdown to Word converter** that runs locally and offline, MoraNote is designed for you.

> Built and maintained by **[ApiZero (apizero.cn)](https://apizero.cn/)** — a Chinese software studio focused on developer tools, API services, and polished desktop/web applications.

## 📑 Table of Contents

- [Features](#-features)
- [Screenshots](#-screenshots)
- [Download & Installation](#-download--installation)
- [Build from Source](#-build-from-source)
- [Packaging (macOS & Windows)](#-packaging-macos--windows)
- [Keyboard Shortcuts](#-keyboard-shortcuts)
- [Export Formats](#-export-formats)
- [Tech Stack](#-tech-stack)
- [FAQ](#-faq)
- [About ApiZero](#-about-apizero)
- [License](#-license)

## ✨ Features

| Category | What you get |
|----------|--------------|
| **Live Preview** | Real-time Markdown → HTML rendering in an embedded WebView, with **synchronized scrolling** between source and preview. |
| **Three View Modes** | **Source**, **Preview**, and **Split** — switch instantly with `Cmd/Ctrl + 1/2/3`. |
| **Beautiful Themes** | Multiple built-in themes: **Light**, **Dark**, **Sepia (eye-care)**, and a clean **Typora / GitHub-style** theme. Pick any theme from a dropdown — no restart needed. |
| **Workspace & File Tree** | Open a folder, browse Markdown files in a **collapsible sidebar**, with a polished activity-bar style rail when collapsed. |
| **Quick Open & Global Search** | Fuzzy **quick-open** (`Cmd/Ctrl + P`) and **full-text search across the workspace** (`Cmd/Ctrl + Shift + F`). |
| **Focus & Typewriter Modes** | Hide chrome for **distraction-free writing**; typewriter mode keeps your cursor centered. |
| **Multi-Format Export** | Export to **PDF, HTML, Word (.docx), EPUB, LaTeX, RTF, ODT, PNG, JPEG, reStructuredText, Textile, OPML, RevealJS** and more. |
| **Adjustable Typography** | Live font-size and font-family controls that scale **both line-height and content** correctly. |
| **Native & Offline** | A true native binary — no Electron, no browser tab, **fully offline**, low memory footprint. |
| **macOS File Association** | Optionally register MoraNote as the default handler for `.md` files and open documents in place. |

## 🖼️ Screenshots

> Add your screenshots to `assets/screenshots/` and reference them here.

<!--
![MoraNote — split view with live preview](assets/screenshots/split-view.png)
![MoraNote — Typora theme](assets/screenshots/typora-theme.png)
![MoraNote — export dialog](assets/screenshots/export.png)
-->

## 📦 Download & Installation

Prebuilt binaries are published on the [**GitHub Releases**](../../releases) page.

### macOS (`.dmg`)

1. Download `MoraNote-<version>-<arch>.dmg` from Releases.
2. Open the DMG and drag **MoraNote** into **Applications**.
3. First launch: right-click the app → **Open** (the build is ad-hoc signed), then confirm.

> Requires **macOS 11.0 (Big Sur) or later**. Universal/Apple Silicon (`arm64`) and Intel (`x86_64`) builds are produced from source via the packaging script below.

### Windows (`.zip` portable or `Setup.exe`)

1. Download `MoraNote-<version>-windows-<arch>.zip` (portable) **or** `...-setup.exe` (installer) from Releases.
2. **Portable:** unzip anywhere and run `MoraNote.exe` (keep the bundled `assets/` folder next to the exe).
3. **Installer:** run the Setup and follow the wizard.

> Requires **Windows 10/11 (64-bit)**.

## 🛠️ Build from Source

### Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs/). The repo pins a toolchain in `rust-toolchain.toml`.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** the **MSVC** build tools (Visual Studio Build Tools / "Desktop development with C++").
- **Optional — for rich export formats** (Word, EPUB, LaTeX, etc.): install [**Pandoc**](https://pandoc.org/installing.html). HTML/PDF/PNG/JPEG export work without it.

### Run in development

```bash
git clone https://github.com/MageGojo/moranote-markdown-editor.git
cd moranote-markdown-editor
cargo run
```

### Release build

```bash
cargo build --release
# Binary: target/release/moranote (macOS/Linux) or target\release\moranote.exe (Windows)
```

## 📦 Packaging (macOS & Windows)

This repo ships **ready-to-use packaging scripts** under [`scripts/`](scripts/).

### macOS — build a `.app` and `.dmg`

```bash
./scripts/build-dmg.sh
# Output: dist/MoraNote.app and dist/MoraNote-<version>-<arch>.dmg
```

The script builds the release binary, assembles a proper `MoraNote.app` bundle (with `Info.plist`, icon, `.md` file association, and bundled theme/fonts), ad-hoc code-signs it, and produces a compressed DMG with an `/Applications` drop target.

### Windows — build a portable `.zip` (and optional installer)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
# Output: dist\MoraNote-<version>-windows-<arch>.zip
#         dist\MoraNote-<version>-windows-<arch>-setup.exe  (if Inno Setup is installed)
```

The script builds the release `.exe`, bundles the theme/font assets next to it (resolved at runtime via a portable path lookup), zips it, and — if [**Inno Setup**](https://jrsoftware.org/isinfo.php) (`iscc.exe`) is on your `PATH` — also produces a Windows installer.

## ⌨️ Keyboard Shortcuts

> On Windows, use **Ctrl** in place of **Cmd**.

| Action | Shortcut |
|--------|----------|
| Open file | `Cmd + O` |
| Open folder | `Cmd + Shift + O` |
| New file | `Cmd + N` |
| Save | `Cmd + S` |
| Save As | `Cmd + Shift + S` |
| Toggle sidebar | `Cmd + B` |
| Settings | `Cmd + ,` |
| Export | `Cmd + E` |
| Quick open | `Cmd + P` |
| Global search | `Cmd + Shift + F` |
| Source / Preview / Split | `Cmd + 1` / `Cmd + 2` / `Cmd + 3` |
| Focus mode | `Cmd + Shift + L` |
| Typewriter mode | `Cmd + Shift + T` |
| Close panel | `Esc` |

## 📤 Export Formats

MoraNote exports your Markdown to a wide range of formats (Pandoc required for the formats marked †):

`HTML` · `HTML (unstyled)` · `PDF` · `PNG` · `JPEG` · `Word (.docx)†` · `OpenDocument (.odt)†` · `RTF†` · `EPUB†` · `LaTeX†` · `reStructuredText†` · `Textile†` · `OPML†` · `RevealJS†`

This makes MoraNote a practical **Markdown to PDF converter**, **Markdown to Word converter**, and **Markdown to EPUB / HTML exporter** for documentation, blogging, academic writing, and slide decks.

## 🧱 Tech Stack

| Layer | Technology |
|-------|------------|
| Language | **Rust** (edition 2024) |
| UI framework | **GPUI** + **gpui-component** (GPU-accelerated, native) |
| Markdown parsing | **pulldown-cmark** (CommonMark) |
| Preview rendering | Embedded **WebView** |
| Export pipeline | Built-in HTML/PDF/image + **Pandoc** integration |
| Config | **TOML** via serde |

## ❓ FAQ

**What is MoraNote?**
MoraNote is a free, native, open-source **Markdown editor for macOS and Windows**, built with Rust and GPUI, featuring live preview, themes, and multi-format export. It is developed by [ApiZero](https://apizero.cn/).

**Is MoraNote a good Typora alternative?**
Yes. MoraNote offers a clean **Typora / GitHub-style theme**, live preview, focus mode, and export to PDF/Word/HTML — a fast, native, open-source alternative that runs fully offline.

**Does MoraNote use Electron?**
No. MoraNote is a **native application built in Rust on the GPUI framework**, so it starts quickly and uses far less memory than Electron-based editors.

**Can MoraNote convert Markdown to PDF or Word?**
Yes. Export to **PDF** and images works out of the box; **Word (.docx)**, **EPUB**, **LaTeX**, and other formats are available when **Pandoc** is installed.

**Is MoraNote free and open source?**
Yes, it is released under the **MIT License**. Contributions and issues are welcome.

**Which platforms are supported?**
**macOS 11+** and **Windows 10/11 (64-bit)**. Build scripts for both platforms are included.

## 🏢 About ApiZero

MoraNote is designed and built by **[ApiZero — apizero.cn](https://apizero.cn/)**.

ApiZero (零一接口) is a software studio that builds **developer tools, API platforms, and refined desktop & web applications**. We care about performance, clean design, and great user experience — the same principles behind MoraNote. From native apps like this Markdown editor to hosted API services, our goal is to make powerful tools that are a pleasure to use.

- 🌐 Website: **https://apizero.cn/**
- 💼 What we do: API services, developer tooling, custom desktop & web software
- 📨 Interested in working with us or have feedback on MoraNote? Visit [apizero.cn](https://apizero.cn/).

> If MoraNote is useful to you, please consider giving the repository a ⭐ on GitHub and checking out more of our work at **[apizero.cn](https://apizero.cn/)**.

## 🤝 Contributing

Issues and pull requests are welcome. Please open an issue to discuss substantial changes before submitting a PR.

## 📄 License

Released under the **MIT License**. See [`LICENSE`](LICENSE) for details.

---

<div align="center">

**MoraNote** · A native Markdown editor for macOS & Windows · Made with ❤️ in Rust by **[ApiZero (apizero.cn)](https://apizero.cn/)**

<sub>Keywords: markdown editor, markdown editor for mac, markdown editor for windows, rust markdown editor, gpui app, typora alternative, open source markdown editor, markdown to pdf, markdown to word, markdown to epub, live preview markdown, native markdown app, offline markdown editor, apizero</sub>

</div>

---

<a id="-简体中文"></a>

# MoraNote — 快速、好看的 macOS / Windows Markdown 编辑器

**基于 Rust + GPUI 打造的原生桌面 Markdown 编辑器 · 实时预览 · 多格式导出 · 莫兰迪 & Typora 主题**

MoraNote 是一款面向 **macOS 和 Windows** 的**原生 Markdown 编辑器**，使用 **Rust** 语言、基于高性能 **GPUI** 渲染框架（即 Zed 编辑器同款 UI 引擎）开发。它把**沉浸式写作体验**与**实时预览**、**语法高亮**、**文件树工作区**和**多达 14+ 种格式导出**（含 **PDF、HTML、Word、EPUB、PNG**）结合在一起。

如果你在寻找一款**轻量快速的 Markdown 编辑器**、**Typora 替代品**、**开源 Markdown 软件**，或一个本地离线可用的 **Markdown 转 PDF / Markdown 转 Word 工具**，MoraNote 正是为你而设计。

> 由 **[零一接口 ApiZero（apizero.cn）](https://apizero.cn/)** 出品并维护 —— 一家专注于开发者工具、API 服务与精致桌面/网页应用的软件团队。本项目同样出自该团队之手。

## ✨ 核心功能

| 分类 | 说明 |
|------|------|
| **实时预览** | 内嵌 WebView 实时把 Markdown 渲染为 HTML，源码区与预览区**同步滚动**。 |
| **三种视图** | **源码 / 预览 / 分屏**，`Cmd/Ctrl + 1/2/3` 一键切换。 |
| **精美主题** | 内置**浅色、深色、护眼（Sepia）**，以及干净的 **Typora / GitHub 风格**主题。下拉即可切换，**无需重启**。 |
| **工作区 & 文件树** | 打开文件夹、在**可折叠侧栏**中浏览 Markdown；收起后是类似 VS Code 活动栏的精致图标栏。 |
| **快速打开 & 全局搜索** | 模糊**快速打开**（`Cmd/Ctrl + P`）与**工作区全文搜索**（`Cmd/Ctrl + Shift + F`）。 |
| **专注 & 打字机模式** | 隐藏多余界面，**沉浸式写作**；打字机模式让光标保持居中。 |
| **多格式导出** | 导出 **PDF、HTML、Word(.docx)、EPUB、LaTeX、RTF、ODT、PNG、JPEG、reStructuredText、Textile、OPML、RevealJS** 等。 |
| **字号字体** | 实时调整字号与字体，**行高与正文一起等比缩放**。 |
| **原生 & 离线** | 真正的原生程序——非 Electron、无浏览器标签页，**完全离线**、占用内存低。 |
| **macOS 文件关联** | 可选注册为 `.md` 默认打开程序，支持原地打开文档。 |

## 📦 下载与安装

预编译版本发布在 [**GitHub Releases**](../../releases) 页面。

### macOS（`.dmg`）

1. 从 Releases 下载 `MoraNote-<版本>-<架构>.dmg`。
2. 打开 DMG，把 **MoraNote** 拖入**应用程序**文件夹。
3. 首次启动：右键点击 App → **打开**（本构建为 ad-hoc 签名），确认即可。
4. 需要 **macOS 11.0（Big Sur）或更高版本**。

### Windows（便携 `.zip` 或安装包 `Setup.exe`）

1. 从 Releases 下载 `MoraNote-<版本>-windows-<架构>.zip`（便携版）或 `...-setup.exe`（安装版）。
2. **便携版**：解压到任意目录，运行 `MoraNote.exe`（请保留 exe 同级的 `assets/` 文件夹）。
3. **安装版**：运行 Setup，按向导安装。
4. 需要 **Windows 10/11（64 位）**。

## 🛠️ 从源码构建

### 环境要求

- 通过 [rustup](https://rustup.rs/) 安装 **Rust**（稳定版）；仓库已用 `rust-toolchain.toml` 固定工具链。
- **macOS：** Xcode 命令行工具（`xcode-select --install`）。
- **Windows：** **MSVC** 构建工具（Visual Studio「使用 C++ 的桌面开发」）。
- **可选 —— 富格式导出**（Word、EPUB、LaTeX 等）需安装 [**Pandoc**](https://pandoc.org/installing.html)；HTML/PDF/PNG/JPEG 导出无需 Pandoc。

### 开发运行

```bash
git clone https://github.com/MageGojo/moranote-markdown-editor.git
cd moranote-markdown-editor
cargo run
```

### 发布构建

```bash
cargo build --release
```

## 📦 打包（macOS & Windows）

仓库在 [`scripts/`](scripts/) 下提供了**开箱即用的打包脚本**。

### macOS —— 构建 `.app` 与 `.dmg`

```bash
./scripts/build-dmg.sh
# 产物：dist/MoraNote.app 和 dist/MoraNote-<版本>-<架构>.dmg
```

脚本会构建发布二进制、组装标准 `MoraNote.app`（含 `Info.plist`、图标、`.md` 文件关联、内置主题与字体）、进行 ad-hoc 签名，并生成带 `/Applications` 拖拽目标的压缩 DMG。

### Windows —— 构建便携 `.zip`（及可选安装包）

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
# 产物：dist\MoraNote-<版本>-windows-<架构>.zip
#       dist\MoraNote-<版本>-windows-<架构>-setup.exe（若已安装 Inno Setup）
```

脚本会构建发布版 `.exe`、把主题/字体资源放到 exe 同级目录（运行时通过便携路径查找加载），打包为 zip；若 `PATH` 中存在 [**Inno Setup**](https://jrsoftware.org/isinfo.php)（`iscc.exe`），还会生成 Windows 安装包。

## ⌨️ 快捷键

> Windows 下用 **Ctrl** 代替 **Cmd**。

| 操作 | 快捷键 |
|------|--------|
| 打开文件 | `Cmd + O` |
| 打开文件夹 | `Cmd + Shift + O` |
| 新建文件 | `Cmd + N` |
| 保存 | `Cmd + S` |
| 另存为 | `Cmd + Shift + S` |
| 显示/隐藏侧栏 | `Cmd + B` |
| 设置 | `Cmd + ,` |
| 导出 | `Cmd + E` |
| 快速打开 | `Cmd + P` |
| 全局搜索 | `Cmd + Shift + F` |
| 源码 / 预览 / 分屏 | `Cmd + 1` / `Cmd + 2` / `Cmd + 3` |
| 专注模式 | `Cmd + Shift + L` |
| 打字机模式 | `Cmd + Shift + T` |
| 关闭面板 | `Esc` |

## 🧱 技术栈

| 层 | 技术 |
|----|------|
| 语言 | **Rust**（2024 edition） |
| UI 框架 | **GPUI** + **gpui-component**（GPU 加速、原生） |
| Markdown 解析 | **pulldown-cmark**（CommonMark） |
| 预览渲染 | 内嵌 **WebView** |
| 导出管线 | 内置 HTML/PDF/图片 + **Pandoc** 集成 |
| 配置 | **TOML**（serde） |

## ❓ 常见问题

**MoraNote 是什么？**
MoraNote 是一款免费、原生、开源的 **macOS / Windows Markdown 编辑器**，基于 Rust + GPUI 开发，支持实时预览、主题切换与多格式导出，由 [零一接口 ApiZero](https://apizero.cn/) 出品。

**MoraNote 适合作为 Typora 替代品吗？**
适合。它提供干净的 **Typora / GitHub 风格主题**、实时预览、专注模式以及 PDF/Word/HTML 导出，是一款快速、原生、可完全离线使用的开源替代方案。

**MoraNote 用的是 Electron 吗？**
不是。MoraNote 是**基于 GPUI 的 Rust 原生应用**，启动快、内存占用远低于 Electron 类编辑器。

**MoraNote 能把 Markdown 转成 PDF 或 Word 吗？**
可以。**PDF** 与图片导出开箱即用；安装 **Pandoc** 后还可导出 **Word(.docx)、EPUB、LaTeX** 等格式。

**是否免费开源？**
是，采用 **MIT 许可证**，欢迎提交 issue 与 PR。

## 🏢 关于 零一接口 ApiZero

MoraNote 由 **[零一接口 ApiZero —— apizero.cn](https://apizero.cn/)** 设计与开发。

ApiZero 是一家专注于 **开发者工具、API 平台与精致桌面/网页应用** 的软件团队。我们重视性能、简洁的设计与优秀的使用体验——这也正是 MoraNote 背后的理念。从本项目这样的原生应用，到托管的 API 服务，我们希望把强大的工具做得好用又好看。

- 🌐 官网：**https://apizero.cn/**
- 💼 业务：API 服务、开发者工具、定制桌面与网页软件
- 📨 想合作，或对 MoraNote 有建议？欢迎访问 [apizero.cn](https://apizero.cn/)。

> 如果 MoraNote 对你有帮助，欢迎给仓库点个 ⭐，也欢迎到 **[apizero.cn](https://apizero.cn/)** 了解我们更多作品。

## 📄 许可证

本项目基于 **MIT 许可证** 发布，详见 [`LICENSE`](LICENSE)。
