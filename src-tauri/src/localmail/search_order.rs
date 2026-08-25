//! Closed ordering values shared by Bottie's provider and Localmail email-search boundaries.

use serde::{Deserialize, Serialize};

/// Closed Localmail ordering criterion exposed through Bottie's smaller search contract.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EmailSearchSort {
    /// Date-ordered matching messages, used by default for predictable recency.
    #[default]
    Date,
    /// Localmail's hybrid relevance ranking, used only when explicitly selected.
    Rank,
}

/// Closed Localmail ordering direction exposed through Bottie's smaller search contract.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(super) enum EmailSearchSortOrder {
    /// Newest first, Bottie's default email-search direction.
    #[default]
    #[serde(rename = "desc")]
    Descending,
    /// Oldest first, valid only for date ordering.
    #[serde(rename = "asc")]
    Ascending,
}
