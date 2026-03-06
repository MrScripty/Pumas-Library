use super::{
    DependencyBindingHistoryRecord, DependencyProfileRecord, ModelDependencyBindingRecord,
    ModelIndex,
};
use crate::model_library::dependency_pins::parse_and_canonicalize_profile_spec;
use crate::{PumasError, Result};
use rusqlite::{params, OptionalExtension};

impl ModelIndex {
    /// Insert or update a dependency profile row.
    pub fn upsert_dependency_profile(&self, record: &DependencyProfileRecord) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let field_context = format!(
            "dependency_profiles.{}:{}",
            record.profile_id, record.profile_version
        );
        let normalized = parse_and_canonicalize_profile_spec(
            &record.spec_json,
            &record.environment_kind,
            &field_context,
        )?;

        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT environment_kind, spec_json
                 FROM dependency_profiles
                 WHERE profile_id = ?1 AND profile_version = ?2",
                params![record.profile_id, record.profile_version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((existing_environment_kind, existing_spec_json)) = existing {
            let existing_normalized = parse_and_canonicalize_profile_spec(
                &existing_spec_json,
                &existing_environment_kind,
                &field_context,
            )?;

            if existing_environment_kind != record.environment_kind
                || existing_normalized.profile_hash != normalized.profile_hash
            {
                return Err(PumasError::Validation {
                    field: field_context,
                    message:
                        "dependency_profile_version_immutable: profile content for this (profile_id, profile_version) cannot change"
                            .to_string(),
                });
            }

            conn.execute(
                "UPDATE dependency_profiles
                 SET profile_hash = ?3,
                     environment_kind = ?4,
                     spec_json = ?5
                 WHERE profile_id = ?1 AND profile_version = ?2",
                params![
                    record.profile_id,
                    record.profile_version,
                    normalized.profile_hash,
                    record.environment_kind,
                    normalized.canonical_json,
                ],
            )?;
            return Ok(());
        }

        conn.execute(
            "INSERT INTO dependency_profiles (
               profile_id, profile_version, profile_hash, environment_kind, spec_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.profile_id,
                record.profile_version,
                normalized.profile_hash,
                record.environment_kind,
                normalized.canonical_json,
                record.created_at,
            ],
        )?;

        Ok(())
    }

    /// Check whether a dependency profile exists by (profile_id, profile_version).
    pub fn dependency_profile_exists(
        &self,
        profile_id: &str,
        profile_version: i64,
    ) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM dependency_profiles
             WHERE profile_id = ?1 AND profile_version = ?2",
            params![profile_id, profile_version],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    /// Load a dependency profile by `(profile_id, profile_version)`.
    pub fn get_dependency_profile(
        &self,
        profile_id: &str,
        profile_version: i64,
    ) -> Result<Option<DependencyProfileRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let record = conn
            .query_row(
                "SELECT
                   profile_id,
                   profile_version,
                   profile_hash,
                   environment_kind,
                   spec_json,
                   created_at
                 FROM dependency_profiles
                 WHERE profile_id = ?1 AND profile_version = ?2",
                params![profile_id, profile_version],
                |row| {
                    Ok(DependencyProfileRecord {
                        profile_id: row.get(0)?,
                        profile_version: row.get(1)?,
                        profile_hash: row.get(2)?,
                        environment_kind: row.get(3)?,
                        spec_json: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?;

        Ok(record)
    }

    /// Insert or update a model dependency binding row.
    pub fn upsert_model_dependency_binding(
        &self,
        record: &ModelDependencyBindingRecord,
    ) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let tx = conn.transaction()?;

        let existing: Option<ModelDependencyBindingRecord> = tx
            .query_row(
                "SELECT
                   binding_id,
                   model_id,
                   profile_id,
                   profile_version,
                   binding_kind,
                   backend_key,
                   platform_selector,
                   status,
                   priority,
                   attached_by,
                   attached_at
                 FROM model_dependency_bindings
                 WHERE binding_id = ?1",
                params![record.binding_id],
                |row| {
                    Ok(ModelDependencyBindingRecord {
                        binding_id: row.get(0)?,
                        model_id: row.get(1)?,
                        profile_id: row.get(2)?,
                        profile_version: row.get(3)?,
                        binding_kind: row.get(4)?,
                        backend_key: row.get(5)?,
                        platform_selector: row.get(6)?,
                        status: row.get(7)?,
                        priority: row.get(8)?,
                        attached_by: row.get(9)?,
                        attached_at: row.get(10)?,
                        profile_hash: None,
                        environment_kind: None,
                        spec_json: None,
                    })
                },
            )
            .optional()?;

        tx.execute(
            "INSERT INTO model_dependency_bindings (
               binding_id, model_id, profile_id, profile_version, binding_kind, backend_key,
               platform_selector, status, priority, attached_by, attached_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(binding_id) DO UPDATE SET
               model_id = excluded.model_id,
               profile_id = excluded.profile_id,
               profile_version = excluded.profile_version,
               binding_kind = excluded.binding_kind,
               backend_key = excluded.backend_key,
               platform_selector = excluded.platform_selector,
               status = excluded.status,
               priority = excluded.priority,
               attached_by = excluded.attached_by,
               attached_at = excluded.attached_at",
            params![
                record.binding_id,
                record.model_id,
                record.profile_id,
                record.profile_version,
                record.binding_kind,
                record.backend_key,
                record.platform_selector,
                record.status,
                record.priority,
                record.attached_by,
                record.attached_at,
            ],
        )?;

        let old_snapshot = existing.as_ref().map(dependency_binding_snapshot_json);
        let new_snapshot = dependency_binding_snapshot_json(record);
        let actor = record
            .attached_by
            .clone()
            .or_else(|| existing.as_ref().and_then(|row| row.attached_by.clone()))
            .unwrap_or_else(|| "system".to_string());

        let action = if existing.is_none() {
            Some("binding_created")
        } else if old_snapshot.as_ref() != Some(&new_snapshot) {
            Some("binding_updated")
        } else {
            None
        };

        if let Some(action) = action {
            let reason = existing.as_ref().and_then(|old| {
                if old.status != record.status {
                    Some("status-changed".to_string())
                } else {
                    None
                }
            });
            tx.execute(
                "INSERT INTO dependency_binding_history (
                   binding_id, model_id, actor, action, old_value_json, new_value_json, reason, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                params![
                    record.binding_id,
                    record.model_id,
                    actor,
                    action,
                    old_snapshot,
                    new_snapshot,
                    reason,
                ],
            )?;
        }

        tx.commit()?;

        Ok(())
    }

    /// List dependency binding history rows in deterministic event order.
    pub fn list_dependency_binding_history(
        &self,
        model_id: &str,
    ) -> Result<Vec<DependencyBindingHistoryRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT
               event_id,
               binding_id,
               model_id,
               actor,
               action,
               old_value_json,
               new_value_json,
               reason,
               created_at
             FROM dependency_binding_history
             WHERE model_id = ?1
             ORDER BY event_id ASC",
        )?;

        let rows = stmt.query_map(params![model_id], |row| {
            Ok(DependencyBindingHistoryRecord {
                event_id: row.get(0)?,
                binding_id: row.get(1)?,
                model_id: row.get(2)?,
                actor: row.get(3)?,
                action: row.get(4)?,
                old_value_json: row.get(5)?,
                new_value_json: row.get(6)?,
                reason: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }
        Ok(history)
    }

    /// List active dependency bindings for a model with optional backend filtering.
    pub fn list_active_model_dependency_bindings(
        &self,
        model_id: &str,
        backend_key: Option<&str>,
    ) -> Result<Vec<ModelDependencyBindingRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT
               b.binding_id,
               b.model_id,
               b.profile_id,
               b.profile_version,
               b.binding_kind,
               b.backend_key,
               b.platform_selector,
               b.status,
               b.priority,
               b.attached_by,
               b.attached_at,
               p.profile_hash,
               p.environment_kind,
               p.spec_json
             FROM model_dependency_bindings b
             LEFT JOIN dependency_profiles p
               ON p.profile_id = b.profile_id AND p.profile_version = b.profile_version
             WHERE b.model_id = ?1
               AND b.status = 'active'
               AND (
                 ?2 IS NULL
                 OR b.backend_key IS NULL
                 OR lower(b.backend_key) = lower(?2)
               )",
        )?;

        let rows = stmt.query_map(params![model_id, backend_key], |row| {
            Ok(ModelDependencyBindingRecord {
                binding_id: row.get(0)?,
                model_id: row.get(1)?,
                profile_id: row.get(2)?,
                profile_version: row.get(3)?,
                binding_kind: row.get(4)?,
                backend_key: row.get(5)?,
                platform_selector: row.get(6)?,
                status: row.get(7)?,
                priority: row.get(8)?,
                attached_by: row.get(9)?,
                attached_at: row.get(10)?,
                profile_hash: row.get(11)?,
                environment_kind: row.get(12)?,
                spec_json: row.get(13)?,
            })
        })?;

        let mut bindings = Vec::new();
        for row in rows {
            bindings.push(row?);
        }

        bindings.sort_by(|a, b| {
            a.binding_kind
                .to_lowercase()
                .cmp(&b.binding_kind.to_lowercase())
                .then_with(|| {
                    a.backend_key
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .cmp(&b.backend_key.as_deref().unwrap_or("").to_lowercase())
                })
                .then_with(|| {
                    a.platform_selector
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .cmp(&b.platform_selector.as_deref().unwrap_or("").to_lowercase())
                })
                .then_with(|| a.profile_id.cmp(&b.profile_id))
                .then_with(|| a.profile_version.cmp(&b.profile_version))
                .then_with(|| a.priority.cmp(&b.priority))
                .then_with(|| a.binding_id.cmp(&b.binding_id))
        });

        Ok(bindings)
    }
}

fn dependency_binding_snapshot_json(record: &ModelDependencyBindingRecord) -> String {
    serde_json::json!({
        "binding_id": record.binding_id,
        "model_id": record.model_id,
        "profile_id": record.profile_id,
        "profile_version": record.profile_version,
        "binding_kind": record.binding_kind,
        "backend_key": record.backend_key,
        "platform_selector": record.platform_selector,
        "status": record.status,
        "priority": record.priority,
        "attached_by": record.attached_by,
        "attached_at": record.attached_at,
    })
    .to_string()
}
