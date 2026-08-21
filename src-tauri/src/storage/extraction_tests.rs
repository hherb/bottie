//! Native PDF and DOCX extraction and migration contract tests.

use std::{fs, io::Write};

use lopdf::{Document, Object, Stream, content::Content, dictionary};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::{
    AttachmentExtractionFormat, AttachmentExtractionState, ConversationStore,
    docx::{MAX_DOCX_ARCHIVE_ENTRIES, MAX_DOCX_XML_DEPTH},
    extraction::MAX_PDF_PAGES,
    migrations::{MIGRATION_10, MIGRATION_11},
    tests::{completed_ingestion, process_pending_attachments, test_database_path},
};

const DOCX_MIME_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const DOCX_DOCUMENT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Builds a small valid PDF fixture with one simple text stream per page.
fn pdf_fixture(page_texts: &[&str]) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let mut page_ids = Vec::with_capacity(page_texts.len());
    for text in page_texts {
        let content = Content {
            operations: vec![
                lopdf::content::Operation::new("BT", vec![]),
                lopdf::content::Operation::new("Tf", vec!["F1".into(), 12.into()]),
                lopdf::content::Operation::new("Td", vec![72.into(), 720.into()]),
                lopdf::content::Operation::new("Tj", vec![Object::string_literal(*text)]),
                lopdf::content::Operation::new("ET", vec![]),
            ],
        }
        .encode()
        .expect("PDF content should encode");
        let content_id = document.add_object(Stream::new(dictionary! {}, content));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        page_ids.push(page_id);
    }
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => page_ids.len() as i64,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("PDF fixture should serialize");
    bytes
}

/// Writes and ingests one PDF fixture beside the isolated test store.
fn ingest_pdf(
    store: &ConversationStore,
    name: &str,
    page_texts: &[&str],
) -> super::IngestedAttachment {
    let source_path = store.path.with_file_name(name);
    fs::write(&source_path, pdf_fixture(page_texts)).expect("PDF fixture should be written");
    let ingested = store
        .ingest_attachment(&source_path)
        .expect("PDF fixture should ingest");
    completed_ingestion(store, ingested)
}

/// Builds one minimal DOCX package with caller-controlled main-document XML and extra entries.
fn docx_fixture(document_xml: &str, extra_entry_count: usize) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut archive = ZipWriter::new(&mut bytes);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        archive
            .start_file("[Content_Types].xml", options)
            .expect("content types entry should start");
        write!(
            archive,
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
                  <Override PartName="/word/document.xml" ContentType="{DOCX_DOCUMENT_CONTENT_TYPE}"/>
                </Types>"#
        )
        .expect("content types should be written");
        archive
            .start_file("word/document.xml", options)
            .expect("document entry should start");
        archive
            .write_all(document_xml.as_bytes())
            .expect("document XML should be written");
        for index in 0..extra_entry_count {
            archive
                .start_file(format!("custom/entry-{index}.xml"), options)
                .expect("extra entry should start");
            archive
                .write_all(b"<empty/>")
                .expect("extra entry should be written");
        }
        archive.finish().expect("DOCX fixture should finish");
    }
    bytes.into_inner()
}

/// Writes and ingests one DOCX package beside the isolated test store.
fn ingest_docx(
    store: &ConversationStore,
    name: &str,
    document_xml: &str,
    extra_entry_count: usize,
) -> super::IngestedAttachment {
    let source_path = store.path.with_file_name(name);
    fs::write(&source_path, docx_fixture(document_xml, extra_entry_count))
        .expect("DOCX fixture should be written");
    let ingested = store
        .ingest_attachment(&source_path)
        .expect("DOCX fixture should ingest");
    completed_ingestion(store, ingested)
}

#[test]
fn extracts_bounded_docx_text_with_content_based_mime() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let docx = ingest_docx(
        &store,
        "field-notes.package",
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p><w:r><w:t>First &amp; second</w:t><w:tab/><w:t>column</w:t></w:r></w:p>
                <w:p><w:r><w:t>Next line</w:t><w:br/><w:t>after break</w:t></w:r></w:p>
              </w:body>
            </w:document>"#,
        0,
    );
    let stored = store
        .stored_attachment_for_test(&docx.id)
        .expect("DOCX attachment should load")
        .expect("DOCX attachment should exist");

    assert_eq!(docx.mime_type, DOCX_MIME_TYPE);
    assert_eq!(stored.extraction.state, AttachmentExtractionState::Ready);
    assert_eq!(
        stored.extraction.format,
        Some(AttachmentExtractionFormat::Docx)
    );
    assert_eq!(stored.extraction.page_count, None);
    assert_eq!(
        store
            .extracted_text_for_test(&docx.id)
            .expect("DOCX extraction should load")
            .as_deref(),
        Some("First & second\tcolumn\nNext line\nafter break")
    );
}

#[test]
fn upgrades_version_eleven_docx_state_and_extracts_retained_content() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let docx = ingest_docx(
        &store,
        "migrated.docx",
        concat!(
            r#"<w:document xmlns:w="urn:test"><w:body><w:p><w:r>"#,
            r#"<w:t>Existing DOCX</w:t></w:r></w:p></w:body></w:document>"#,
        ),
        0,
    );
    let connection = store.open().expect("database should open");
    connection
        .execute_batch("DROP TABLE attachment_extractions;")
        .expect("version twelve extraction table should be removable");
    connection
        .execute_batch(MIGRATION_10)
        .expect("version ten extraction table should be restorable");
    connection
        .execute_batch(MIGRATION_11)
        .expect("version eleven extraction table should be restorable");
    connection
        .execute(
            "UPDATE attachments SET mime_type = 'application/zip' WHERE id = ?1",
            [&docx.id],
        )
        .expect("older DOCX MIME should be restored");
    connection
        .execute(
            "UPDATE attachment_extractions SET state = 'unsupported' WHERE attachment_id = ?1",
            [&docx.id],
        )
        .expect("version eleven DOCX should be unsupported");
    connection
        .execute_batch("DROP TABLE attachment_image_normalizations;")
        .expect("version thirteen table should be removable");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 11", [])
        .expect("newer migration record should be removable");
    connection
        .pragma_update(None, "user_version", 11)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded =
        ConversationStore::initialize(path).expect("version eleven store should upgrade");
    process_pending_attachments(&upgraded);
    let stored = upgraded
        .stored_attachment_for_test(&docx.id)
        .expect("DOCX attachment should load")
        .expect("DOCX attachment should remain present");

    assert_eq!(stored.mime_type, DOCX_MIME_TYPE);
    assert_eq!(stored.extraction.state, AttachmentExtractionState::Ready);
    assert_eq!(
        stored.extraction.format,
        Some(AttachmentExtractionFormat::Docx)
    );
    assert_eq!(
        upgraded
            .extracted_text_for_test(&docx.id)
            .expect("migrated DOCX text should load")
            .as_deref(),
        Some("Existing DOCX")
    );
}

#[test]
fn rejects_docx_archive_and_xml_depth_limits_without_partial_text() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let archive_limited = ingest_docx(
        &store,
        "too-many-entries.docx",
        r#"<w:document xmlns:w="urn:test"><w:body><w:p><w:r><w:t>Hidden</w:t></w:r></w:p></w:body></w:document>"#,
        MAX_DOCX_ARCHIVE_ENTRIES,
    );
    let nested_start = "<w:r>".repeat(MAX_DOCX_XML_DEPTH + 1);
    let nested_end = "</w:r>".repeat(MAX_DOCX_XML_DEPTH + 1);
    let depth_limited = ingest_docx(
        &store,
        "too-deep.docx",
        &format!(
            concat!(
                r#"<w:document xmlns:w="urn:test"><w:body><w:p>{nested_start}"#,
                r#"<w:t>Hidden</w:t>{nested_end}</w:p></w:body></w:document>"#,
            ),
            nested_start = nested_start,
            nested_end = nested_end
        ),
        0,
    );
    let archive_stored = store
        .stored_attachment_for_test(&archive_limited.id)
        .expect("archive-limited DOCX should load")
        .expect("archive-limited DOCX should exist");
    let depth_stored = store
        .stored_attachment_for_test(&depth_limited.id)
        .expect("depth-limited DOCX should load")
        .expect("depth-limited DOCX should exist");

    assert_eq!(
        archive_stored.extraction.error_code.as_deref(),
        Some("docx_archive_limit_exceeded")
    );
    assert_eq!(
        depth_stored.extraction.error_code.as_deref(),
        Some("docx_xml_limit_exceeded")
    );
    assert_eq!(
        store
            .extracted_text_for_test(&archive_limited.id)
            .expect("failed archive extraction should remain queryable"),
        None
    );
    assert_eq!(
        store
            .extracted_text_for_test(&depth_limited.id)
            .expect("failed XML extraction should remain queryable"),
        None
    );
}

#[test]
fn records_path_free_failures_for_text_free_and_malformed_docx() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let text_free = ingest_docx(
        &store,
        "empty.docx",
        r#"<w:document xmlns:w="urn:test"><w:body><w:p/></w:body></w:document>"#,
        0,
    );
    let malformed = ingest_docx(
        &store,
        "malformed.docx",
        r#"<w:document xmlns:w="urn:test"><w:body><w:p><w:t>broken</w:body></w:document>"#,
        0,
    );
    let text_free_stored = store
        .stored_attachment_for_test(&text_free.id)
        .expect("text-free DOCX should load")
        .expect("text-free DOCX should exist");
    let malformed_stored = store
        .stored_attachment_for_test(&malformed.id)
        .expect("malformed DOCX should load")
        .expect("malformed DOCX should exist");

    assert_eq!(
        text_free_stored.extraction.error_code.as_deref(),
        Some("docx_no_text")
    );
    assert_eq!(
        malformed_stored.extraction.error_code.as_deref(),
        Some("docx_xml_invalid")
    );
    assert_eq!(text_free_stored.extraction.format, None);
    assert_eq!(malformed_stored.extraction.format, None);
}

#[test]
fn extracts_bounded_pdf_text_with_page_aware_metadata() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let pdf = ingest_pdf(&store, "field-guide.pdf", &["First page", "Second page"]);
    let stored = store
        .stored_attachment_for_test(&pdf.id)
        .expect("PDF attachment should load")
        .expect("PDF attachment should exist");
    let text = store
        .extracted_text_for_test(&pdf.id)
        .expect("PDF extraction should load")
        .expect("PDF text should be retained");

    assert_eq!(stored.extraction.state, AttachmentExtractionState::Ready);
    assert_eq!(
        stored.extraction.format,
        Some(AttachmentExtractionFormat::Pdf)
    );
    assert_eq!(stored.extraction.page_count, Some(2));
    assert!(text.contains("First page"));
    assert!(text.contains("Second page"));
}

#[test]
fn upgrades_version_ten_pdf_state_and_extracts_retained_content() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let pdf = ingest_pdf(&store, "migrated.pdf", &["Existing PDF"]);
    let connection = store.open().expect("database should open");
    connection
        .execute_batch("DROP TABLE attachment_extractions;")
        .expect("version eleven extraction table should be removable");
    connection
        .execute_batch(MIGRATION_10)
        .expect("version ten extraction table should be restorable");
    connection
        .execute(
            "UPDATE attachment_extractions SET state = 'unsupported' WHERE attachment_id = ?1",
            [&pdf.id],
        )
        .expect("version ten PDF should be unsupported");
    connection
        .execute_batch("DROP TABLE attachment_image_normalizations;")
        .expect("version thirteen table should be removable");
    connection
        .execute("DELETE FROM schema_migrations WHERE version > 10", [])
        .expect("newer migration record should be removable");
    connection
        .pragma_update(None, "user_version", 10)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version ten store should upgrade");
    process_pending_attachments(&upgraded);
    let stored = upgraded
        .stored_attachment_for_test(&pdf.id)
        .expect("PDF attachment should load")
        .expect("PDF attachment should remain present");

    assert_eq!(stored.extraction.state, AttachmentExtractionState::Ready);
    assert_eq!(
        stored.extraction.format,
        Some(AttachmentExtractionFormat::Pdf)
    );
    assert_eq!(stored.extraction.page_count, Some(1));
    assert_eq!(
        upgraded
            .extracted_text_for_test(&pdf.id)
            .expect("migrated PDF text should load")
            .as_deref(),
        Some("Existing PDF")
    );
}

#[test]
fn rejects_pdfs_over_the_page_limit_without_retaining_partial_text() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let page_texts = vec!["bounded"; MAX_PDF_PAGES + 1];
    let pdf = ingest_pdf(&store, "too-many-pages.pdf", &page_texts);
    let stored = store
        .stored_attachment_for_test(&pdf.id)
        .expect("PDF attachment should load")
        .expect("PDF attachment should exist");

    assert_eq!(stored.extraction.state, AttachmentExtractionState::Failed);
    assert_eq!(stored.extraction.page_count, None);
    assert_eq!(
        stored.extraction.error_code.as_deref(),
        Some("pdf_page_limit_exceeded")
    );
    assert_eq!(
        store
            .extracted_text_for_test(&pdf.id)
            .expect("failed PDF extraction should remain queryable"),
        None
    );
}

#[test]
fn records_path_free_failures_for_empty_and_malformed_pdfs() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let empty = ingest_pdf(&store, "scan.pdf", &[""]);
    let malformed_path = store.path.with_file_name("malformed.pdf");
    fs::write(&malformed_path, b"%PDF-1.7\nnot a valid document")
        .expect("malformed PDF should be written");
    let malformed = store
        .ingest_attachment(&malformed_path)
        .expect("malformed PDF should retain metadata");
    process_pending_attachments(&store);
    let empty_stored = store
        .stored_attachment_for_test(&empty.id)
        .expect("empty PDF should load")
        .expect("empty PDF should exist");
    let malformed_stored = store
        .stored_attachment_for_test(&malformed.id)
        .expect("malformed PDF should load")
        .expect("malformed PDF should exist");

    assert_eq!(
        empty_stored.extraction.error_code.as_deref(),
        Some("pdf_no_text")
    );
    assert_eq!(malformed.mime_type, "application/pdf");
    assert_eq!(
        malformed_stored.extraction.error_code.as_deref(),
        Some("pdf_invalid")
    );
    assert_eq!(empty_stored.extraction.page_count, None);
    assert_eq!(malformed_stored.extraction.page_count, None);
}
