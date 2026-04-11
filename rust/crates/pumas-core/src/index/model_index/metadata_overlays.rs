use super::{ModelIndex, ModelMetadataHistoryRecord, ModelMetadataOverlayRecord};
use crate::{PumasError, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

impl ModelIndex {
    /// Return the active metadata overlay row for a model, if present.
    pub fn get_active_metadata_overlay(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelMetadataOverlayRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let row = conn
            .query_row(
                "SELECT
                   overlay_id,
                   model_id,
                   overlay_json,
                   status,
                   reason,
                   created_at,
                   created_by
                 FROM model_metadata_overlays
                 WHERE model_id = ?1 AND status = 'active'
                 ORDER BY created_at DESC
                 LIMIT 1",
                params![model_id],
                |row| {
                    Ok(ModelMetadataOverlayRecord {
                        overlay_id: row.get(0)?,
                        model_id: row.get(1)?,
                        overlay_json: row.get(2)?,
                        status: row.get(3)?,
                        reason: row.get(4)?,
                        created_at: row.get(5)?,
                        created_by: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(row)
    }

    /// Return model metadata history rows in deterministic order.
    pub fn list_model_metadata_history(
        &self,
        model_id: &str,
    ) -> Result<Vec<ModelMetadataHistoryRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT
               event_id,
               model_id,
               overlay_id,
               actor,
               action,
               field_path,
               old_value_json,
               new_value_json,
               reason,
               created_at
             FROM model_metadata_history
             WHERE model_id = ?1
             ORDER BY created_at ASC, event_id ASC",
        )?;

        let rows = stmt.query_map(params![model_id], |row| {
            Ok(ModelMetadataHistoryRecord {
                event_id: row.get(0)?,
                model_id: row.get(1)?,
                overlay_id: row.get(2)?,
                actor: row.get(3)?,
                action: row.get(4)?,
                field_path: row.get(5)?,
                old_value_json: row.get(6)?,
                new_value_json: row.get(7)?,
                reason: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }
        Ok(history)
    }

    /// Resolve effective metadata JSON (`baseline + active overlay`) for a model.
    pub fn get_effective_metadata_json(&self, model_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let baseline_json: Option<String> = conn
            .query_row(
                "SELECT baseline_json
                 FROM model_metadata_baselines
                 WHERE model_id = ?1",
                params![model_id],
                |row| row.get(0),
            )
            .optional()?;

        let baseline_json = if let Some(value) = baseline_json {
            Some(value)
        } else {
            conn.query_row(
                "SELECT metadata_json
                 FROM models
                 WHERE id = ?1",
                params![model_id],
                |row| row.get(0),
            )
            .optional()?
        };

        let Some(baseline_json) = baseline_json else {
            return Ok(None);
        };

        let active_overlay_json: Option<String> = conn
            .query_row(
                "SELECT overlay_json
                 FROM model_metadata_overlays
                 WHERE model_id = ?1 AND status = 'active'
                 ORDER BY created_at DESC
                 LIMIT 1",
                params![model_id],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(overlay_json) = active_overlay_json {
            let mut effective: Value = serde_json::from_str(&baseline_json)?;
            let patch: Value = serde_json::from_str(&overlay_json)?;
            apply_merge_patch(&mut effective, &patch);
            Ok(Some(serde_json::to_string(&effective)?))
        } else {
            Ok(Some(baseline_json))
        }
    }

    /// Return baseline metadata JSON for a model if present.
    pub fn get_baseline_metadata_json(&self, model_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let baseline_json: Option<String> = conn
            .query_row(
                "SELECT baseline_json
                 FROM model_metadata_baselines
                 WHERE model_id = ?1",
                params![model_id],
                |row| row.get(0),
            )
            .optional()?;

        Ok(baseline_json)
    }

    /// Apply a metadata overlay in one transaction (supersede current active, create new active, append history).
    pub fn apply_metadata_overlay(
        &self,
        model_id: &str,
        overlay_id: &str,
        overlay_json: &Value,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        let reason = reason.map(str::to_string);
        let overlay_json_str = serde_json::to_string(overlay_json)?;

        Self::ensure_metadata_baseline_in_tx(&tx, model_id, actor, &now)?;

        let existing_active: Option<(String, String)> = tx
            .query_row(
                "SELECT overlay_id, overlay_json
                 FROM model_metadata_overlays
                 WHERE model_id = ?1 AND status = 'active'
                 LIMIT 1",
                params![model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((old_overlay_id, old_overlay_json)) = existing_active {
            tx.execute(
                "UPDATE model_metadata_overlays
                 SET status = 'superseded', reason = COALESCE(?1, reason)
                 WHERE overlay_id = ?2",
                params![reason, old_overlay_id],
            )?;

            tx.execute(
                "INSERT INTO model_metadata_history (
                   model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
                 ) VALUES (?1, ?2, ?3, 'overlay_superseded', NULL, ?4, ?5, ?6, ?7)",
                params![
                    model_id,
                    old_overlay_id,
                    actor,
                    old_overlay_json,
                    overlay_json_str,
                    reason,
                    now
                ],
            )?;
        }

        tx.execute(
            "INSERT INTO model_metadata_overlays (
               overlay_id, model_id, overlay_json, status, reason, created_at, created_by
             ) VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6)",
            params![overlay_id, model_id, overlay_json_str, reason, now, actor],
        )?;

        tx.execute(
            "INSERT INTO model_metadata_history (
               model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
             ) VALUES (?1, ?2, ?3, 'overlay_created', NULL, NULL, ?4, ?5, ?6)",
            params![model_id, overlay_id, actor, overlay_json_str, reason, now],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Reset model metadata to baseline by reverting/removing active overlay.
    pub fn reset_metadata_overlay(
        &self,
        model_id: &str,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<bool> {
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        let reason = reason.map(str::to_string);

        let active_overlay: Option<(String, String)> = tx
            .query_row(
                "SELECT overlay_id, overlay_json
                 FROM model_metadata_overlays
                 WHERE model_id = ?1 AND status = 'active'
                 LIMIT 1",
                params![model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let Some((overlay_id, overlay_json)) = active_overlay else {
            return Ok(false);
        };

        tx.execute(
            "UPDATE model_metadata_overlays
             SET status = 'reverted', reason = COALESCE(?1, reason)
             WHERE overlay_id = ?2",
            params![reason, overlay_id],
        )?;

        tx.execute(
            "INSERT INTO model_metadata_history (
               model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
             ) VALUES (?1, ?2, ?3, 'overlay_reverted', NULL, ?4, NULL, ?5, ?6)",
            params![model_id, overlay_id, actor, overlay_json, reason, now],
        )?;

        tx.execute(
            "INSERT INTO model_metadata_history (
               model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
             ) VALUES (?1, ?2, ?3, 'reset_to_original', NULL, ?4, NULL, ?5, ?6)",
            params![model_id, overlay_id, actor, overlay_json, reason, now],
        )?;

        tx.commit()?;
        Ok(true)
    }

    fn ensure_metadata_baseline_in_tx(
        tx: &rusqlite::Transaction<'_>,
        model_id: &str,
        actor: &str,
        now: &str,
    ) -> Result<()> {
        let baseline_exists: i64 = tx.query_row(
            "SELECT COUNT(*)
             FROM model_metadata_baselines
             WHERE model_id = ?1",
            params![model_id],
            |row| row.get(0),
        )?;
        if baseline_exists > 0 {
            return Ok(());
        }

        let model_metadata_json: String = tx
            .query_row(
                "SELECT metadata_json
                 FROM models
                 WHERE id = ?1",
                params![model_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;

        let parsed: Value = serde_json::from_str(&model_metadata_json)?;
        let schema_version = parsed
            .get("schema_version")
            .and_then(|value| value.as_i64())
            .unwrap_or(1);

        tx.execute(
            "INSERT OR IGNORE INTO model_metadata_baselines (
               model_id, schema_version, baseline_json, created_at, created_by
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![model_id, schema_version, model_metadata_json, now, actor],
        )?;

        tx.execute(
            "INSERT INTO model_metadata_history (
               model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
             )
             SELECT
               b.model_id,
               NULL,
               ?1,
               'baseline_created',
               NULL,
               NULL,
               b.baseline_json,
               'overlay-baseline-bootstrap',
               ?2
             FROM model_metadata_baselines b
             WHERE b.model_id = ?3
               AND NOT EXISTS (
                 SELECT 1 FROM model_metadata_history h
                 WHERE h.model_id = b.model_id AND h.action = 'baseline_created'
               )",
            params![actor, now, model_id],
        )?;

        Ok(())
    }
}

/// Apply RFC 7396-style JSON Merge Patch.
fn apply_merge_patch(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(patch_map) => {
            if !target.is_object() {
                *target = Value::Object(serde_json::Map::new());
            }
            let target_map = target
                .as_object_mut()
                .expect("target must be an object after normalization");

            for (key, patch_value) in patch_map {
                if patch_value.is_null() {
                    target_map.remove(key);
                    continue;
                }

                match target_map.get_mut(key) {
                    Some(current) => apply_merge_patch(current, patch_value),
                    None => {
                        target_map.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        _ => {
            *target = patch.clone();
        }
    }
}
