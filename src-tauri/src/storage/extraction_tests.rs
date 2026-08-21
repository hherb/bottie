//! Native PDF extraction and migration contract tests.

use std::fs;

use lopdf::{Document, Object, Stream, content::Content, dictionary};

use super::{
    AttachmentExtractionFormat, AttachmentExtractionState, ConversationStore,
    extraction::MAX_PDF_PAGES, migrations::MIGRATION_10, tests::test_database_path,
};

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
    store
        .ingest_attachment(&source_path)
        .expect("PDF fixture should ingest")
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
        .execute("DELETE FROM schema_migrations WHERE version > 10", [])
        .expect("newer migration record should be removable");
    connection
        .pragma_update(None, "user_version", 10)
        .expect("fixture version should be set");
    drop(connection);
    drop(store);

    let upgraded = ConversationStore::initialize(path).expect("version ten store should upgrade");
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
