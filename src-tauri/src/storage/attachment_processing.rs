//! Durable coordination for pending attachment extraction and normalization work.

use rusqlite::OptionalExtension;

use super::{ConversationStore, StorageError, StoredAttachment, attachments::stored_attachment};

impl ConversationStore {
    /// Completes the oldest retained attachment with pending native work.
    ///
    /// The caller controls scheduling so ingestion and application initialization never run
    /// document parsing or image codecs inline. Returning one completed path-free record makes
    /// it possible for the native lifecycle worker to publish a narrow presentation update.
    pub(crate) fn process_next_pending_attachment(
        &self,
    ) -> Result<Option<StoredAttachment>, StorageError> {
        let connection = self.open()?;
        let attachment_id = connection
            .query_row(
                "SELECT attachments.id
                 FROM attachments
                 JOIN attachment_extractions
                   ON attachment_extractions.attachment_id = attachments.id
                 JOIN attachment_image_normalizations
                   ON attachment_image_normalizations.attachment_id = attachments.id
                 WHERE attachment_extractions.state = 'pending'
                    OR attachment_image_normalizations.state = 'pending'
                 ORDER BY MIN(
                     attachment_extractions.updated_at_ms,
                     attachment_image_normalizations.updated_at_ms
                 ), attachments.id
                 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        drop(connection);
        let Some(attachment_id) = attachment_id else {
            return Ok(None);
        };

        self.process_attachment_extraction(&attachment_id)?;
        self.process_image_normalization(&attachment_id)?;
        stored_attachment(&self.open()?, &attachment_id)
    }
}
