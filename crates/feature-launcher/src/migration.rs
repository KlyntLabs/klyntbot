//! One-shot ID migrations for the launcher.
//!
//! After AppIndex first resolves bundle IDs from Info.plist, any pre-existing
//! pin or usage-log rows keyed by `app:{path}` are rewritten to `app:{bundle_id}`.
//! Idempotent: rows already in bundle-ID form are no-ops.

use crate::search::AppEntry;
use sqlx::SqlitePool;

/// Rewrite pin + usage-log IDs for any app whose `bundle_id` is now known.
/// Returns the total number of rows updated across both tables.
pub async fn migrate_app_ids_to_bundle_ids(
    pool: &SqlitePool,
    apps: &[AppEntry],
) -> Result<u64, sqlx::Error> {
    let mut total: u64 = 0;
    for app in apps {
        let Some(bid) = &app.bundle_id else { continue };
        let old_id = format!("app:{}", app.path.display());
        let new_id = format!("app:{bid}");
        if old_id == new_id {
            continue;
        }

        let pins_result = sqlx::query(
            "UPDATE launcher_pins SET item_id = ?1 \
             WHERE item_id = ?2 AND kind = 'application'",
        )
        .bind(&new_id)
        .bind(&old_id)
        .execute(pool)
        .await?;
        total += pins_result.rows_affected();

        let usage_result = sqlx::query(
            "UPDATE launcher_usage_log SET item_id = ?1 \
             WHERE item_id = ?2 AND kind = 'application'",
        )
        .bind(&new_id)
        .bind(&old_id)
        .execute(pool)
        .await?;
        total += usage_result.rows_affected();
    }
    Ok(total)
}
