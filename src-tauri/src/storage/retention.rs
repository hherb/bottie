//! Durable opt-in time-based deletion for conversations already in Trash.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{ConversationStore, DEFAULT_PROFILE_ID, StorageError, now_ms};

const MILLISECONDS_PER_DAY: i64 = 24 * 60 * 60 * 1_000;
const THIRTY_DAYS: i64 = 30;
const NINETY_DAYS: i64 = 90;
const ONE_YEAR_DAYS: i64 = 365;

/// User-selectable duration for retaining conversations after they enter Trash.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConversationRetentionPeriod {
    /// Keep Trash until the user invokes explicit permanent forget.
    #[default]
    Forever,
    /// Forget conversations after thirty complete days in Trash.
    ThirtyDays,
    /// Forget conversations after ninety complete days in Trash.
    NinetyDays,
    /// Forget conversations after 365 complete days in Trash.
    OneYear,
}

impl ConversationRetentionPeriod {
    /// Returns the stored period name, with manual retention represented by no row.
    fn database_value(self) -> Option<&'static str> {
        match self {
            Self::Forever => None,
            Self::ThirtyDays => Some("thirty_days"),
            Self::NinetyDays => Some("ninety_days"),
            Self::OneYear => Some("one_year"),
        }
    }

    /// Maps one trusted stored period into the typed retention contract.
    fn from_database(value: Option<&str>) -> Result<Self, StorageError> {
        match value {
            None => Ok(Self::Forever),
            Some("thirty_days") => Ok(Self::ThirtyDays),
            Some("ninety_days") => Ok(Self::NinetyDays),
            Some("one_year") => Ok(Self::OneYear),
            Some(_) => Err(StorageError::internal()),
        }
    }

    /// Returns the number of complete Trash days before automatic forget.
    fn days(self) -> Option<i64> {
        match self {
            Self::Forever => None,
            Self::ThirtyDays => Some(THIRTY_DAYS),
            Self::NinetyDays => Some(NINETY_DAYS),
            Self::OneYear => Some(ONE_YEAR_DAYS),
        }
    }
}

/// Path-free durable Trash retention state returned to Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationRetentionPolicy {
    /// Current opt-in retention period.
    pub(crate) period: ConversationRetentionPeriod,
}

/// Result of one healthy-startup retention pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConversationRetentionOutcome {
    /// Number of expired Trash conversations permanently removed from the live store.
    pub(crate) forgotten_conversations: usize,
}

impl ConversationStore {
    /// Returns the built-in local profile's durable Trash retention policy.
    pub(crate) fn conversation_retention_policy(
        &self,
    ) -> Result<ConversationRetentionPolicy, StorageError> {
        let connection = self.open()?;
        Ok(ConversationRetentionPolicy {
            period: load_period(&connection)?,
        })
    }

    /// Persists one bounded Trash retention period without deleting content immediately.
    pub(crate) fn set_conversation_retention_period(
        &self,
        period: ConversationRetentionPeriod,
    ) -> Result<ConversationRetentionPolicy, StorageError> {
        let connection = self.open()?;
        match period.database_value() {
            Some(value) => {
                connection.execute(
                    "INSERT INTO conversation_retention_policies (profile_id, period, updated_at_ms)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(profile_id) DO UPDATE
                     SET period = excluded.period, updated_at_ms = excluded.updated_at_ms",
                    params![DEFAULT_PROFILE_ID, value, now_ms()?],
                )?;
            }
            None => {
                connection.execute(
                    "DELETE FROM conversation_retention_policies WHERE profile_id = ?1",
                    [DEFAULT_PROFILE_ID],
                )?;
            }
        }
        Ok(ConversationRetentionPolicy { period })
    }

    /// Applies the saved policy against the current time during one healthy app startup.
    pub(crate) fn apply_conversation_retention(
        &self,
    ) -> Result<ConversationRetentionOutcome, StorageError> {
        self.apply_conversation_retention_at(now_ms()?)
    }

    /// Applies the saved policy against a supplied clock for exact policy testing.
    pub(crate) fn apply_conversation_retention_at(
        &self,
        timestamp_ms: i64,
    ) -> Result<ConversationRetentionOutcome, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let Some(days) = load_period(&transaction)?.days() else {
            return Ok(ConversationRetentionOutcome::default());
        };
        let retention_ms = days
            .checked_mul(MILLISECONDS_PER_DAY)
            .ok_or_else(StorageError::internal)?;
        let cutoff_ms = timestamp_ms
            .checked_sub(retention_ms)
            .ok_or_else(StorageError::internal)?;
        let forgotten_conversations = transaction.execute(
            "DELETE FROM conversations
             WHERE profile_id = ?1
               AND deleted_at_ms IS NOT NULL
               AND deleted_at_ms <= ?2
               AND NOT EXISTS (
                   SELECT 1 FROM provider_runs
                   WHERE provider_runs.conversation_id = conversations.id
                     AND provider_runs.state = 'running'
               )",
            params![DEFAULT_PROFILE_ID, cutoff_ms],
        )?;
        transaction.commit()?;
        Ok(ConversationRetentionOutcome {
            forgotten_conversations,
        })
    }
}

/// Loads the optional database period for the built-in local profile.
fn load_period(
    connection: &rusqlite::Connection,
) -> Result<ConversationRetentionPeriod, StorageError> {
    let value = connection
        .query_row(
            "SELECT period FROM conversation_retention_policies WHERE profile_id = ?1",
            [DEFAULT_PROFILE_ID],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    ConversationRetentionPeriod::from_database(value.as_deref())
}
