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
    let mappings: Vec<(String, String)> = apps
        .iter()
        .filter_map(|app| {
            let bid = app.bundle_id.as_ref()?;
            let old_id = format!("app:{}", app.path.display());
            let new_id = format!("app:{bid}");
            if old_id == new_id {
                None
            } else {
                Some((old_id, new_id))
            }
        })
        .collect();

    if mappings.is_empty() {
        return Ok(0);
    }

    // Batch updates via a CTE mapping table to avoid N+1 queries.
    let mut total: u64 = 0;
    for chunk in mappings.chunks(400) {
        let values_sql = chunk.iter().map(|_| "(?, ?)").collect::<Vec<_>>().join(", ");

        let pins_sql = format!(
            "WITH mappings(old_id, new_id) AS (VALUES {})
             UPDATE launcher_pins SET item_id = (
                 SELECT new_id FROM mappings WHERE old_id = launcher_pins.item_id
             )
             WHERE item_id IN (SELECT old_id FROM mappings)
               AND kind = 'application'",
            values_sql
        );
        let mut pins_query = sqlx::query(&pins_sql);
        for (old_id, new_id) in chunk {
            pins_query = pins_query.bind(old_id).bind(new_id);
        }
        total += pins_query.execute(pool).await?.rows_affected();

        let usage_sql = format!(
            "WITH mappings(old_id, new_id) AS (VALUES {})
             UPDATE launcher_usage_log SET item_id = (
                 SELECT new_id FROM mappings WHERE old_id = launcher_usage_log.item_id
             )
             WHERE item_id IN (SELECT old_id FROM mappings)
               AND kind = 'application'",
            values_sql
        );
        let mut usage_query = sqlx::query(&usage_sql);
        for (old_id, new_id) in chunk {
            usage_query = usage_query.bind(old_id).bind(new_id);
        }
        total += usage_query.execute(pool).await?.rows_affected();
    }
    Ok(total)
}
