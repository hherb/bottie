//! Pure conversion of bounded page source into inert text and metadata.

use dom_query::Document;

use super::{WebFetchError, malformed_response};

/// Maximum UTF-8 bytes retained from one page's visible inert text.
pub(super) const MAX_WEB_FETCH_TEXT_BYTES: usize = 24 * 1_024;
/// Maximum UTF-8 bytes retained from one page title.
pub(super) const MAX_WEB_FETCH_TITLE_BYTES: usize = 512;
/// Maximum UTF-8 bytes retained from optional publication metadata.
pub(super) const MAX_WEB_FETCH_PUBLICATION_BYTES: usize = 256;

const HTML_MEDIA_TYPE: &str = "text/html";
const XHTML_MEDIA_TYPE: &str = "application/xhtml+xml";
const PLAIN_TEXT_MEDIA_TYPE: &str = "text/plain";
const NON_CONTENT_SELECTOR: &str =
    "script, style, template, noscript, svg, math, canvas, iframe, object, embed";
const TITLE_META_KEYS: &[&str] = &["og:title", "twitter:title"];
const PUBLICATION_META_KEYS: &[&str] = &[
    "article:published_time",
    "datepublished",
    "date",
    "pubdate",
    "publishdate",
    "publish-date",
    "parsely-pub-date",
    "dc.date",
    "dcterms.date",
];

/// Bounded content extracted from one already accepted UTF-8 response body.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ExtractedPage {
    /// Optional normalized document title.
    pub(super) title: Option<String>,
    /// Optional untrusted publication metadata copied from recognized page fields.
    pub(super) published_at: Option<String>,
    /// Visible text with markup and embedded executable content removed.
    pub(super) content: String,
}

/// Converts one accepted page source into bounded inert text and optional metadata.
pub(super) fn extract_page_content(
    media_type: &str,
    source: &str,
) -> Result<ExtractedPage, WebFetchError> {
    match media_type {
        HTML_MEDIA_TYPE | XHTML_MEDIA_TYPE => Ok(extract_html(source)),
        PLAIN_TEXT_MEDIA_TYPE => Ok(ExtractedPage {
            title: None,
            published_at: None,
            content: normalize_block_text(source, MAX_WEB_FETCH_TEXT_BYTES),
        }),
        _ => Err(malformed_response()),
    }
}

/// Parses tolerant HTML, reads metadata, removes non-content nodes, and formats body text.
fn extract_html(source: &str) -> ExtractedPage {
    let document = Document::from(source);
    let title =
        first_meta_content(&document, TITLE_META_KEYS, MAX_WEB_FETCH_TITLE_BYTES).or_else(|| {
            bounded_inline(
                &document.select_single("title").text(),
                MAX_WEB_FETCH_TITLE_BYTES,
            )
        });
    let published_at = first_meta_content(
        &document,
        PUBLICATION_META_KEYS,
        MAX_WEB_FETCH_PUBLICATION_BYTES,
    )
    .or_else(|| time_publication(&document));
    document.select(NON_CONTENT_SELECTOR).remove();
    let visible = document
        .body()
        .map_or_else(String::new, |body| body.formatted_text().to_string());
    ExtractedPage {
        title,
        published_at,
        content: normalize_block_text(&visible, MAX_WEB_FETCH_TEXT_BYTES),
    }
}

/// Returns the first recognized meta content value using key priority then document order.
fn first_meta_content(document: &Document, keys: &[&str], limit: usize) -> Option<String> {
    let metas = document.select("meta");
    for expected in keys {
        for meta in metas.nodes() {
            let key = meta
                .attr("property")
                .or_else(|| meta.attr("name"))
                .or_else(|| meta.attr("itemprop"));
            if key
                .as_deref()
                .is_some_and(|value| value.trim().eq_ignore_ascii_case(expected))
                && let Some(value) = meta.attr("content")
                && let Some(value) = bounded_inline(&value, limit)
            {
                return Some(value);
            }
        }
    }
    None
}

/// Reads a semantic publication time element when no recognized meta value exists.
fn time_publication(document: &Document) -> Option<String> {
    for time in document.select("time").nodes() {
        let is_publication = time
            .attr("itemprop")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("datePublished"));
        if !is_publication {
            continue;
        }
        if let Some(value) = time.attr("datetime").or_else(|| Some(time.text()))
            && let Some(value) = bounded_inline(&value, MAX_WEB_FETCH_PUBLICATION_BYTES)
        {
            return Some(value);
        }
    }
    None
}

/// Collapses inline whitespace, drops control characters, and applies a UTF-8 byte ceiling.
fn bounded_inline(value: &str, limit: usize) -> Option<String> {
    let sanitized = value
        .trim_start_matches('\u{feff}')
        .chars()
        .map(|character| {
            if character.is_control() && !character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let normalized = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded = truncate_utf8(normalized.trim(), limit)
        .trim_end()
        .to_owned();
    (!bounded.is_empty()).then_some(bounded)
}

/// Normalizes line-local whitespace while retaining at most one blank paragraph separator.
fn normalize_block_text(value: &str, limit: usize) -> String {
    let mut output = String::new();
    let mut saw_blank = false;
    for line in value.trim_start_matches('\u{feff}').lines() {
        let normalized = bounded_inline(line, limit).unwrap_or_default();
        if normalized.is_empty() {
            saw_blank = !output.is_empty();
            continue;
        }
        if !output.is_empty() {
            output.push('\n');
            if saw_blank {
                output.push('\n');
            }
        }
        output.push_str(&normalized);
        saw_blank = false;
        if output.len() >= limit {
            break;
        }
    }
    truncate_utf8(output.trim_end(), limit)
        .trim_end()
        .to_owned()
}

/// Truncates a string slice without splitting a UTF-8 code point.
fn truncate_utf8(value: &str, limit: usize) -> &str {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}
