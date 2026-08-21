//! Durable readiness state for later extracted-text indexing.

use rusqlite::params;
use serde::Serialize;

use super::{AttachmentExtractionState, ConversationStore, StorageError, now_ms};

/// Whether one attachment can participate in a future native text index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttachmentIndexingState {
    /// Text extraction has not reached a terminal result yet.
    WaitingForExtraction,
    /// Durable extracted text exists and is eligible for a later indexer.
    Indexable,
    /// The retained content type has no supported text extraction path.
    Unsupported,
    /// A failed extraction prevents indexing until the source can be processed.
    Blocked,
}

impl AttachmentIndexingState {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::WaitingForExtraction => "waiting_for_extraction",
            Self::Indexable => "indexable",
            Self::Unsupported => "unsupported",
            Self::Blocked => "blocked",
        }
    }

    /// Parses a trusted state constrained by the schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "waiting_for_extraction" => Ok(Self::WaitingForExtraction),
            "indexable" => Ok(Self::Indexable),
            "unsupported" => Ok(Self::Unsupported),
            "blocked" => Ok(Self::Blocked),
            _ => Err(StorageError::internal()),
        }
    }

    /// Derives indexing readiness from the durable extraction outcome.
    fn from_extraction(state: AttachmentExtractionState) -> Self {
        match state {
            AttachmentExtractionState::Pending => Self::WaitingForExtraction,
            AttachmentExtractionState::Ready => Self::Indexable,
            AttachmentExtractionState::Unsupported => Self::Unsupported,
            AttachmentExtractionState::Failed => Self::Blocked,
        }
    }
}

/// Path-free indexing metadata that deliberately makes no indexed-content claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredAttachmentIndexing {
    /// Current eligibility state for later background indexing.
    pub(crate) state: AttachmentIndexingState,
}

impl ConversationStore {
    /// Reconciles waiting indexing readiness after extraction completes in the worker.
    pub(super) fn reconcile_attachment_indexing(
        &self,
        attachment_id: &str,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        let (extraction, indexing) = connection.query_row(
            "SELECT attachment_extractions.state, attachment_text_indexing.state
             FROM attachment_extractions
             JOIN attachment_text_indexing
               ON attachment_text_indexing.attachment_id = attachment_extractions.attachment_id
             WHERE attachment_extractions.attachment_id = ?1",
            [attachment_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;
        let extraction = AttachmentExtractionState::from_database(&extraction)?;
        let indexing = AttachmentIndexingState::from_database(&indexing)?;
        if indexing != AttachmentIndexingState::WaitingForExtraction {
            return Ok(());
        }
        let next = AttachmentIndexingState::from_extraction(extraction);
        if next == indexing {
            return Ok(());
        }
        connection.execute(
            "UPDATE attachment_text_indexing SET state = ?1, updated_at_ms = ?2
             WHERE attachment_id = ?3 AND state = 'waiting_for_extraction'",
            params![next.as_str(), now_ms()?, attachment_id],
        )?;
        Ok(())
    }
}
