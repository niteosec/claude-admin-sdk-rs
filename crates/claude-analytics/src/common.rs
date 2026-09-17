//! Types and checks shared across endpoints.

use std::pin::Pin;

use claude_api_core::{Error, Result};
use futures_core::Stream;
use jiff::civil::Date;
use jiff::{SignedDuration, Span, Timestamp};
use serde::{Deserialize, Serialize};

/// The stream returned by the `stream()` methods. Boxed so it is `Unpin` and works with
/// `TryStreamExt::try_next` directly.
pub type AnalyticsStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send + 'static>>;

/// The earliest date with data: 2026-01-01.
pub const EARLIEST_DATE: Date = jiff::civil::date(2026, 1, 1);

/// The earliest `starting_at` the cost and usage reports accept: 2026-01-01T00:00:00Z.
pub const EARLIEST_TIMESTAMP: Timestamp = Timestamp::constant(1_767_225_600, 0);

/// The documented upper bound on every array parameter (`filter[]`, `group_by[]`, `products[]`, …).
pub const MAX_ARRAY_ITEMS: usize = 100;

claude_api_core::string_enum! {
    /// Sort direction (`order`).
    pub enum SortOrder {
        /// `asc`
        Asc = "asc",
        /// `desc`
        Desc = "desc",
    }
}

claude_api_core::string_enum! {
    /// A currency code. Currently always `USD`.
    pub enum Currency {
        /// `USD`
        Usd = "USD",
    }
}

claude_api_core::string_enum! {
    /// `type` of an [`AnalyticsUser`]. Always `user`.
    pub enum AnalyticsUserType {
        /// `user`
        User = "user",
    }
}

claude_api_core::string_enum! {
    /// `type` of an [`AnalyticsUserActor`]. Always `user_actor`.
    pub enum AnalyticsUserActorType {
        /// `user_actor`
        UserActor = "user_actor",
    }
}

/// A user in the organization, identified by tagged ID and email address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyticsUser {
    /// Object type. Always `user`.
    #[serde(rename = "type")]
    pub kind: AnalyticsUserType,
    /// Tagged user ID (`user_…`).
    pub id: String,
    /// Email address of the user.
    pub email_address: String,
}

/// The user a per-user cost or usage row is attributed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyticsUserActor {
    /// Actor type. Always `user_actor`.
    #[serde(rename = "type")]
    pub kind: AnalyticsUserActorType,
    /// True when the account was deleted or the user is no longer a member of the organization.
    pub deleted: bool,
    /// Email address. `null` when the account was deleted and for system-minted service accounts.
    #[serde(default)]
    pub email: Option<String>,
    /// Full name. `null` when unset; `"Deleted User"` for deleted accounts and, when the
    /// organization hides them, removed users; the service name for service accounts.
    #[serde(default)]
    pub name: Option<String>,
    /// Tagged user ID, populated even for deleted users.
    pub user_id: String,
}

pub(crate) fn invalid(message: String) -> Error {
    Error::InvalidArgument(message)
}

pub(crate) fn check_items(name: &str, count: usize) -> Result<()> {
    if count > MAX_ARRAY_ITEMS {
        return Err(invalid(format!("{name} takes at most {MAX_ARRAY_ITEMS} entries, got {count}")));
    }
    Ok(())
}

pub(crate) fn check_limit(limit: Option<u32>, max: u32) -> Result<()> {
    match limit {
        Some(limit) if !(1..=max).contains(&limit) => {
            Err(invalid(format!("limit must be between 1 and {max}, got {limit}")))
        }
        _ => Ok(()),
    }
}

pub(crate) fn check_date(name: &str, date: Date) -> Result<()> {
    if date < EARLIEST_DATE {
        return Err(invalid(format!("{name} must be no earlier than {EARLIEST_DATE}, got {date}")));
    }
    Ok(())
}

pub(crate) fn check_timestamp(name: &str, at: Timestamp) -> Result<()> {
    if at < EARLIEST_TIMESTAMP {
        return Err(invalid(format!("{name} must be no earlier than {EARLIEST_TIMESTAMP}, got {at}")));
    }
    Ok(())
}

/// `ending_date` at most `days` after `starting_date`.
pub(crate) fn check_date_span(starting: Date, ending: Date, days: i64) -> Result<()> {
    if let Ok(latest) = starting.checked_add(Span::new().days(days)) {
        if ending > latest {
            return Err(invalid(format!(
                "ending_date must be at most {days} days after starting_date ({starting}), got {ending}"
            )));
        }
    }
    Ok(())
}

/// `ending_at` at most `max` after `starting_at`.
pub(crate) fn check_time_span(starting: Timestamp, ending: Timestamp, max: SignedDuration, what: &str) -> Result<()> {
    if ending.duration_since(starting) > max {
        return Err(invalid(format!("{what}: {starting} to {ending}")));
    }
    Ok(())
}
