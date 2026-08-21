//! SQLite migration for the native derived FTS5 lexical-memory index.

/// Adds the derived FTS5 index for final message text and ready extracted attachment text.
pub(super) const MIGRATION_16: &str = r#"
DROP TRIGGER IF EXISTS memory_message_blocks_after_insert;
DROP TRIGGER IF EXISTS memory_message_blocks_after_update;
DROP TRIGGER IF EXISTS memory_message_blocks_after_delete;
DROP TRIGGER IF EXISTS memory_messages_after_state_update;
DROP TRIGGER IF EXISTS memory_messages_after_delete;
DROP TRIGGER IF EXISTS memory_extractions_after_insert;
DROP TRIGGER IF EXISTS memory_extractions_after_update;
DROP TRIGGER IF EXISTS memory_extractions_after_delete;
DROP TABLE IF EXISTS memory_lexical_index;

CREATE VIRTUAL TABLE memory_lexical_index USING fts5(
    source_kind UNINDEXED,
    source_id UNINDEXED,
    profile_id UNINDEXED,
    created_at_ms UNINDEXED,
    text_content,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO memory_lexical_index
    (source_kind, source_id, profile_id, created_at_ms, text_content)
SELECT 'message', messages.id, conversations.profile_id, messages.created_at_ms,
       (
           SELECT group_concat(ordered_blocks.text_content, '')
           FROM (
               SELECT text_content FROM message_blocks
               WHERE message_id = messages.id AND block_type = 'text'
               ORDER BY ordinal
           ) AS ordered_blocks
       )
FROM messages
JOIN conversations ON conversations.id = messages.conversation_id
WHERE messages.state = 'final'
  AND EXISTS (
      SELECT 1 FROM message_blocks
      WHERE message_id = messages.id AND block_type = 'text' AND length(trim(text_content)) > 0
  );

INSERT INTO memory_lexical_index
    (source_kind, source_id, profile_id, created_at_ms, text_content)
SELECT 'attachment', attachments.id, 'local', attachments.created_at_ms,
       attachment_extractions.text_content
FROM attachments
JOIN attachment_extractions ON attachment_extractions.attachment_id = attachments.id
WHERE attachment_extractions.state = 'ready'
  AND length(trim(attachment_extractions.text_content)) > 0;

CREATE TRIGGER memory_message_blocks_after_insert
AFTER INSERT ON message_blocks
WHEN NEW.block_type = 'text'
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'message' AND source_id = NEW.message_id;
    INSERT INTO memory_lexical_index
        (source_kind, source_id, profile_id, created_at_ms, text_content)
    SELECT 'message', messages.id, conversations.profile_id, messages.created_at_ms,
           (
               SELECT group_concat(ordered_blocks.text_content, '')
               FROM (
                   SELECT text_content FROM message_blocks
                   WHERE message_id = messages.id AND block_type = 'text'
                   ORDER BY ordinal
               ) AS ordered_blocks
           )
    FROM messages
    JOIN conversations ON conversations.id = messages.conversation_id
    WHERE messages.id = NEW.message_id AND messages.state = 'final'
      AND EXISTS (
          SELECT 1 FROM message_blocks
          WHERE message_id = messages.id AND block_type = 'text'
            AND length(trim(text_content)) > 0
      );
END;

CREATE TRIGGER memory_message_blocks_after_update
AFTER UPDATE OF message_id, ordinal, block_type, text_content ON message_blocks
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'message' AND source_id IN (OLD.message_id, NEW.message_id);
    INSERT INTO memory_lexical_index
        (source_kind, source_id, profile_id, created_at_ms, text_content)
    SELECT 'message', messages.id, conversations.profile_id, messages.created_at_ms,
           (
               SELECT group_concat(ordered_blocks.text_content, '')
               FROM (
                   SELECT text_content FROM message_blocks
                   WHERE message_id = messages.id AND block_type = 'text'
                   ORDER BY ordinal
               ) AS ordered_blocks
           )
    FROM messages
    JOIN conversations ON conversations.id = messages.conversation_id
    WHERE messages.id IN (OLD.message_id, NEW.message_id) AND messages.state = 'final'
      AND EXISTS (
          SELECT 1 FROM message_blocks
          WHERE message_id = messages.id AND block_type = 'text'
            AND length(trim(text_content)) > 0
      );
END;

CREATE TRIGGER memory_message_blocks_after_delete
AFTER DELETE ON message_blocks
WHEN OLD.block_type = 'text'
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'message' AND source_id = OLD.message_id;
    INSERT INTO memory_lexical_index
        (source_kind, source_id, profile_id, created_at_ms, text_content)
    SELECT 'message', messages.id, conversations.profile_id, messages.created_at_ms,
           (
               SELECT group_concat(ordered_blocks.text_content, '')
               FROM (
                   SELECT text_content FROM message_blocks
                   WHERE message_id = messages.id AND block_type = 'text'
                   ORDER BY ordinal
               ) AS ordered_blocks
           )
    FROM messages
    JOIN conversations ON conversations.id = messages.conversation_id
    WHERE messages.id = OLD.message_id AND messages.state = 'final'
      AND EXISTS (
          SELECT 1 FROM message_blocks
          WHERE message_id = messages.id AND block_type = 'text'
            AND length(trim(text_content)) > 0
      );
END;

CREATE TRIGGER memory_messages_after_state_update
AFTER UPDATE OF state ON messages
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'message' AND source_id = NEW.id;
    INSERT INTO memory_lexical_index
        (source_kind, source_id, profile_id, created_at_ms, text_content)
    SELECT 'message', messages.id, conversations.profile_id, messages.created_at_ms,
           (
               SELECT group_concat(ordered_blocks.text_content, '')
               FROM (
                   SELECT text_content FROM message_blocks
                   WHERE message_id = messages.id AND block_type = 'text'
                   ORDER BY ordinal
               ) AS ordered_blocks
           )
    FROM messages
    JOIN conversations ON conversations.id = messages.conversation_id
    WHERE messages.id = NEW.id AND messages.state = 'final'
      AND EXISTS (
          SELECT 1 FROM message_blocks
          WHERE message_id = messages.id AND block_type = 'text'
            AND length(trim(text_content)) > 0
      );
END;

CREATE TRIGGER memory_messages_after_delete
AFTER DELETE ON messages
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'message' AND source_id = OLD.id;
END;

CREATE TRIGGER memory_extractions_after_insert
AFTER INSERT ON attachment_extractions
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'attachment' AND source_id = NEW.attachment_id;
    INSERT INTO memory_lexical_index
        (source_kind, source_id, profile_id, created_at_ms, text_content)
    SELECT 'attachment', attachments.id, 'local', attachments.created_at_ms,
           attachment_extractions.text_content
    FROM attachments
    JOIN attachment_extractions ON attachment_extractions.attachment_id = attachments.id
    WHERE attachments.id = NEW.attachment_id AND attachment_extractions.state = 'ready'
      AND length(trim(attachment_extractions.text_content)) > 0;
END;

CREATE TRIGGER memory_extractions_after_update
AFTER UPDATE OF attachment_id, state, text_content ON attachment_extractions
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'attachment'
      AND source_id IN (OLD.attachment_id, NEW.attachment_id);
    INSERT INTO memory_lexical_index
        (source_kind, source_id, profile_id, created_at_ms, text_content)
    SELECT 'attachment', attachments.id, 'local', attachments.created_at_ms,
           attachment_extractions.text_content
    FROM attachments
    JOIN attachment_extractions ON attachment_extractions.attachment_id = attachments.id
    WHERE attachments.id IN (OLD.attachment_id, NEW.attachment_id)
      AND attachment_extractions.state = 'ready'
      AND length(trim(attachment_extractions.text_content)) > 0;
END;

CREATE TRIGGER memory_extractions_after_delete
AFTER DELETE ON attachment_extractions
BEGIN
    DELETE FROM memory_lexical_index
    WHERE source_kind = 'attachment' AND source_id = OLD.attachment_id;
END;
"#;
