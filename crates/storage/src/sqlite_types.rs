//! Custom SQLx `Type`, `Encode`, and `Decode` implementations for Jiff types.
//!
//! Because SQLx traits are foreign to this crate, we can't implement them
//! directly for `jiff::Timestamp` or `jiff::civil::Date` (orphan rules).
//! Instead we use transparent newtype wrappers that are **defined in this crate**
//! so the trait impls are legal.
//!
//! ## Wire formats
//! - `SqlTs` wraps `jiff::Timestamp` — stored as `INTEGER` (Unix epoch
//!   **milliseconds**, UTC).
//! - `SqlDate` wraps `jiff::civil::Date` — stored as `TEXT` (`YYYY-MM-DD`),
//!   matching SQLite's `date('now')` default.
//!
//! Both types deref to their inner jiff type so that existing code accessing
//! fields continues to work via `*field` or method calls.

use std::ops::Deref;

use jiff::{civil::Date, Timestamp};
use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef},
    Decode, Encode, Sqlite, Type,
};

// ── SqlTs — Timestamp as INTEGER (epoch ms) ──────────────────────────────────

/// Newtype wrapper: encodes/decodes `jiff::Timestamp` as an `INTEGER` column
/// holding Unix epoch milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct SqlTs(pub Timestamp);

impl Deref for SqlTs {
    type Target = Timestamp;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Timestamp> for SqlTs {
    fn from(ts: Timestamp) -> Self {
        SqlTs(ts)
    }
}

impl From<SqlTs> for Timestamp {
    fn from(w: SqlTs) -> Self {
        w.0
    }
}

impl Type<Sqlite> for SqlTs {
    fn type_info() -> SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for SqlTs {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let ms = <i64 as Decode<Sqlite>>::decode(value)?;
        Timestamp::from_millisecond(ms)
            .map(SqlTs)
            .map_err(Into::into)
    }
}

impl std::fmt::Display for SqlTs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'q> Encode<'q, Sqlite> for SqlTs {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        <i64 as Encode<'q, Sqlite>>::encode_by_ref(&self.0.as_millisecond(), buf)
    }
}

// ── SqlDate — civil::Date as TEXT (YYYY-MM-DD) ───────────────────────────────

/// Newtype wrapper: encodes/decodes `jiff::civil::Date` as a `TEXT` column
/// in ISO `YYYY-MM-DD` format, matching SQLite's `date('now')` default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct SqlDate(pub Date);

impl Deref for SqlDate {
    type Target = Date;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Date> for SqlDate {
    fn from(d: Date) -> Self {
        SqlDate(d)
    }
}

impl From<SqlDate> for Date {
    fn from(w: SqlDate) -> Self {
        w.0
    }
}

impl Type<Sqlite> for SqlDate {
    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for SqlDate {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <&str as Decode<Sqlite>>::decode(value)?;
        s.parse::<Date>().map(SqlDate).map_err(Into::into)
    }
}

impl<'q> Encode<'q, Sqlite> for SqlDate {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        <String as Encode<'q, Sqlite>>::encode_by_ref(&self.0.to_string(), buf)
    }
}
