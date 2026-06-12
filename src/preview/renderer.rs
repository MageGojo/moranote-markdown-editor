use crate::model::{DocumentStats, OutlineItem};
use gpui_component::wry::http::{Response, StatusCode};
use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

const MORANDI_CSS: &str = include_str!("../../assets/themes/morandigarden/morandigarden.css");
const THEME_URL_ROOT: &str = "mdres://localhost/__theme/morandigarden/";
const THEME_PATH_PREFIX: &str = "__theme/morandigarden/";
pub const MORANDI_EXPORT_FONT_DIR: &str = "morandigarden";

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub markdown: String,
    pub base_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedDocument {
    pub html: String,
    pub outline: Vec<OutlineItem>,
    pub stats: DocumentStats,
    pub front_matter: BTreeMap<String, String>,
}

pub type SharedResourceRoot = Arc<Mutex<Option<PathBuf>>>;

pub fn morandi_theme_asset_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        // macOS：.app/Contents/Resources/assets/themes/morandigarden
        if let Some(contents) = exe
            .parent()
            .and_then(|macos_dir| macos_dir.parent())
            .filter(|contents_dir| {
                contents_dir
                    .file_name()
                    .is_some_and(|name| name == "Contents")
            })
        {
            let resources = contents
                .join("Resources")
                .join("assets")
                .join("themes")
                .join("morandigarden");
            if resources.exists() {
                return resources;
            }
        }

        // 便携式布局（Windows / Linux）：可执行文件同级目录下的 assets/themes/morandigarden
        if let Some(exe_dir) = exe.parent() {
            let portable = exe_dir
                .join("assets")
                .join("themes")
                .join("morandigarden");
            if portable.exists() {
                return portable;
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("themes")
        .join("morandigarden")
}

pub fn copy_morandi_font_assets(destination_dir: &Path) -> std::io::Result<()> {
    let source = morandi_theme_asset_dir().join("morandigarden");
    let destination = destination_dir.join(MORANDI_EXPORT_FONT_DIR);
    fs::create_dir_all(&destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            fs::copy(&path, destination.join(entry.file_name()))?;
        }
    }

    Ok(())
}

pub fn html_for_export(html: &str) -> String {
    html_for_export_with_base(html, None)
}

pub fn html_for_export_with_base(html: &str, base_dir: Option<&Path>) -> String {
    let exported = html
        .replace(
            &format!("{THEME_URL_ROOT}morandigarden/"),
            &format!("{MORANDI_EXPORT_FONT_DIR}/"),
        )
        .replace("<base href=\"mdres://localhost/\">\n", "");

    if let Some(base_dir) = base_dir {
        rewrite_relative_resource_urls(&exported, base_dir)
    } else {
        exported
    }
}

pub fn html_for_export_with_asset_package(
    html: &str,
    base_dir: Option<&Path>,
    asset_url_prefix: &str,
    asset_dir: &Path,
) -> std::io::Result<String> {
    let exported = html
        .replace(
            &format!("{THEME_URL_ROOT}morandigarden/"),
            &format!("{asset_url_prefix}/{MORANDI_EXPORT_FONT_DIR}/"),
        )
        .replace("<base href=\"mdres://localhost/\">\n", "");

    if let Some(base_dir) = base_dir {
        rewrite_packaged_resource_urls(&exported, base_dir, asset_url_prefix, asset_dir)
    } else {
        Ok(exported)
    }
}

pub fn render_markdown(request: RenderRequest) -> RenderedDocument {
    let (front_matter, body) = split_front_matter(&request.markdown);
    let outline = extract_outline(body);
    let stats = compute_stats(body);
    let mut html_body = markdown_to_html(body);
    html_body = inject_heading_ids(&html_body, &outline);

    RenderedDocument {
        html: wrap_document(&html_body, request.base_dir.as_deref()),
        outline,
        stats,
        front_matter,
    }
}

pub fn preview_shell() -> String {
    let empty = wrap_document("<p class=\"empty\">Preview is ready.</p>", None);
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN" data-theme="light">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<base href="mdres://localhost/">
<style>{}</style>
</head>
<body>
<main id="content" class="markdown-body">{}</main>
<script>
{}
</script>
</body>
</html>"#,
        preview_css(),
        extract_body(&empty),
        preview_runtime_script()
    )
}

pub fn update_script(rendered: &RenderedDocument) -> String {
    let payload = serde_json::json!({
        "html": extract_body(&rendered.html),
        "outline": rendered.outline,
        "stats": rendered.stats,
        "frontMatter": rendered.front_matter,
    });
    format!("window.__setRendered({});", payload)
}

pub fn scroll_script(anchor: &str) -> String {
    let anchor = serde_json::to_string(anchor).unwrap_or_else(|_| "\"\"".to_string());
    format!("window.__scrollToAnchor({anchor});")
}

pub fn resource_response(
    resource_root: &SharedResourceRoot,
    uri_path: &str,
) -> Response<Cow<'static, [u8]>> {
    let path = uri_path.trim_start_matches('/');
    if let Some(candidate) = theme_asset_candidate(path) {
        return file_response(candidate);
    }

    let root = resource_root.lock().ok().and_then(|root| root.clone());
    let Some(root) = root else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"resource root not set".to_vec(),
        );
    };

    let candidate = root.join(path);
    let Ok(root) = root.canonicalize() else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"resource root missing".to_vec(),
        );
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"resource missing".to_vec(),
        );
    };

    if !candidate.starts_with(&root) || !candidate.is_file() {
        return response(
            StatusCode::FORBIDDEN,
            "text/plain",
            b"resource denied".to_vec(),
        );
    }

    match std::fs::read(&candidate) {
        Ok(bytes) => response(StatusCode::OK, mime_for_path(&candidate), bytes),
        Err(_) => response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"resource read failed".to_vec(),
        ),
    }
}

fn theme_asset_candidate(uri_path: &str) -> Option<PathBuf> {
    let relative = uri_path.strip_prefix(THEME_PATH_PREFIX)?;
    let root = morandi_theme_asset_dir();
    let candidate = root.join(relative);
    let root = root.canonicalize().ok()?;
    let candidate = candidate.canonicalize().ok()?;
    if candidate.starts_with(&root) && candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn file_response(path: PathBuf) -> Response<Cow<'static, [u8]>> {
    match std::fs::read(&path) {
        Ok(bytes) => response(StatusCode::OK, mime_for_path(&path), bytes),
        Err(_) => response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"theme resource missing".to_vec(),
        ),
    }
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn wrap_document(body: &str, _base_dir: Option<&Path>) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN" data-theme="light">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<base href="mdres://localhost/">
<style>{}</style>
</head>
<body>
<main id="content" class="markdown-body">{}</main>
<script>
{}
</script>
</body>
</html>"#,
        preview_css(),
        body,
        preview_runtime_script()
    )
}

fn extract_body(document: &str) -> String {
    let Some(start) = document.find("<main id=\"content\" class=\"markdown-body\">") else {
        return document.to_string();
    };
    let body_start = start + "<main id=\"content\" class=\"markdown-body\">".len();
    let Some(end) = document[body_start..].find("</main>") else {
        return document[body_start..].to_string();
    };
    document[body_start..body_start + end].to_string()
}

pub fn file_url_for_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }

    let mut url = String::from("file://");
    for byte in normalized.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                url.push(byte as char)
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }

    if path.is_dir() && !url.ends_with('/') {
        url.push('/');
    }

    url
}

fn rewrite_relative_resource_urls(html: &str, base_dir: &Path) -> String {
    let html = rewrite_attribute_urls(html, "src=\"", base_dir);
    rewrite_attribute_urls(&html, "href=\"", base_dir)
}

fn rewrite_packaged_resource_urls(
    html: &str,
    base_dir: &Path,
    asset_url_prefix: &str,
    asset_dir: &Path,
) -> std::io::Result<String> {
    let html =
        rewrite_packaged_attribute_urls(html, "src=\"", base_dir, asset_url_prefix, asset_dir)?;
    rewrite_packaged_attribute_urls(&html, "href=\"", base_dir, asset_url_prefix, asset_dir)
}

fn rewrite_packaged_attribute_urls(
    html: &str,
    marker: &str,
    base_dir: &Path,
    asset_url_prefix: &str,
    asset_dir: &Path,
) -> std::io::Result<String> {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(start) = rest.find(marker) {
        output.push_str(&rest[..start + marker.len()]);
        rest = &rest[start + marker.len()..];

        let Some(end) = rest.find('"') else {
            output.push_str(rest);
            return Ok(output);
        };

        let value = &rest[..end];
        if let Some(rewritten) =
            packaged_url_for_export_attribute(value, base_dir, asset_url_prefix, asset_dir)?
        {
            output.push_str(&rewritten);
        } else {
            output.push_str(value);
        }
        rest = &rest[end..];
    }

    output.push_str(rest);
    Ok(output)
}

fn packaged_url_for_export_attribute(
    value: &str,
    base_dir: &Path,
    asset_url_prefix: &str,
    asset_dir: &Path,
) -> std::io::Result<Option<String>> {
    let Some((path, suffix, original_path_part)) = local_resource_parts(value, base_dir) else {
        return Ok(None);
    };

    if !path.is_file() || !should_package_resource(&path) {
        return Ok(Some(format!("{}{}", file_url_for_path(&path), suffix)));
    }

    let package_path = package_relative_path(&path, base_dir, &original_path_part);
    let target = asset_dir.join(&package_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&path, &target)?;

    Ok(Some(format!(
        "{}/{}{}",
        asset_url_prefix,
        relative_url_path(&package_path),
        suffix
    )))
}

fn rewrite_attribute_urls(html: &str, marker: &str, base_dir: &Path) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(start) = rest.find(marker) {
        output.push_str(&rest[..start + marker.len()]);
        rest = &rest[start + marker.len()..];

        let Some(end) = rest.find('"') else {
            output.push_str(rest);
            return output;
        };

        let value = &rest[..end];
        if let Some(rewritten) = file_url_for_export_attribute(value, base_dir) {
            output.push_str(&rewritten);
        } else {
            output.push_str(value);
        }
        rest = &rest[end..];
    }

    output.push_str(rest);
    output
}

fn file_url_for_export_attribute(value: &str, base_dir: &Path) -> Option<String> {
    let (path, suffix, _) = local_resource_parts(value, base_dir)?;
    Some(format!("{}{}", file_url_for_path(&path), suffix))
}

fn local_resource_parts(value: &str, base_dir: &Path) -> Option<(PathBuf, String, String)> {
    if value.is_empty() || value.starts_with('#') || has_url_scheme(value) {
        return None;
    }

    let split = value
        .find(['?', '#'])
        .map(|index| (&value[..index], &value[index..]))
        .unwrap_or((value, ""));
    let (path_part, suffix) = split;
    if path_part.is_empty() {
        return None;
    }

    let path_part = percent_decode(path_part);
    let path = if Path::new(&path_part).is_absolute() {
        PathBuf::from(&path_part)
    } else {
        base_dir.join(&path_part)
    };

    Some((path, suffix.to_string(), path_part))
}

fn should_package_resource(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "apng"
                    | "avif"
                    | "bmp"
                    | "gif"
                    | "ico"
                    | "jpeg"
                    | "jpg"
                    | "png"
                    | "svg"
                    | "webp"
                    | "pdf"
                    | "mp3"
                    | "mp4"
                    | "ogg"
                    | "wav"
                    | "webm"
            )
        })
        .unwrap_or(false)
}

fn package_relative_path(path: &Path, base_dir: &Path, original_path_part: &str) -> PathBuf {
    if !Path::new(original_path_part).is_absolute() {
        let relative = Path::new(original_path_part);
        let sanitized = sanitize_relative_path(relative);
        if !sanitized.as_os_str().is_empty() {
            return sanitized;
        }
    }

    let canonical_base = base_dir.canonicalize().ok();
    let canonical_path = path.canonicalize().ok();
    if let (Some(base), Some(path)) = (canonical_base, canonical_path) {
        if let Ok(relative) = path.strip_prefix(base) {
            let sanitized = sanitize_relative_path(relative);
            if !sanitized.as_os_str().is_empty() {
                return sanitized;
            }
        }
    }

    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("resource"))
}

fn sanitize_relative_path(path: &Path) -> PathBuf {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        if let Component::Normal(part) = component {
            sanitized.push(part);
        }
    }
    sanitized
}

fn relative_url_path(path: &Path) -> String {
    let mut output = String::new();
    for (index, component) in path.components().enumerate() {
        if let Component::Normal(part) = component {
            if index > 0 {
                output.push('/');
            }
            output.push_str(&percent_encode_path_part(&part.to_string_lossy()));
        }
    }
    output
}

fn percent_encode_path_part(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn has_url_scheme(value: &str) -> bool {
    let Some(colon) = value.find(':') else {
        return false;
    };
    value[..colon]
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn split_front_matter(markdown: &str) -> (BTreeMap<String, String>, &str) {
    let mut front_matter = BTreeMap::new();
    if !markdown.starts_with("---\n") {
        return (front_matter, markdown);
    }

    let Some(end) = markdown[4..].find("\n---") else {
        return (front_matter, markdown);
    };

    let raw = &markdown[4..4 + end];
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(':') {
            front_matter.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let body_start = 4 + end + "\n---".len();
    let body = markdown[body_start..]
        .strip_prefix('\n')
        .unwrap_or(&markdown[body_start..]);
    (front_matter, body)
}

fn extract_outline(markdown: &str) -> Vec<OutlineItem> {
    markdown
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
            if hashes == 0 || hashes > 6 {
                return None;
            }
            let rest = trimmed[hashes..].trim_start();
            if rest.is_empty() {
                return None;
            }
            let title = rest.trim_end_matches('#').trim().to_string();
            if title.is_empty() {
                return None;
            }
            Some(OutlineItem {
                level: hashes as u8,
                anchor: slugify(&title, index),
                title,
                line: index + 1,
            })
        })
        .collect()
}

fn inject_heading_ids(html: &str, outline: &[OutlineItem]) -> String {
    let mut output = html.to_string();
    let mut search_from = 0;
    for item in outline {
        let needle = format!("<h{}>", item.level);
        let Some(relative) = output[search_from..].find(&needle) else {
            continue;
        };
        let index = search_from + relative;
        let replacement = format!("<h{} id=\"{}\">", item.level, escape_attr(&item.anchor));
        output.replace_range(index..index + needle.len(), &replacement);
        search_from = index + replacement.len();
    }
    output
}

fn compute_stats(markdown: &str) -> DocumentStats {
    let words = markdown
        .split_whitespace()
        .filter(|word| {
            !word
                .trim_matches(|ch: char| ch.is_ascii_punctuation())
                .is_empty()
        })
        .count();
    let chars = markdown.chars().filter(|ch| !ch.is_whitespace()).count();
    let lines = markdown.lines().count().max(1);
    let reading_units = words.max(chars / 2);
    let reading_time_minutes = (reading_units / 240).max(1);

    DocumentStats {
        words,
        chars,
        lines,
        reading_time_minutes,
    }
}

fn slugify(title: &str, line: usize) -> String {
    let mut slug = title
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_whitespace() || matches!(ch, '-' | '_') {
                Some('-')
            } else if ch.is_alphanumeric() {
                Some(ch)
            } else {
                None
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if slug.is_empty() {
        slug = "section".to_string();
    }
    format!("{slug}-{}", line + 1)
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn response(
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .body(Cow::Owned(body))
        .unwrap_or_else(|_| Response::new(Cow::Owned(Vec::new())))
}

fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "css" => "text/css",
        "js" => "text/javascript",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn preview_css() -> String {
    let upstream = MORANDI_CSS
        .replace(
            "url('./morandigarden/",
            &format!("url('{THEME_URL_ROOT}morandigarden/"),
        )
        .replace(
            "url(\"./morandigarden/",
            &format!("url(\"{THEME_URL_ROOT}morandigarden/"),
        );

    format!("{upstream}\n{MORANDI_ADAPTER_CSS}")
}

const MORANDI_ADAPTER_CSS: &str = r#"
:root,
:root[data-theme="light"] {
  color-scheme: light;
  --bg-color: #FBFDFB;
  --text-color: #403C3C;
  --h1-color: #7A5F52;
  --h2-color: #506956;
  --h3-color: #7A6F89;
  --h4-color: #6F7F88;
  --h5-color: #8A7A6F;
  --h6-color: #7D7A71;
  --link-color: #506956;
  --link-hover-color: #405946;
  --code-bg-color: #F0F5F3;
  --code-text-color: #403C3C;
  --blockquote-bg-color: #EEF4EF;
  --blockquote-border-color: #7A5F52;
  --table-border-color: #7EA388;
  --th-bg-color: #EAF0EB;
  --paper-color: rgba(255, 255, 255, 0.72);
  --soft-border-color: rgba(126, 163, 136, 0.34);
  --shadow-color: rgba(64, 60, 60, 0.08);
}

:root[data-theme="sepia"] {
  color-scheme: light;
  --bg-color: #F5EFE5;
  --text-color: #423D38;
  --h1-color: #7A5F52;
  --h2-color: #596E58;
  --h3-color: #756A83;
  --h4-color: #6B7A7E;
  --h5-color: #8A7A6F;
  --h6-color: #7D7A71;
  --link-color: #596E58;
  --link-hover-color: #465A47;
  --code-bg-color: #EFE8DC;
  --code-text-color: #403C3C;
  --blockquote-bg-color: #EFE9DD;
  --blockquote-border-color: #8A6C5D;
  --table-border-color: #93A486;
  --th-bg-color: #E8E2D6;
  --paper-color: rgba(255, 250, 241, 0.76);
  --soft-border-color: rgba(138, 122, 111, 0.34);
  --shadow-color: rgba(75, 62, 51, 0.08);
}

:root[data-theme="dark"] {
  color-scheme: dark;
  --bg-color: #252823;
  --text-color: #E7E1D7;
  --h1-color: #D0B6A8;
  --h2-color: #A8B89E;
  --h3-color: #B5AFC8;
  --h4-color: #A7BBC1;
  --h5-color: #CAB4A4;
  --h6-color: #BEB8AA;
  --link-color: #A8B89E;
  --link-hover-color: #C2CFB9;
  --code-bg-color: #1F2926;
  --code-text-color: #E7E1D7;
  --blockquote-bg-color: #2D352D;
  --blockquote-border-color: #D0B6A8;
  --table-border-color: #657A6B;
  --th-bg-color: #303830;
  --paper-color: rgba(32, 35, 31, 0.78);
  --soft-border-color: rgba(168, 184, 158, 0.28);
  --shadow-color: rgba(0, 0, 0, 0.22);
}

/* Typora / GitHub 风格：纯白底、近黑正文、GitHub 蓝链接，干净无暖色。 */
:root[data-theme="typora"] {
  color-scheme: light;
  --bg-color: #FFFFFF;
  --text-color: #1F2328;
  --h1-color: #1F2328;
  --h2-color: #1F2328;
  --h3-color: #1F2328;
  --h4-color: #1F2328;
  --h5-color: #1F2328;
  --h6-color: #636C76;
  --link-color: #0969DA;
  --link-hover-color: #0550AE;
  --code-bg-color: #EFF1F3;
  --code-text-color: #1F2328;
  --blockquote-bg-color: #FFFFFF;
  --blockquote-border-color: #D0D7DE;
  --table-border-color: #D0D7DE;
  --th-bg-color: #F6F8FA;
  --paper-color: #FFFFFF;
  --soft-border-color: #D0D7DE;
  --shadow-color: rgba(31, 35, 40, 0.04);
}

* { box-sizing: border-box; }
html, body { min-height: 100%; }
body {
  margin: 0;
  overflow-y: auto;
  background:
    radial-gradient(circle at top left, rgba(126, 163, 136, 0.16), transparent 34rem),
    linear-gradient(135deg, var(--bg-color), color-mix(in srgb, var(--bg-color) 88%, var(--h2-color)));
  color: var(--text-color);
  font-family: var(--font-main);
  font-size: var(--font-size-base);
  line-height: var(--line-height);
}

/* Typora 主题：纯净背景、无暖色渐变、纸张采用细边框而非重投影。 */
:root[data-theme="typora"] body {
  background: var(--bg-color);
}
:root[data-theme="typora"] .markdown-body {
  border: 1px solid var(--soft-border-color);
  box-shadow: none;
}
:root[data-theme="typora"] .markdown-body h1 {
  border-bottom-color: var(--soft-border-color);
}
:root[data-theme="typora"] .markdown-body h2 {
  border-left: none;
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--soft-border-color);
}

/* 让预览正文像普通网页一样可以逐字选择文本，而不是整行整块选中 */
.markdown-body,
.markdown-body * {
  -webkit-user-select: text;
  user-select: text;
}

.markdown-body {
  cursor: text;
  -webkit-user-modify: read-only;
}

.markdown-body a,
.markdown-body button,
.markdown-body input,
.markdown-body summary {
  cursor: pointer;
}

.markdown-body ::selection {
  background: color-mix(in srgb, var(--h2-color) 32%, transparent);
}
.markdown-body ::-moz-selection {
  background: color-mix(in srgb, var(--h2-color) 32%, transparent);
}

.markdown-body {
  width: min(var(--write-width), calc(100vw - 4rem));
  min-height: 100vh;
  margin: 0 auto;
  padding: 3.5rem 4rem 6rem;
  background: var(--paper-color);
  color: var(--text-color);
  box-shadow: 0 24px 70px var(--shadow-color);
  font-family: var(--font-main);
  line-height: var(--line-height);
}

.markdown-body h1,
.markdown-body h2,
.markdown-body h3,
.markdown-body h4,
.markdown-body h5,
.markdown-body h6 {
  letter-spacing: 0;
  scroll-margin-top: 2rem;
}

.markdown-body h1 { color: var(--h1-color); border-bottom: 2px solid #DDE6DF; }
.markdown-body h2 { color: var(--h2-color); border-left: 4px solid var(--h2-color); border-bottom: none; }
.markdown-body h3 { color: var(--h3-color); }
.markdown-body h4 { color: var(--h4-color); }
.markdown-body h5 { color: var(--h5-color); }
.markdown-body h6 { color: var(--h6-color); }

.markdown-body p { margin: 0 0 1rem; }
.markdown-body a { color: var(--link-color); }
.markdown-body a:hover { color: var(--link-hover-color); }

.markdown-body p code,
.markdown-body li code,
.markdown-body td code {
  padding: 0.15rem 0.38rem;
  color: var(--h3-color);
  background: var(--code-bg-color);
  border-radius: 5px;
  font-family: var(--font-mono);
  font-size: 0.92em;
}

.markdown-body pre {
  position: relative;
  margin: 1.2rem 0;
  overflow: auto;
  padding: 1.1rem 1.2rem;
  color: var(--code-text-color);
  background: var(--code-bg-color);
  border: 1px solid var(--soft-border-color);
  border-radius: 12px;
}

.markdown-body pre code {
  padding: 0;
  color: inherit;
  background: transparent;
  font-family: var(--font-mono);
  line-height: 1.7;
}

.copy-code-button {
  position: absolute;
  top: 0.55rem;
  right: 0.55rem;
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  height: 1.85rem;
  padding: 0 0.6rem;
  color: var(--h2-color);
  background: color-mix(in srgb, var(--bg-color) 86%, white);
  border: 1px solid var(--soft-border-color);
  border-radius: 999px;
  box-shadow: 0 6px 18px var(--shadow-color);
  font-family: var(--font-main);
  font-size: 0.76rem;
  cursor: pointer;
  opacity: 0;
  transform: translateY(-2px);
  transition: opacity 160ms ease, transform 160ms ease, background 160ms ease;
}

.markdown-body pre:hover .copy-code-button,
.copy-code-button:focus-visible,
.copy-code-button.copied {
  opacity: 1;
  transform: translateY(0);
}

.copy-code-button:hover {
  background: var(--th-bg-color);
}

.copy-code-button.copied {
  color: var(--h1-color);
  border-color: color-mix(in srgb, var(--h1-color) 45%, transparent);
}

.copy-icon {
  position: relative;
  width: 0.78rem;
  height: 0.78rem;
  display: inline-block;
}

.copy-icon::before,
.copy-icon::after {
  content: "";
  position: absolute;
  width: 0.52rem;
  height: 0.62rem;
  border: 1.5px solid currentColor;
  border-radius: 0.16rem;
}

.copy-icon::before {
  left: 0;
  top: 0.16rem;
  opacity: 0.55;
}

.copy-icon::after {
  right: 0;
  top: 0;
  background: color-mix(in srgb, var(--code-bg-color) 90%, white);
}

@media print {
  .copy-code-button {
    display: none !important;
  }
}

.markdown-body blockquote {
  margin: 1.25rem 0;
  padding: 0.75rem 1rem 0.75rem 1.1rem;
  color: color-mix(in srgb, var(--text-color) 78%, var(--h1-color));
  background: var(--blockquote-bg-color);
  border-left: 4px solid var(--blockquote-border-color);
  border-radius: 0 10px 10px 0;
}

.markdown-body table {
  width: 100%;
  margin: 1.25rem 0;
  border-collapse: collapse;
}

.markdown-body th,
.markdown-body td {
  border: 1px solid var(--table-border-color);
  padding: 0.55rem 0.75rem;
}

.markdown-body th { background: var(--th-bg-color); }
.markdown-body img { max-width: 100%; height: auto; border-radius: 8px; }
.markdown-body hr {
  border: none;
  border-top: 1px solid var(--soft-border-color);
  margin: 2rem 0;
}

.markdown-body ul,
.markdown-body ol {
  padding-left: 1.8rem;
}

.markdown-body li { margin: 0.2rem 0; }
.markdown-body input[type="checkbox"] {
  width: 1em;
  height: 1em;
  margin: 0 0.45em 0 -1.45em;
  accent-color: var(--h2-color);
}

.markdown-body .empty { color: var(--h6-color); }
"#;

/// 选择相关的 JS 脚本体（自执行 IIFE，幂等）。
///
/// 同一份代码会通过两条路径注入，互为兜底：
/// 1. 写进预览 HTML 的内联 `<script>`（一定执行，是主路径）；
/// 2. `with_initialization_script`（文档创建时最早执行，作为补充）。
///
/// 之所以要双保险：在 `build_as_child` 创建的子 WKWebView 上，
/// `with_initialization_script` 走的是 `addUserScript`，已知在部分 macOS/wry
/// 版本上对子 webview 不可靠（注入可能被静默丢弃）；而内联 `<script>` 是文档
/// 内容的一部分，必定运行。脚本内部用 `__moranote_selection_bound` 标志保证
/// 无论被注入几次都只真正生效一次。
///
/// # 选择策略
/// 在嵌入式（`build_as_child`）WKWebView 里，**macOS 原生选区高亮是一层叠在
/// 内容之上的原生 NSView overlay，它和 DOM 实际选区会失步**——这正是「选不全 /
/// 拖不动 / 退化成整行」的真正根因（Craft 团队公开复盘过同一问题）。因此这里
/// **由 JS 完全接管拖选，并用 CSS Custom Highlight API 自绘高亮**，彻底绕开那层
/// 会失步的原生 overlay。macOS 26 / WebKit 26 原生支持 Highlight API。
///
/// 关键点：
/// - `pointerdown` 抓锚点，`pointermove` 持续扩展，全程 `preventDefault` 抢占，
///   阻止原生选区/拖拽介入；
/// - `caretRangeFromPoint` 在行尾空白、行间间隙会返回 `null`——这是上一版「跨行
///   选不全」的元凶。这里用「把 x 钳制进内容盒 + 沿 y 逐步回退找最近文本」的兜底，
///   保证跨行拖动时焦点一直跟手；
/// - 用 `setBaseAndExtent` 同步真实 DOM 选区，确保 `Cmd+C` / 右键复制拿到的就是
///   高亮的那段文本；
/// - 用 `::highlight()` 画高亮，原生 `::selection` 同时置透明，避免双层高亮打架。
const SELECTION_SCRIPT_BODY: &str = r#"
(function () {
  if (window.__moranote_selection_bound) return;
  window.__moranote_selection_bound = true;

  var HL_NAME = 'moranote-sel';
  var supportsHighlight = (typeof Highlight !== 'undefined') &&
    window.CSS && CSS.highlights && (typeof CSS.highlights.set === 'function');

  function injectSelectionStyle() {
    if (document.getElementById('__moranote_selection_style')) return;
    var style = document.createElement('style');
    style.id = '__moranote_selection_style';
    var rules = [
      'html, body, .markdown-body, .markdown-body * {',
      '  -webkit-user-select: text !important;',
      '  user-select: text !important;',
      '  -webkit-touch-callout: default !important;',
      '}',
      'img, .markdown-body img { -webkit-user-drag: none !important; }',
      '.markdown-body { cursor: text !important; }',
      '.markdown-body a, .markdown-body button, .markdown-body input, .markdown-body summary, .markdown-body label { cursor: pointer !important; }'
    ];
    if (supportsHighlight) {
      // 自绘高亮，原生选区高亮置透明（绕开会失步的原生 overlay）
      rules.push('.markdown-body ::selection { background: transparent !important; }');
      rules.push('::highlight(' + HL_NAME + ') { background-color: rgba(126,163,136,0.40); color: inherit; }');
    } else {
      rules.push('.markdown-body ::selection { background: rgba(126,163,136,0.40); }');
    }
    style.textContent = rules.join('\n');
    (document.head || document.documentElement).appendChild(style);
  }

  var root = null;
  function contentRoot() {
    if (root && document.contains(root)) return root;
    root = document.querySelector('.markdown-body') || document.body;
    return root;
  }

  // 把点 (x,y) 解析成一个文本 caret 位置（{node, offset}）。
  // 直接命中文本时用 caretRangeFromPoint；落在空白/行间时，先把 x 钳制进内容盒，
  // 再沿 y 微调，尽力找到同一视觉行里最近的文本位置，避免跨行拖动时丢点。
  function caretFromPoint(x, y) {
    var r = caretRange(x, y);
    if (r) return r;
    var box = contentRoot().getBoundingClientRect();
    var cx = Math.min(Math.max(x, box.left + 1), box.right - 1);
    r = caretRange(cx, y);
    if (r) return r;
    // 沿 y 方向上下各探一点，命中相邻行
    for (var dy = 2; dy <= 28; dy += 2) {
      r = caretRange(cx, y - dy);
      if (r) return r;
      r = caretRange(cx, y + dy);
      if (r) return r;
    }
    return null;
  }

  function caretRange(x, y) {
    var range = null;
    if (document.caretRangeFromPoint) {
      range = document.caretRangeFromPoint(x, y);
    } else if (document.caretPositionFromPoint) {
      var pos = document.caretPositionFromPoint(x, y);
      if (pos) {
        range = document.createRange();
        range.setStart(pos.offsetNode, pos.offset);
        range.collapse(true);
      }
    }
    if (!range) return null;
    // 只接受落在内容区里的位置
    if (!contentRoot().contains(range.startContainer)) return null;
    return { node: range.startContainer, offset: range.startOffset };
  }

  var anchor = null;        // 选区锚点 {node, offset}
  var dragging = false;
  var moved = false;
  var pointerId = null;

  function clearHighlight() {
    if (supportsHighlight) CSS.highlights.delete(HL_NAME);
    var sel = window.getSelection();
    if (sel) sel.removeAllRanges();
  }

  function paint(focus) {
    if (!anchor || !focus) return;
    var range = document.createRange();
    // 按文档顺序摆正 start/end，支持向上、向左反向拖选
    var cmp = comparePoints(anchor, focus);
    var a = cmp <= 0 ? anchor : focus;
    var b = cmp <= 0 ? focus : anchor;
    try {
      range.setStart(a.node, a.offset);
      range.setEnd(b.node, b.offset);
    } catch (e) { return; }

    if (supportsHighlight) {
      CSS.highlights.set(HL_NAME, new Highlight(range));
    }
    // 同步真实 DOM 选区，保证复制内容正确
    var sel = window.getSelection();
    if (sel) {
      try { sel.setBaseAndExtent(a.node, a.offset, b.node, b.offset); }
      catch (e) { sel.removeAllRanges(); sel.addRange(range.cloneRange()); }
    }
  }

  function comparePoints(p, q) {
    if (p.node === q.node) return p.offset - q.offset;
    var pos = p.node.compareDocumentPosition(q.node);
    if (pos & Node.DOCUMENT_POSITION_FOLLOWING) return -1;
    if (pos & Node.DOCUMENT_POSITION_PRECEDING) return 1;
    return 0;
  }

  function isInteractive(target) {
    return !!(target && target.closest &&
      target.closest('a, button, input, textarea, select, summary, label, .copy-code-button'));
  }

  function onPointerDown(e) {
    if (e.button !== 0) return;
    if (isInteractive(e.target)) return;        // 链接/按钮交给原生
    var c = caretFromPoint(e.clientX, e.clientY);
    if (!c) return;
    anchor = c;
    dragging = true;
    moved = false;
    pointerId = e.pointerId;
    clearHighlight();
    e.preventDefault();                         // 抢占，阻止原生拖拽消歧 + overlay
  }

  function onPointerMove(e) {
    if (!dragging) return;
    var c = caretFromPoint(e.clientX, e.clientY);
    if (!c) return;
    moved = true;
    paint(c);
    e.preventDefault();
  }

  function onPointerUp(e) {
    if (!dragging) return;
    dragging = false;
    pointerId = null;
    if (!moved) clearHighlight();               // 纯点击：清掉，不留高亮
  }

  function bindEvents() {
    var rootEl = contentRoot();
    // 绑在内容根上，动态替换内容（__setRendered）后仍生效，因为根节点不变
    rootEl.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('pointermove', onPointerMove, true);
    document.addEventListener('pointerup', onPointerUp, true);
    document.addEventListener('pointercancel', onPointerUp, true);
    // 双击/三击交给原生选词选段，但同样自绘高亮
    document.addEventListener('selectionchange', function () {
      if (dragging) return;                     // 拖动期间由我们接管，忽略
      if (!supportsHighlight) return;
      var sel = window.getSelection();
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
        CSS.highlights.delete(HL_NAME);
        return;
      }
      var r = sel.getRangeAt(0);
      if (!contentRoot().contains(r.commonAncestorContainer)) return;
      CSS.highlights.set(HL_NAME, new Highlight(r.cloneRange()));
    });
  }

  function init() {
    injectSelectionStyle();
    bindEvents();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
"#;

/// `with_initialization_script` 注入口（文档创建时最早执行）。
///
/// 与内联脚本同源，互为兜底。详见 [`SELECTION_SCRIPT_BODY`]。
pub const SELECTION_INIT_SCRIPT: &str = SELECTION_SCRIPT_BODY;

fn preview_runtime_script() -> String {
    // 末尾拼接选择脚本：内联 `<script>` 必定执行，是逐字符选择的主路径，
    // 不依赖在子 webview 上不可靠的 `with_initialization_script`。
    format!("{}\n{}", PREVIEW_RUNTIME_SCRIPT_BODY, SELECTION_SCRIPT_BODY)
}

const PREVIEW_RUNTIME_SCRIPT_BODY: &str = r#"
function fallbackCopyText(text) {
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.left = '-9999px';
  document.body.appendChild(area);
  area.select();
  let ok = false;
  try { ok = document.execCommand('copy'); } catch (_) { ok = false; }
  area.remove();
  if (!ok) throw new Error('copy failed');
}

async function writeClipboardText(text) {
  if (navigator.clipboard && navigator.clipboard.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch (_) {}
  }
  fallbackCopyText(text);
}

function showCopyFeedback(button, ok) {
  const label = button.querySelector('.copy-label');
  button.classList.toggle('copied', ok);
  if (label) label.textContent = ok ? '已复制' : '复制失败';
  window.setTimeout(() => {
    button.classList.remove('copied');
    if (label) label.textContent = '复制';
  }, 1400);
}

function decorateCodeBlocks() {
  document.querySelectorAll('.markdown-body pre').forEach((pre) => {
    if (pre.dataset.copyReady === 'true') return;
    const code = pre.querySelector('code');
    if (!code) return;
    pre.dataset.copyReady = 'true';

    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'copy-code-button';
    button.title = '复制代码';
    button.setAttribute('aria-label', '复制代码');
    button.innerHTML = '<span class="copy-icon" aria-hidden="true"></span><span class="copy-label">复制</span>';
    button.addEventListener('click', async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        await writeClipboardText(code.textContent || '');
        showCopyFeedback(button, true);
      } catch (_) {
        showCopyFeedback(button, false);
      }
    });
    pre.appendChild(button);
  });
}

window.__setRendered = function(payload) {
  const target = document.getElementById('content');
  target.innerHTML = payload.html || '';
  document.documentElement.dataset.words = String(payload.stats?.words || 0);
  decorateCodeBlocks();
};

window.__scrollToAnchor = function(anchor) {
  const target = document.getElementById(anchor);
  if (target) target.scrollIntoView({ block: 'start', behavior: 'smooth' });
};

window.addEventListener('DOMContentLoaded', decorateCodeBlocks);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_front_matter_outline_and_stats() {
        let rendered = render_markdown(RenderRequest {
            markdown: "---\ntitle: Demo\n---\n# One\n\nhello world\n\n## Two".to_string(),
            base_dir: None,
        });

        assert_eq!(
            rendered.front_matter.get("title"),
            Some(&"Demo".to_string())
        );
        assert_eq!(rendered.outline.len(), 2);
        assert!(rendered.stats.words >= 2);
        assert!(rendered.html.contains("id=\"one-1\""));
    }

    #[test]
    fn morandi_theme_urls_are_export_rewritable() {
        let rendered = render_markdown(RenderRequest {
            markdown: "# Title".to_string(),
            base_dir: None,
        });

        assert!(
            rendered
                .html
                .contains("mdres://localhost/__theme/morandigarden/morandigarden/")
        );

        let exported = html_for_export(&rendered.html);
        assert!(!exported.contains("mdres://localhost/__theme/morandigarden/"));
        assert!(exported.contains("morandigarden/AlibabaPuHuiTi"));
    }

    #[test]
    fn relative_resources_are_exported_as_file_urls() {
        let rendered = render_markdown(RenderRequest {
            markdown: "![Logo](images/logo%201.png)\n\n[Guide](docs/start.md#intro)".to_string(),
            base_dir: None,
        });
        let exported =
            html_for_export_with_base(&rendered.html, Some(Path::new("/tmp/markdown assets")));

        assert!(
            exported.contains("file:///tmp/markdown%20assets/images/logo%201.png"),
            "{exported}"
        );
        assert!(exported.contains("file:///tmp/markdown%20assets/docs/start.md#intro"));
        assert!(exported.contains("morandigarden/AlibabaPuHuiTi"));
    }

    #[test]
    fn code_blocks_include_copy_runtime() {
        let rendered = render_markdown(RenderRequest {
            markdown: "```rust\nfn main() {}\n```".to_string(),
            base_dir: None,
        });

        assert!(rendered.html.contains("copy-code-button"));
        assert!(rendered.html.contains("decorateCodeBlocks"));
        assert!(rendered.html.contains("已复制"));
    }

    #[test]
    fn denies_resource_path_traversal() {
        let root = Arc::new(Mutex::new(Some(PathBuf::from("."))));
        let response = resource_response(&root, "../Cargo.toml");
        assert_ne!(response.status(), StatusCode::OK);
    }
}
