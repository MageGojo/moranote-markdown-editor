use crate::preview::{
    RenderedDocument, copy_morandi_font_assets, file_url_for_path, html_for_export,
    html_for_export_with_asset_package, html_for_export_with_base,
};
use anyhow::{Context as _, anyhow};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use zip::write::{SimpleFileOptions, ZipWriter};

const PANDOC_EXPORT_TIMEOUT: Duration = Duration::from_secs(90);
const CHROMIUM_EXPORT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    Html,
    HtmlPlain,
    Pdf,
    Png,
    Jpeg,
    Docx,
    Odt,
    Rtf,
    Epub,
    Latex,
    Rst,
    Textile,
    Opml,
    RevealJs,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Html => "HTML",
            ExportFormat::HtmlPlain => "HTML (无样式)",
            ExportFormat::Pdf => "PDF",
            ExportFormat::Png => "PNG",
            ExportFormat::Jpeg => "JPEG",
            ExportFormat::Docx => "Word (.docx)",
            ExportFormat::Odt => "OpenOffice (.odt)",
            ExportFormat::Rtf => "RTF",
            ExportFormat::Epub => "EPUB",
            ExportFormat::Latex => "LaTeX",
            ExportFormat::Rst => "reStructuredText",
            ExportFormat::Textile => "Textile",
            ExportFormat::Opml => "OPML",
            ExportFormat::RevealJs => "RevealJS",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Html | ExportFormat::HtmlPlain => "html",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Png => "png",
            ExportFormat::Jpeg => "jpg",
            ExportFormat::Docx => "docx",
            ExportFormat::Odt => "odt",
            ExportFormat::Rtf => "rtf",
            ExportFormat::Epub => "epub",
            ExportFormat::Latex => "tex",
            ExportFormat::Rst => "rst",
            ExportFormat::Textile => "textile",
            ExportFormat::Opml => "opml",
            ExportFormat::RevealJs => "html",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Html,
            Self::HtmlPlain,
            Self::Pdf,
            Self::Png,
            Self::Jpeg,
            Self::Docx,
            Self::Odt,
            Self::Rtf,
            Self::Epub,
            Self::Latex,
            Self::Rst,
            Self::Textile,
            Self::Opml,
            Self::RevealJs,
        ]
    }

    fn pandoc_target(self) -> Option<&'static str> {
        match self {
            ExportFormat::Docx => Some("docx"),
            ExportFormat::Odt => Some("odt"),
            ExportFormat::Rtf => Some("rtf"),
            ExportFormat::Epub => Some("epub3"),
            ExportFormat::Latex => Some("latex"),
            ExportFormat::Rst => Some("rst"),
            ExportFormat::Textile => Some("textile"),
            ExportFormat::Opml => Some("opml"),
            ExportFormat::RevealJs => Some("revealjs"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub rendered: RenderedDocument,
    pub base_dir: Option<PathBuf>,
    pub output_path: PathBuf,
    pub pandoc_path: Option<PathBuf>,
    pub chromium_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExportResult {
    pub path: PathBuf,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportProfile {
    Html,
    Print,
    Image,
    Pandoc,
}

struct ExportAssetPackage {
    dir: PathBuf,
    url_prefix: String,
}

pub fn export_document(request: ExportRequest) -> anyhow::Result<ExportResult> {
    match request.format {
        ExportFormat::Html => {
            if let Some(parent) = request.output_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("创建导出目录失败: {}", parent.display()))?;
            }
            let package = output_asset_package(&request.output_path)?;
            copy_morandi_font_assets(&package.dir).with_context(|| {
                format!("复制 MorandiGarden 字体失败: {}", package.dir.display())
            })?;
            let html = export_html(
                &request.rendered,
                request.base_dir.as_deref(),
                ExportProfile::Html,
                Some(&package),
            )?;
            std::fs::write(&request.output_path, html.as_bytes())?;
            Ok(ExportResult {
                path: request.output_path,
                command: None,
            })
        }
        ExportFormat::HtmlPlain => {
            ensure_output_parent(&request.output_path)?;
            let html = export_html(
                &request.rendered,
                request.base_dir.as_deref(),
                ExportProfile::Pandoc,
                None,
            )?;
            std::fs::write(&request.output_path, plain_html(&html).as_bytes())?;
            Ok(ExportResult {
                path: request.output_path,
                command: None,
            })
        }
        ExportFormat::Docx => export_docx_builtin(request),
        ExportFormat::Pdf => export_pdf_with_chromium(request),
        ExportFormat::Png | ExportFormat::Jpeg => export_image_with_chromium(request),
        format => export_with_pandoc(format, request),
    }
}

pub fn default_export_path(
    source_path: Option<&Path>,
    default_dir: Option<&Path>,
    format: ExportFormat,
) -> PathBuf {
    let stem = source_path
        .and_then(|path| path.file_stem())
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    let dir = default_dir
        .map(Path::to_path_buf)
        .or_else(|| {
            source_path
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    dir.join(format!("{stem}.{}", format.extension()))
}

fn export_docx_builtin(request: ExportRequest) -> anyhow::Result<ExportResult> {
    ensure_output_parent(&request.output_path)?;
    let html = export_html(
        &request.rendered,
        request.base_dir.as_deref(),
        ExportProfile::Pandoc,
        None,
    )?;
    let blocks = docx_blocks_from_html(&html);
    write_docx_package(&request.output_path, &request.rendered, &blocks)?;

    Ok(ExportResult {
        path: request.output_path,
        command: Some("内置 Word 导出".to_string()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DocxBlock {
    Paragraph { style: DocxStyle, text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocxStyle {
    Normal,
    Heading(u8),
    Quote,
    List,
    Code,
}

fn docx_blocks_from_html(html: &str) -> Vec<DocxBlock> {
    let body = extract_main_body(html);
    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut style = DocxStyle::Normal;
    let mut in_pre = false;
    let mut index = 0;

    while index < body.len() {
        let rest = &body[index..];
        let Some(tag_start) = rest.find('<') else {
            append_docx_text(&mut text, rest, in_pre);
            break;
        };

        append_docx_text(&mut text, &rest[..tag_start], in_pre);
        index += tag_start;
        let rest = &body[index..];
        let Some(tag_end) = rest.find('>') else {
            append_docx_text(&mut text, rest, in_pre);
            break;
        };

        let tag = &rest[1..tag_end];
        let tag_name = html_tag_name(tag);
        let closing = tag.trim_start().starts_with('/');

        match (closing, tag_name.as_deref()) {
            (false, Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6")) => {
                push_docx_block(&mut blocks, &mut text, style);
                let level = tag_name
                    .as_deref()
                    .and_then(|name| name[1..].parse::<u8>().ok())
                    .unwrap_or(1);
                style = DocxStyle::Heading(level);
            }
            (true, Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (false, Some("p")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (true, Some("p")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (false, Some("blockquote")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Quote;
            }
            (true, Some("blockquote")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (false, Some("li")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::List;
                text.push_str("• ");
            }
            (true, Some("li")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (false, Some("pre")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Code;
                in_pre = true;
            }
            (true, Some("pre")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
                in_pre = false;
            }
            (_, Some("br")) => text.push('\n'),
            (false, Some("tr")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (true, Some("tr")) => {
                push_docx_block(&mut blocks, &mut text, style);
                style = DocxStyle::Normal;
            }
            (false, Some("td" | "th")) => {
                if !text.trim().is_empty() && !text.ends_with('\t') {
                    text.push('\t');
                }
            }
            (false, Some("img")) => {
                let label = html_attr(tag, "alt")
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| html_attr(tag, "src"))
                    .unwrap_or_else(|| "图片".to_string());
                append_docx_text(&mut text, &format!("[图片: {label}]"), false);
            }
            _ => {}
        }

        index += tag_end + 1;
    }

    push_docx_block(&mut blocks, &mut text, style);
    if blocks.is_empty() {
        blocks.push(DocxBlock::Paragraph {
            style: DocxStyle::Normal,
            text: "Empty document".to_string(),
        });
    }
    blocks
}

fn append_docx_text(target: &mut String, text: &str, preserve: bool) {
    let decoded = decode_html_entities(text);
    if preserve {
        target.push_str(&decoded);
        return;
    }

    for part in decoded.split_whitespace() {
        if !target.is_empty()
            && !target.ends_with([' ', '\n', '\t'])
            && !matches!(target.chars().last(), Some('•'))
        {
            target.push(' ');
        }
        target.push_str(part);
    }
}

fn push_docx_block(blocks: &mut Vec<DocxBlock>, text: &mut String, style: DocxStyle) {
    let normalized = if style == DocxStyle::Code {
        text.trim_matches('\n').to_string()
    } else {
        text.trim().to_string()
    };
    text.clear();

    if normalized.is_empty() {
        return;
    }

    blocks.push(DocxBlock::Paragraph {
        style,
        text: normalized,
    });
}

fn extract_main_body(html: &str) -> String {
    html.find("<main")
        .and_then(|start| html[start..].find('>').map(|end| start + end + 1))
        .and_then(|body_start| {
            html[body_start..]
                .find("</main>")
                .map(|body_end| html[body_start..body_start + body_end].to_string())
        })
        .unwrap_or_else(|| html.to_string())
}

fn html_tag_name(tag: &str) -> Option<String> {
    let tag = tag
        .trim()
        .trim_start_matches('/')
        .trim_start_matches('!')
        .trim_start_matches('?');
    let name = tag
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_ascii_lowercase();
    (!name.is_empty()).then_some(name)
}

fn html_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(decode_html_entities(&tag[start..start + end]))
}

fn decode_html_entities(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        let Some(end) = rest.find(';') else {
            output.push('&');
            output.push_str(rest);
            return output;
        };
        let entity = &rest[..end];
        match entity {
            "amp" => output.push('&'),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            "quot" => output.push('"'),
            "apos" | "#39" => output.push('\''),
            _ if entity.starts_with("#x") => {
                if let Ok(code) = u32::from_str_radix(&entity[2..], 16) {
                    if let Some(ch) = char::from_u32(code) {
                        output.push(ch);
                    }
                }
            }
            _ if entity.starts_with('#') => {
                if let Ok(code) = entity[1..].parse::<u32>() {
                    if let Some(ch) = char::from_u32(code) {
                        output.push(ch);
                    }
                }
            }
            _ => {
                output.push('&');
                output.push_str(entity);
                output.push(';');
            }
        }
        rest = &rest[end + 1..];
    }

    output.push_str(rest);
    output
}

fn write_docx_package(
    output_path: &Path,
    rendered: &RenderedDocument,
    blocks: &[DocxBlock],
) -> anyhow::Result<()> {
    let file = std::fs::File::create(output_path)
        .with_context(|| format!("创建 Word 文件失败: {}", output_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    write_zip_file(&mut zip, options, "[Content_Types].xml", CONTENT_TYPES_XML)?;
    write_zip_file(&mut zip, options, "_rels/.rels", PACKAGE_RELS_XML)?;
    write_zip_file(&mut zip, options, "word/styles.xml", DOCX_STYLES_XML)?;
    write_zip_file(&mut zip, options, "word/settings.xml", DOCX_SETTINGS_XML)?;
    write_zip_file(&mut zip, options, "docProps/app.xml", DOCX_APP_XML)?;
    write_zip_file(
        &mut zip,
        options,
        "docProps/core.xml",
        &docx_core_xml(rendered),
    )?;
    write_zip_file(
        &mut zip,
        options,
        "word/document.xml",
        &docx_document_xml(blocks),
    )?;
    zip.finish()?;
    Ok(())
}

fn write_zip_file(
    zip: &mut ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
    name: &str,
    content: &str,
) -> anyhow::Result<()> {
    zip.start_file(name, options)?;
    zip.write_all(content.as_bytes())?;
    Ok(())
}

fn docx_document_xml(blocks: &[DocxBlock]) -> String {
    let mut body = String::new();
    for block in blocks {
        match block {
            DocxBlock::Paragraph { style, text } => {
                body.push_str(&docx_paragraph(*style, text));
            }
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:w10="urn:schemas-microsoft-com:office:word" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wpi="http://schemas.microsoft.com/office/word/2010/wordprocessingInk" xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" mc:Ignorable="w14 wp14">
  <w:body>
    {body}
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1440" w:right="1530" w:bottom="1440" w:left="1530" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
}

fn docx_paragraph(style: DocxStyle, text: &str) -> String {
    let style_id = match style {
        DocxStyle::Normal => None,
        DocxStyle::Heading(level) => Some(format!("Heading{}", level.min(6))),
        DocxStyle::Quote => Some("Quote".to_string()),
        DocxStyle::List => Some("ListParagraph".to_string()),
        DocxStyle::Code => Some("CodeBlock".to_string()),
    };
    let style_xml = style_id
        .map(|style_id| format!(r#"<w:pStyle w:val="{style_id}"/>"#))
        .unwrap_or_default();
    let run_pr = match style {
        DocxStyle::Code => {
            r#"<w:rPr><w:rFonts w:ascii="JetBrains Mono NL" w:hAnsi="JetBrains Mono NL"/><w:sz w:val="20"/></w:rPr>"#
        }
        _ => "",
    };

    format!(
        r#"<w:p><w:pPr>{style_xml}</w:pPr><w:r>{run_pr}{}</w:r></w:p>"#,
        docx_text_runs(text)
    )
}

fn docx_text_runs(text: &str) -> String {
    let mut xml = String::new();
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            xml.push_str("<w:br/>");
        }
        xml.push_str(&format!(
            r#"<w:t xml:space="preserve">{}</w:t>"#,
            escape_xml(line)
        ));
    }
    xml
}

fn docx_core_xml(rendered: &RenderedDocument) -> String {
    let title = export_title(rendered);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{}</dc:title>
  <dc:creator>MoraNote</dc:creator>
  <cp:lastModifiedBy>MoraNote</cp:lastModifiedBy>
</cp:coreProperties>"#,
        escape_xml(&title)
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#;

const PACKAGE_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

const DOCX_SETTINGS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:zoom w:percent="100"/>
  <w:defaultTabStop w:val="720"/>
</w:settings>"#;

const DOCX_APP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>MoraNote</Application>
</Properties>"#;

const DOCX_STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:spacing w:after="160" w:line="360" w:lineRule="auto"/></w:pPr>
    <w:rPr><w:rFonts w:ascii="Alibaba PuHuiTi 3.0" w:hAnsi="Alibaba PuHuiTi 3.0" w:eastAsia="Alibaba PuHuiTi 3.0"/><w:sz w:val="24"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="360" w:after="220"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:color w:val="7A5F52"/><w:sz w:val="40"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="300" w:after="180"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:color w:val="506956"/><w:sz w:val="32"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="260" w:after="160"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:color w:val="7A6F89"/><w:sz w:val="28"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="220" w:after="140"/><w:outlineLvl w:val="3"/></w:pPr><w:rPr><w:b/><w:color w:val="6F7F88"/><w:sz w:val="25"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="200" w:after="120"/><w:outlineLvl w:val="4"/></w:pPr><w:rPr><w:b/><w:color w:val="8A7A6F"/><w:sz w:val="23"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="180" w:after="100"/><w:outlineLvl w:val="5"/></w:pPr><w:rPr><w:b/><w:color w:val="7D7A71"/><w:sz w:val="22"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:ind w:left="360"/><w:spacing w:before="120" w:after="160"/><w:shd w:fill="EEF4EF"/></w:pPr><w:rPr><w:i/><w:color w:val="7A5F52"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:ind w:left="420"/></w:pPr></w:style>
  <w:style w:type="paragraph" w:styleId="CodeBlock"><w:name w:val="Code Block"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="120" w:after="120"/><w:shd w:fill="F0F5F3"/></w:pPr><w:rPr><w:rFonts w:ascii="JetBrains Mono NL" w:hAnsi="JetBrains Mono NL"/><w:sz w:val="20"/></w:rPr></w:style>
</w:styles>"#;

fn export_with_pandoc(
    format: ExportFormat,
    request: ExportRequest,
) -> anyhow::Result<ExportResult> {
    let pandoc = request
        .pandoc_path
        .clone()
        .or_else(|| find_on_path("pandoc"))
        .ok_or_else(|| anyhow!("需要配置 Pandoc 才能导出 {}", format.label()))?;
    let target = format
        .pandoc_target()
        .ok_or_else(|| anyhow!("格式 {} 不支持 Pandoc 导出", format.label()))?;
    ensure_output_parent(&request.output_path)?;
    let preview_input = materialize_preview_export(
        &request.rendered,
        request.base_dir.as_deref(),
        ExportProfile::Pandoc,
    )?;
    let args = build_pandoc_args(
        &preview_input.html_path,
        &request.output_path,
        "html",
        target,
        format,
        &preview_input.dir,
    );

    let mut command = Command::new(&pandoc);
    command.args(&args);
    let output = run_command_with_timeout(&mut command, PANDOC_EXPORT_TIMEOUT, "Pandoc")
        .with_context(|| format!("启动 Pandoc 失败: {}", pandoc.display()))?;

    let _ = std::fs::remove_dir_all(&preview_input.dir);
    if output.timed_out {
        return Err(anyhow!(
            "Pandoc 导出超时，已停止等待: {}{}",
            format.label(),
            command_stderr(&output.stderr)
        ));
    }
    if !output.success {
        return Err(anyhow!(
            "Pandoc 导出失败: {}{}",
            format.label(),
            command_stderr(&output.stderr)
        ));
    }

    Ok(ExportResult {
        path: request.output_path,
        command: Some(format!("{} {}", pandoc.display(), args.join(" "))),
    })
}

fn export_pdf_with_chromium(request: ExportRequest) -> anyhow::Result<ExportResult> {
    let chromium = resolve_chromium(request.chromium_path.as_deref())
        .ok_or_else(|| anyhow!("需要配置 Chromium/Chrome 才能按预览效果导出 PDF"))?;
    ensure_output_parent(&request.output_path)?;
    let preview_input = materialize_preview_export(
        &request.rendered,
        request.base_dir.as_deref(),
        ExportProfile::Print,
    )?;
    let pdf_arg = format!("--print-to-pdf={}", request.output_path.display());
    let user_data_arg = format!(
        "--user-data-dir={}",
        preview_input.dir.join("chrome-profile").display()
    );
    let url = file_url(&preview_input.html_path);

    let mut command = Command::new(&chromium);
    command.args([
        "--headless",
        "--disable-gpu",
        "--disable-dev-shm-usage",
        "--allow-file-access-from-files",
        "--print-to-pdf-no-header",
        "--no-pdf-header-footer",
        "--virtual-time-budget=1000",
        user_data_arg.as_str(),
        pdf_arg.as_str(),
        url.as_str(),
    ]);
    let output = run_command_with_timeout(&mut command, CHROMIUM_EXPORT_TIMEOUT, "Chromium")
        .with_context(|| format!("启动 Chromium 失败: {}", chromium.display()))?;

    let _ = std::fs::remove_dir_all(&preview_input.dir);
    if output.timed_out {
        return Err(anyhow!(
            "Chromium PDF 导出超时，已停止等待{}",
            command_stderr(&output.stderr)
        ));
    }
    if !output.success {
        return Err(anyhow!(
            "Chromium PDF 导出失败{}",
            command_stderr(&output.stderr)
        ));
    }

    Ok(ExportResult {
        path: request.output_path,
        command: Some(format!("{} --headless --print-to-pdf", chromium.display())),
    })
}

fn export_image_with_chromium(request: ExportRequest) -> anyhow::Result<ExportResult> {
    let chromium = resolve_chromium(request.chromium_path.as_deref())
        .ok_or_else(|| anyhow!("需要配置 Chromium/Chrome 才能按预览效果导出图片"))?;

    ensure_output_parent(&request.output_path)?;
    let preview_input = materialize_preview_export(
        &request.rendered,
        request.base_dir.as_deref(),
        ExportProfile::Image,
    )?;
    let url = file_url(&preview_input.html_path);
    let screenshot_path = if request.format == ExportFormat::Jpeg {
        preview_input.dir.join("screenshot.png")
    } else {
        request.output_path.clone()
    };
    let screenshot_arg = format!("--screenshot={}", screenshot_path.display());
    let user_data_arg = format!(
        "--user-data-dir={}",
        preview_input.dir.join("chrome-profile").display()
    );
    let mut command = Command::new(&chromium);
    command.args([
        "--headless",
        "--disable-gpu",
        "--disable-dev-shm-usage",
        "--allow-file-access-from-files",
        "--hide-scrollbars",
        "--run-all-compositor-stages-before-draw",
        "--virtual-time-budget=1000",
        "--window-size=1200,1600",
        user_data_arg.as_str(),
        screenshot_arg.as_str(),
        url.as_str(),
    ]);
    let output = run_command_with_timeout(&mut command, CHROMIUM_EXPORT_TIMEOUT, "Chromium")
        .with_context(|| format!("启动 Chromium 失败: {}", chromium.display()))?;

    if output.timed_out {
        let _ = std::fs::remove_dir_all(&preview_input.dir);
        return Err(anyhow!(
            "Chromium 图片导出超时，已停止等待{}",
            command_stderr(&output.stderr)
        ));
    }

    if !output.success {
        let _ = std::fs::remove_dir_all(&preview_input.dir);
        return Err(anyhow!(
            "Chromium 图片导出失败{}",
            command_stderr(&output.stderr)
        ));
    }

    if request.format == ExportFormat::Jpeg {
        ImageReader::open(&screenshot_path)
            .with_context(|| format!("读取临时 PNG 失败: {}", screenshot_path.display()))?
            .decode()
            .context("解析临时 PNG 失败")?
            .save_with_format(&request.output_path, image::ImageFormat::Jpeg)
            .with_context(|| format!("写入 JPEG 失败: {}", request.output_path.display()))?;
    }

    let _ = std::fs::remove_dir_all(&preview_input.dir);

    Ok(ExportResult {
        path: request.output_path,
        command: Some(format!("{} --headless --screenshot", chromium.display())),
    })
}

fn build_pandoc_args(
    input: &Path,
    output: &Path,
    source: &str,
    target: &str,
    format: ExportFormat,
    resource_dir: &Path,
) -> Vec<String> {
    let mut args = vec![
        input.display().to_string(),
        "--standalone".to_string(),
        "-f".to_string(),
        source.to_string(),
        "-t".to_string(),
        target.to_string(),
        "-o".to_string(),
        output.display().to_string(),
        format!("--resource-path={}", resource_dir.display()),
    ];

    if matches!(format, ExportFormat::RevealJs) {
        args.push("--slide-level=2".to_string());
    }

    args
}

struct PreviewExportInput {
    dir: PathBuf,
    html_path: PathBuf,
}

fn materialize_preview_export(
    rendered: &RenderedDocument,
    base_dir: Option<&Path>,
    profile: ExportProfile,
) -> anyhow::Result<PreviewExportInput> {
    let dir = temp_path("markdown-editor-preview-export", "dir");
    std::fs::create_dir_all(&dir)?;
    let package = ExportAssetPackage {
        dir: dir.join("preview.assets"),
        url_prefix: "preview.assets".to_string(),
    };
    copy_morandi_font_assets(&package.dir)
        .with_context(|| format!("复制 MorandiGarden 字体失败: {}", package.dir.display()))?;
    let html_path = dir.join("preview.html");
    std::fs::write(
        &html_path,
        export_html(rendered, base_dir, profile, Some(&package))?.as_bytes(),
    )?;
    Ok(PreviewExportInput { dir, html_path })
}

fn export_html(
    rendered: &RenderedDocument,
    base_dir: Option<&Path>,
    profile: ExportProfile,
    package: Option<&ExportAssetPackage>,
) -> anyhow::Result<String> {
    let html = if let Some(package) = package {
        html_for_export_with_asset_package(
            &rendered.html,
            base_dir,
            &package.url_prefix,
            &package.dir,
        )?
    } else if let Some(base_dir) = base_dir {
        html_for_export_with_base(&rendered.html, Some(base_dir))
    } else {
        html_for_export(&rendered.html)
    };

    Ok(apply_export_profile(html, rendered, profile))
}

fn ensure_output_parent(output_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建导出目录失败: {}", parent.display()))?;
    }
    Ok(())
}

fn output_asset_package(output_path: &Path) -> anyhow::Result<ExportAssetPackage> {
    let parent = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = output_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| "export".to_string());
    let url_prefix = format!("{stem}.assets");
    let dir = parent.join(&url_prefix);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("创建导出资源目录失败: {}", dir.display()))?;
    Ok(ExportAssetPackage { dir, url_prefix })
}

fn apply_export_profile(
    mut html: String,
    rendered: &RenderedDocument,
    profile: ExportProfile,
) -> String {
    html = html.replacen(
        "<html lang=\"zh-CN\" data-theme=\"light\">",
        &format!(
            "<html lang=\"zh-CN\" data-theme=\"light\" data-export=\"{}\">",
            profile.data_value()
        ),
        1,
    );

    if !html.contains("<title>") {
        html = html.replacen(
            "<head>",
            &format!(
                "<head>\n<title>{}</title>",
                escape_html(&export_title(rendered))
            ),
            1,
        );
    }

    html = html.replacen("</style>", &format!("\n{}\n</style>", profile.css()), 1);

    if profile == ExportProfile::Pandoc {
        strip_preview_script(&html)
    } else {
        html
    }
}

impl ExportProfile {
    fn data_value(self) -> &'static str {
        match self {
            ExportProfile::Html => "typora-html",
            ExportProfile::Print => "typora-print",
            ExportProfile::Image => "typora-image",
            ExportProfile::Pandoc => "typora-pandoc",
        }
    }

    fn css(self) -> String {
        match self {
            ExportProfile::Html => format!("{TYPORA_BASE_EXPORT_CSS}{TYPORA_HTML_EXPORT_CSS}"),
            ExportProfile::Print => format!("{TYPORA_BASE_EXPORT_CSS}{TYPORA_PRINT_EXPORT_CSS}"),
            ExportProfile::Image => format!("{TYPORA_BASE_EXPORT_CSS}{TYPORA_IMAGE_EXPORT_CSS}"),
            ExportProfile::Pandoc => format!("{TYPORA_BASE_EXPORT_CSS}{TYPORA_PANDOC_EXPORT_CSS}"),
        }
    }
}

const TYPORA_BASE_EXPORT_CSS: &str = r#"
/* Typora-like export profile */
:root {
  --typora-export-width: 860px;
}

[data-export^="typora-"] body {
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
}

[data-export^="typora-"] .markdown-body {
  line-height: 1.82;
  overflow-wrap: anywhere;
}

[data-export^="typora-"] .markdown-body h1,
[data-export^="typora-"] .markdown-body h2,
[data-export^="typora-"] .markdown-body h3,
[data-export^="typora-"] .markdown-body h4,
[data-export^="typora-"] .markdown-body h5,
[data-export^="typora-"] .markdown-body h6 {
  break-after: avoid;
  page-break-after: avoid;
}

[data-export^="typora-"] .markdown-body pre,
[data-export^="typora-"] .markdown-body blockquote,
[data-export^="typora-"] .markdown-body table,
[data-export^="typora-"] .markdown-body img {
  break-inside: avoid;
  page-break-inside: avoid;
}

[data-export^="typora-"] .markdown-body table {
  display: table;
}
"#;

const TYPORA_HTML_EXPORT_CSS: &str = r#"
[data-export="typora-html"] body {
  background: var(--bg-color);
}

[data-export="typora-html"] .markdown-body {
  min-height: auto;
  margin: 48px auto;
  padding: 52px 68px 72px;
  border: 1px solid var(--soft-border-color);
  border-radius: 8px;
}
"#;

const TYPORA_PRINT_EXPORT_CSS: &str = r#"
@page {
  size: A4;
  margin: 18mm 20mm 20mm;
}

[data-export="typora-print"] body {
  background: #fff !important;
}

[data-export="typora-print"] .markdown-body {
  width: auto;
  min-height: auto;
  margin: 0;
  padding: 0;
  background: #fff;
  box-shadow: none;
}

[data-export="typora-print"] .markdown-body a {
  color: inherit;
  border-bottom-color: transparent;
}

[data-export="typora-print"] .copy-code-button {
  display: none !important;
}

@media print {
  body {
    background: #fff !important;
  }

  .markdown-body {
    width: auto !important;
    min-height: auto !important;
    margin: 0 !important;
    padding: 0 !important;
    background: #fff !important;
    box-shadow: none !important;
  }

  .copy-code-button {
    display: none !important;
  }
}
"#;

const TYPORA_IMAGE_EXPORT_CSS: &str = r#"
[data-export="typora-image"] body {
  background: var(--bg-color);
}

[data-export="typora-image"] .markdown-body {
  width: min(var(--typora-export-width), calc(100vw - 96px));
  min-height: auto;
  margin: 36px auto;
  padding: 48px 64px 64px;
}

[data-export="typora-image"] .copy-code-button {
  display: none !important;
}
"#;

const TYPORA_PANDOC_EXPORT_CSS: &str = r#"
[data-export="typora-pandoc"] .copy-code-button {
  display: none !important;
}
"#;

fn export_title(rendered: &RenderedDocument) -> String {
    rendered
        .front_matter
        .get("title")
        .filter(|title| !title.trim().is_empty())
        .cloned()
        .or_else(|| rendered.outline.first().map(|item| item.title.clone()))
        .unwrap_or_else(|| "Markdown Export".to_string())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn strip_preview_script(html: &str) -> String {
    let Some(start) = html.find("<script>") else {
        return html.to_string();
    };
    let Some(end) = html[start..].find("</script>") else {
        return html.to_string();
    };

    let script_end = start + end + "</script>".len();
    format!("{}{}", &html[..start], &html[script_end..])
}

fn plain_html(html: &str) -> String {
    let body = html
        .find("<main")
        .and_then(|start| html[start..].find('>').map(|end| start + end + 1))
        .and_then(|body_start| {
            html[body_start..]
                .find("</main>")
                .map(|body_end| html[body_start..body_start + body_end].to_string())
        })
        .unwrap_or_else(|| html.to_string());

    format!(
        "<!DOCTYPE html><html lang=\"zh-CN\"><head><meta charset=\"UTF-8\"><title>Export</title></head><body>{body}</body></html>"
    )
}

fn temp_path(prefix: &str, extension: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("{prefix}-{nanos}.{extension}"))
}

fn find_on_path(binary: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|path| path.join(binary))
        .find(|path| path.is_file())
}

struct CommandRun {
    success: bool,
    timed_out: bool,
    stderr: Vec<u8>,
}

fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
    label: &str,
) -> anyhow::Result<CommandRun> {
    let stderr_path = temp_path("markdown-editor-command-stderr", "log");
    let stderr_file = std::fs::File::create(&stderr_path)
        .with_context(|| format!("创建 {label} 日志文件失败"))?;
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("启动 {label} 失败"))?;
    let started = Instant::now();

    loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("等待 {label} 失败"))?
        {
            let stderr = std::fs::read(&stderr_path).unwrap_or_default();
            let _ = std::fs::remove_file(&stderr_path);
            return Ok(CommandRun {
                success: status.success(),
                timed_out: false,
                stderr,
            });
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = std::fs::read(&stderr_path).unwrap_or_default();
            let _ = std::fs::remove_file(&stderr_path);
            return Ok(CommandRun {
                success: false,
                timed_out: true,
                stderr,
            });
        }

        std::thread::sleep(Duration::from_millis(120));
    }
}

fn command_stderr(stderr: &[u8]) -> String {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.is_empty() {
        return String::new();
    }

    let mut snippet: String = message.chars().take(600).collect();
    if snippet.len() < message.len() {
        snippet.push_str("...");
    }
    format!(": {snippet}")
}

fn resolve_chromium(configured: Option<&Path>) -> Option<PathBuf> {
    configured
        .and_then(chromium_executable)
        .or_else(|| find_on_path("chromium"))
        .or_else(|| find_on_path("google-chrome"))
        .or_else(|| find_on_path("chrome"))
        .or_else(|| find_on_path("msedge"))
        .or_else(|| {
            [
                "/Applications/Google Chrome.app",
                "/Applications/Chromium.app",
                "/Applications/Microsoft Edge.app",
            ]
            .into_iter()
            .map(Path::new)
            .find_map(chromium_executable)
        })
        .or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from)?;
            [
                "Applications/Google Chrome.app",
                "Applications/Chromium.app",
                "Applications/Microsoft Edge.app",
            ]
            .into_iter()
            .map(|path| home.join(path))
            .find_map(|path| chromium_executable(&path))
        })
}

fn chromium_executable(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }

    let app_name = path.file_name()?.to_string_lossy();
    let executable = if app_name.contains("Google Chrome") {
        "Google Chrome"
    } else if app_name.contains("Chromium") {
        "Chromium"
    } else if app_name.contains("Microsoft Edge") {
        "Microsoft Edge"
    } else {
        return None;
    };

    let candidate = path.join("Contents").join("MacOS").join(executable);
    candidate.is_file().then_some(candidate)
}

fn file_url(path: &Path) -> String {
    file_url_for_path(path)
}

pub fn normalize_export_path(mut path: PathBuf, format: ExportFormat) -> PathBuf {
    let expected = format.extension();
    let current = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    if current.eq_ignore_ascii_case(expected) {
        return path;
    }

    path.set_extension(expected);
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{RenderRequest, render_markdown};

    #[test]
    fn export_path_uses_source_name_and_format_extension() {
        let path = default_export_path(
            Some(Path::new("/tmp/demo.note.md")),
            Some(Path::new("/exports")),
            ExportFormat::Docx,
        );
        assert_eq!(path, PathBuf::from("/exports/demo.note.docx"));
    }

    #[test]
    fn pandoc_args_are_stable() {
        let args = build_pandoc_args(
            Path::new("in.html"),
            Path::new("out.docx"),
            "html",
            "docx",
            ExportFormat::Docx,
            Path::new("assets"),
        );
        assert_eq!(
            args,
            [
                "in.html",
                "--standalone",
                "-f",
                "html",
                "-t",
                "docx",
                "-o",
                "out.docx",
                "--resource-path=assets"
            ]
        );
    }

    #[test]
    fn normalizes_wrong_export_extension() {
        let path = normalize_export_path(PathBuf::from("/tmp/demo.txt"), ExportFormat::Pdf);
        assert_eq!(path, PathBuf::from("/tmp/demo.pdf"));

        let path = normalize_export_path(PathBuf::from("/tmp/demo"), ExportFormat::Jpeg);
        assert_eq!(path, PathBuf::from("/tmp/demo.jpg"));
    }

    #[test]
    fn plain_html_removes_preview_shell_styles() {
        let rendered = render_markdown(RenderRequest {
            markdown: "# Title".to_string(),
            base_dir: None,
        });
        let plain = plain_html(&rendered.html);
        assert!(!plain.contains("markdown-body {"));
        assert!(plain.contains("<h1"));
    }

    #[test]
    fn html_export_writes_preview_document_not_source() {
        let dir = temp_path("markdown-editor-html-export-test", "dir");
        std::fs::create_dir_all(&dir).unwrap();
        let source_dir = dir.join("source assets");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("asset 1.png"), b"not-a-real-png").unwrap();
        let output_path = dir.join("demo.html");
        let rendered = render_markdown(RenderRequest {
            markdown: "# Title\n\n![Logo](asset%201.png)\n\n```js\nconsole.log(1)\n```".to_string(),
            base_dir: None,
        });

        let result = export_document(ExportRequest {
            format: ExportFormat::Html,
            rendered,
            base_dir: Some(source_dir),
            output_path: output_path.clone(),
            pandoc_path: None,
            chromium_path: None,
        })
        .unwrap();

        let html = std::fs::read_to_string(&result.path).unwrap();
        assert!(html.contains("class=\"markdown-body\""));
        assert!(html.contains("<h1"));
        assert!(html.contains("copy-code-button"));
        assert!(html.contains("demo.assets/asset%201.png"));
        assert!(
            result
                .path
                .with_file_name("demo.assets")
                .join("asset 1.png")
                .is_file()
        );
        assert!(!html.contains("# Title"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn docx_export_is_builtin_and_openxml() {
        use std::io::Read as _;

        let dir = temp_path("markdown-editor-docx-export-test", "dir");
        std::fs::create_dir_all(&dir).unwrap();
        let output_path = dir.join("demo.docx");
        let rendered = render_markdown(RenderRequest {
            markdown: "# 标题\n\n正文内容\n\n```rust\nfn main() {}\n```".to_string(),
            base_dir: None,
        });

        let result = export_document(ExportRequest {
            format: ExportFormat::Docx,
            rendered,
            base_dir: None,
            output_path: output_path.clone(),
            pandoc_path: None,
            chromium_path: None,
        })
        .unwrap();

        let file = std::fs::File::open(&result.path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut document)
            .unwrap();
        assert!(document.contains("Heading1"));
        assert!(document.contains("标题"));
        assert!(document.contains("fn main()"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
