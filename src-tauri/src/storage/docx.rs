//! Bounded DOCX package validation and WordprocessingML text extraction.

use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};

use quick_xml::{Reader, XmlVersion, events::Event};
use zip::{ZipArchive, read::ZipFile, result::ZipError};

use super::extraction::MAX_EXTRACTED_TEXT_BYTES;

const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;
const CONTENT_TYPES_ENTRY: &str = "[Content_Types].xml";
const DOCUMENT_ENTRY: &str = "word/document.xml";
const DOCX_DOCUMENT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const MAX_DOCX_ARCHIVE_MEBIBYTES: u64 = 64;
const MAX_DOCX_ARCHIVE_UNCOMPRESSED_BYTES: u64 = MAX_DOCX_ARCHIVE_MEBIBYTES * BYTES_PER_MEBIBYTE;
const MAX_DOCX_CONTENT_TYPES_KIBIBYTES: u64 = 256;
const MAX_DOCX_CONTENT_TYPES_BYTES: u64 = MAX_DOCX_CONTENT_TYPES_KIBIBYTES * 1024;
const MAX_DOCX_DOCUMENT_XML_MEBIBYTES: u64 = 8;
const MAX_DOCX_DOCUMENT_XML_BYTES: u64 = MAX_DOCX_DOCUMENT_XML_MEBIBYTES * BYTES_PER_MEBIBYTE;
const MAX_DOCX_XML_EVENTS: usize = 500_000;

/// Stable MIME type assigned only after the retained package marker is verified.
pub(crate) const DOCX_MIME_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
/// Maximum package entry count accepted by synchronous DOCX extraction.
pub(crate) const MAX_DOCX_ARCHIVE_ENTRIES: usize = 1_024;
/// Maximum element nesting depth accepted by the bounded XML parser.
pub(crate) const MAX_DOCX_XML_DEPTH: usize = 128;
/// Stable failure for packages whose declared archive shape exceeds policy.
pub(crate) const ERROR_DOCX_ARCHIVE_LIMIT_EXCEEDED: &str = "docx_archive_limit_exceeded";
/// Stable failure for encrypted DOCX entries.
pub(crate) const ERROR_DOCX_ENCRYPTED: &str = "docx_encrypted";
/// Stable failure for malformed or non-DOCX ZIP packages selected as DOCX.
pub(crate) const ERROR_DOCX_INVALID: &str = "docx_invalid";
/// Stable failure for valid documents without visible main-document text.
pub(crate) const ERROR_DOCX_NO_TEXT: &str = "docx_no_text";
/// Stable failure for XML documents that exceed parser policy.
pub(crate) const ERROR_DOCX_XML_LIMIT_EXCEEDED: &str = "docx_xml_limit_exceeded";
/// Stable failure for malformed WordprocessingML.
pub(crate) const ERROR_DOCX_XML_INVALID: &str = "docx_xml_invalid";

/// Native-only DOCX text ready for durable storage.
pub(crate) struct ExtractedDocx {
    /// Extracted main-document text.
    pub(crate) text: String,
    /// Unicode scalar count of the extracted text.
    pub(crate) character_count: u64,
}

/// Returns whether a bounded ZIP package declares the DOCX main-document content type.
pub(crate) fn is_docx_package(path: &Path) -> bool {
    inspect_docx_package(path).is_ok()
}

/// Extracts visible text from the DOCX main document without unpacking files onto disk.
pub(crate) fn extract_docx(path: &Path) -> Result<ExtractedDocx, &'static str> {
    let mut archive = open_bounded_archive(path)?;
    ensure_docx_content_type(&mut archive)?;
    let document_xml =
        read_bounded_entry(&mut archive, DOCUMENT_ENTRY, MAX_DOCX_DOCUMENT_XML_BYTES)?;
    let text = extract_wordprocessing_text(&document_xml)?;
    if !text.chars().any(|character| !character.is_whitespace()) {
        return Err(ERROR_DOCX_NO_TEXT);
    }
    Ok(ExtractedDocx {
        character_count: text.chars().count() as u64,
        text,
    })
}

/// Validates the bounded archive and its DOCX package marker without reading document content.
fn inspect_docx_package(path: &Path) -> Result<(), &'static str> {
    let mut archive = open_bounded_archive(path)?;
    ensure_docx_content_type(&mut archive)
}

/// Opens and validates archive metadata before any member is decompressed.
fn open_bounded_archive(path: &Path) -> Result<ZipArchive<File>, &'static str> {
    let file = File::open(path).map_err(|_| ERROR_DOCX_INVALID)?;
    let mut archive = ZipArchive::new(file).map_err(zip_error_code)?;
    if archive.len() > MAX_DOCX_ARCHIVE_ENTRIES {
        return Err(ERROR_DOCX_ARCHIVE_LIMIT_EXCEEDED);
    }
    if archive.has_overlapping_files().map_err(zip_error_code)? {
        return Err(ERROR_DOCX_INVALID);
    }
    let mut uncompressed_bytes = 0_u64;
    let mut content_types_count = 0_usize;
    let mut document_count = 0_usize;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(zip_error_code)?;
        if entry.enclosed_name().is_none() || entry.is_symlink() {
            return Err(ERROR_DOCX_INVALID);
        }
        if entry.encrypted() {
            return Err(ERROR_DOCX_ENCRYPTED);
        }
        uncompressed_bytes = uncompressed_bytes
            .checked_add(entry.size())
            .ok_or(ERROR_DOCX_ARCHIVE_LIMIT_EXCEEDED)?;
        if uncompressed_bytes > MAX_DOCX_ARCHIVE_UNCOMPRESSED_BYTES {
            return Err(ERROR_DOCX_ARCHIVE_LIMIT_EXCEEDED);
        }
        content_types_count += usize::from(entry.name() == CONTENT_TYPES_ENTRY);
        document_count += usize::from(entry.name() == DOCUMENT_ENTRY);
    }
    if content_types_count != 1 || document_count != 1 {
        return Err(ERROR_DOCX_INVALID);
    }
    Ok(archive)
}

/// Confirms that the package maps the canonical main document to the DOCX content type.
fn ensure_docx_content_type<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<(), &'static str> {
    let content_types =
        read_bounded_entry(archive, CONTENT_TYPES_ENTRY, MAX_DOCX_CONTENT_TYPES_BYTES)?;
    let mut reader = Reader::from_reader(content_types.as_slice());
    let mut buffer = Vec::new();
    let mut depth = 0_usize;
    let mut event_count = 0_usize;
    let mut found_document_override = false;
    loop {
        event_count += 1;
        if event_count > MAX_DOCX_XML_EVENTS {
            return Err(ERROR_DOCX_XML_LIMIT_EXCEEDED);
        }
        match reader
            .read_event_into(&mut buffer)
            .map_err(|_| ERROR_DOCX_XML_INVALID)?
        {
            Event::Start(element) => {
                depth = checked_xml_depth(depth)?;
                if is_docx_override(&element, &reader)? {
                    found_document_override = true;
                }
            }
            Event::Empty(element) => {
                if is_docx_override(&element, &reader)? {
                    found_document_override = true;
                }
            }
            Event::End(_) => depth = depth.checked_sub(1).ok_or(ERROR_DOCX_XML_INVALID)?,
            Event::DocType(_) | Event::GeneralRef(_) => return Err(ERROR_DOCX_XML_INVALID),
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if depth == 0 && found_document_override {
        Ok(())
    } else {
        Err(ERROR_DOCX_INVALID)
    }
}

/// Matches the exact main-document override without relying on namespace prefixes.
fn is_docx_override(
    element: &quick_xml::events::BytesStart<'_>,
    reader: &Reader<&[u8]>,
) -> Result<bool, &'static str> {
    let is_override = local_name(element.name().as_ref()) == b"Override";
    let mut part_name = None;
    let mut content_type = None;
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ERROR_DOCX_XML_INVALID)?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|_| ERROR_DOCX_XML_INVALID)?;
        if is_override {
            match local_name(attribute.key.as_ref()) {
                b"PartName" => part_name = Some(value.into_owned()),
                b"ContentType" => content_type = Some(value.into_owned()),
                _ => {}
            }
        }
    }
    Ok(is_override
        && part_name.as_deref() == Some("/word/document.xml")
        && content_type.as_deref() == Some(DOCX_DOCUMENT_CONTENT_TYPE))
}

/// Reads one required archive member with declared and actual decompression ceilings.
fn read_bounded_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, &'static str> {
    let entry = archive.by_name(name).map_err(zip_error_code)?;
    if entry.size() > limit {
        return Err(ERROR_DOCX_XML_LIMIT_EXCEEDED);
    }
    read_entry_bytes(entry, limit)
}

/// Reads at most one byte beyond the declared XML limit to catch forged metadata.
fn read_entry_bytes<R: Read>(entry: ZipFile<'_, R>, limit: u64) -> Result<Vec<u8>, &'static str> {
    if entry.encrypted() {
        return Err(ERROR_DOCX_ENCRYPTED);
    }
    let mut bytes = Vec::new();
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ERROR_DOCX_INVALID)?;
    if bytes.len() as u64 > limit {
        return Err(ERROR_DOCX_XML_LIMIT_EXCEEDED);
    }
    Ok(bytes)
}

/// Extracts Word text, paragraph boundaries, tabs, and explicit line breaks.
fn extract_wordprocessing_text(xml: &[u8]) -> Result<String, &'static str> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut text = String::new();
    let mut depth = 0_usize;
    let mut event_count = 0_usize;
    let mut text_depth = None;
    loop {
        event_count += 1;
        if event_count > MAX_DOCX_XML_EVENTS {
            return Err(ERROR_DOCX_XML_LIMIT_EXCEEDED);
        }
        match reader
            .read_event_into(&mut buffer)
            .map_err(|_| ERROR_DOCX_XML_INVALID)?
        {
            Event::Start(element) => {
                depth = checked_xml_depth(depth)?;
                match local_name(element.name().as_ref()) {
                    b"t" => text_depth = Some(depth),
                    b"tab" => push_bounded(&mut text, "\t")?,
                    b"br" | b"cr" => push_bounded(&mut text, "\n")?,
                    _ => {}
                }
            }
            Event::Empty(element) => match local_name(element.name().as_ref()) {
                b"tab" => push_bounded(&mut text, "\t")?,
                b"br" | b"cr" => push_bounded(&mut text, "\n")?,
                _ => {}
            },
            Event::Text(value) if text_depth.is_some() => {
                let decoded = value.decode().map_err(|_| ERROR_DOCX_XML_INVALID)?;
                push_bounded(&mut text, &decoded)?;
            }
            Event::CData(value) if text_depth.is_some() => {
                let decoded = value.decode().map_err(|_| ERROR_DOCX_XML_INVALID)?;
                push_bounded(&mut text, &decoded)?;
            }
            Event::GeneralRef(reference) if text_depth.is_some() => {
                let resolved = resolve_reference(&reference)?;
                push_bounded(&mut text, &resolved)?;
            }
            Event::End(element) => {
                match local_name(element.name().as_ref()) {
                    b"t" => text_depth = None,
                    b"p" | b"tr" => push_separator(&mut text, '\n')?,
                    b"tc" => push_separator(&mut text, '\t')?,
                    _ => {}
                }
                depth = depth.checked_sub(1).ok_or(ERROR_DOCX_XML_INVALID)?;
            }
            Event::DocType(_) => return Err(ERROR_DOCX_XML_INVALID),
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if depth != 0 {
        return Err(ERROR_DOCX_XML_INVALID);
    }
    while matches!(text.as_bytes().last(), Some(b'\n' | b'\t')) {
        text.pop();
    }
    Ok(text)
}

/// Resolves predefined and numeric XML references while rejecting document-defined entities.
fn resolve_reference(reference: &quick_xml::events::BytesRef<'_>) -> Result<String, &'static str> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|_| ERROR_DOCX_XML_INVALID)?
    {
        return Ok(character.to_string());
    }
    match reference
        .decode()
        .map_err(|_| ERROR_DOCX_XML_INVALID)?
        .as_ref()
    {
        "amp" => Ok("&".into()),
        "lt" => Ok("<".into()),
        "gt" => Ok(">".into()),
        "apos" => Ok("'".into()),
        "quot" => Ok("\"".into()),
        _ => Err(ERROR_DOCX_XML_INVALID),
    }
}

/// Advances XML depth while enforcing the native parser ceiling.
fn checked_xml_depth(depth: usize) -> Result<usize, &'static str> {
    let depth = depth.checked_add(1).ok_or(ERROR_DOCX_XML_LIMIT_EXCEEDED)?;
    if depth > MAX_DOCX_XML_DEPTH {
        return Err(ERROR_DOCX_XML_LIMIT_EXCEEDED);
    }
    Ok(depth)
}

/// Appends extracted text without allocating beyond the shared retained-text ceiling.
fn push_bounded(target: &mut String, value: &str) -> Result<(), &'static str> {
    if target.len().saturating_add(value.len()) > MAX_EXTRACTED_TEXT_BYTES {
        return Err(super::extraction::ERROR_CONTENT_TOO_LARGE);
    }
    target.push_str(value);
    Ok(())
}

/// Adds a structural separator without repeating it at the current boundary.
fn push_separator(target: &mut String, separator: char) -> Result<(), &'static str> {
    if !target.is_empty() && !target.ends_with(separator) {
        let mut encoded = [0_u8; 4];
        push_bounded(target, separator.encode_utf8(&mut encoded))?;
    }
    Ok(())
}

/// Returns the namespace-independent local component of one XML name.
fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

/// Maps ZIP failures to stable path-free DOCX categories.
fn zip_error_code(error: ZipError) -> &'static str {
    match error {
        ZipError::UnsupportedArchive(message) if message == ZipError::PASSWORD_REQUIRED => {
            ERROR_DOCX_ENCRYPTED
        }
        _ => ERROR_DOCX_INVALID,
    }
}
