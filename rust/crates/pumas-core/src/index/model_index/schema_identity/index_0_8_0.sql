-- Schema of a model index written by Pumas Library 0.8.0 (base commit
-- b091ac45), dumped from sqlite_master. FTS5 shadow tables are omitted; the
-- CREATE VIRTUAL TABLE statement recreates them. The database carries no
-- PRAGMA user_version, as every release before the schema identity.

CREATE TABLE models (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                cleaned_name TEXT NOT NULL,
                official_name TEXT NOT NULL,
                model_type TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                hashes_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

CREATE INDEX idx_models_type ON models(model_type);

CREATE INDEX idx_models_updated ON models(updated_at);

CREATE TABLE model_link_exclusions (
                model_id TEXT NOT NULL,
                app_id TEXT NOT NULL,
                excluded_at TEXT NOT NULL,
                PRIMARY KEY (model_id, app_id)
            );

CREATE TABLE task_signature_mappings (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              signature_key TEXT NOT NULL,
              mapping_version INTEGER NOT NULL,
              input_modalities_json TEXT NOT NULL,
              output_modalities_json TEXT NOT NULL,
              task_type_primary TEXT NOT NULL,
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              supersedes_id INTEGER,
              change_reason TEXT,
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (supersedes_id) REFERENCES task_signature_mappings(id),
              UNIQUE(signature_key, mapping_version)
            );

CREATE INDEX idx_task_signature_mappings_lookup
              ON task_signature_mappings(status, signature_key, priority, mapping_version DESC);

CREATE UNIQUE INDEX idx_task_signature_mappings_one_active
              ON task_signature_mappings(signature_key)
              WHERE status = 'active';

CREATE UNIQUE INDEX idx_task_signature_mappings_one_pending
              ON task_signature_mappings(signature_key)
              WHERE status = 'pending';

CREATE TABLE model_type_arch_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              pattern TEXT NOT NULL,
              match_style TEXT NOT NULL CHECK (match_style IN ('exact', 'prefix', 'suffix', 'wildcard')),
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'vlm', 'reranker', 'decision', 'diffusion', 'video', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

CREATE UNIQUE INDEX idx_model_type_arch_rules_active_unique
              ON model_type_arch_rules(pattern, match_style)
              WHERE status = 'active';

CREATE INDEX idx_model_type_arch_rules_lookup
              ON model_type_arch_rules(status, priority, pattern, match_style);

CREATE TABLE model_type_config_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              config_model_type TEXT NOT NULL,
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'vlm', 'reranker', 'decision', 'diffusion', 'video', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

CREATE UNIQUE INDEX idx_model_type_config_rules_active_unique
              ON model_type_config_rules(config_model_type)
              WHERE status = 'active';

CREATE INDEX idx_model_type_config_rules_lookup
              ON model_type_config_rules(status, priority, config_model_type);

CREATE TABLE model_metadata_baselines (
              model_id TEXT PRIMARY KEY,
              schema_version INTEGER NOT NULL,
              baseline_json TEXT NOT NULL CHECK (json_valid(baseline_json)),
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              created_by TEXT NOT NULL DEFAULT 'pumas-library',
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

CREATE TRIGGER trg_model_metadata_baselines_no_update
            BEFORE UPDATE ON model_metadata_baselines
            FOR EACH ROW
            BEGIN
              SELECT RAISE(ABORT, 'model_metadata_baselines is immutable');
            END;

CREATE TABLE model_metadata_overlays (
              overlay_id TEXT PRIMARY KEY,
              model_id TEXT NOT NULL,
              overlay_json TEXT NOT NULL CHECK (json_valid(overlay_json)),
              status TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'superseded', 'reverted')),
              reason TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              created_by TEXT NOT NULL,
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

CREATE INDEX idx_model_metadata_overlays_model
              ON model_metadata_overlays(model_id, created_at);

CREATE UNIQUE INDEX idx_model_metadata_overlays_one_active
              ON model_metadata_overlays(model_id)
              WHERE status = 'active';

CREATE TABLE model_metadata_history (
              event_id INTEGER PRIMARY KEY AUTOINCREMENT,
              model_id TEXT NOT NULL,
              overlay_id TEXT,
              actor TEXT NOT NULL,
              action TEXT NOT NULL
                CHECK (action IN (
                  'baseline_created',
                  'overlay_created',
                  'overlay_superseded',
                  'overlay_reverted',
                  'reset_to_original',
                  'field_updated'
                )),
              field_path TEXT,
              old_value_json TEXT,
              new_value_json TEXT,
              reason TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
              FOREIGN KEY (overlay_id) REFERENCES model_metadata_overlays(overlay_id) ON DELETE SET NULL
            );

CREATE INDEX idx_model_metadata_history_model
              ON model_metadata_history(model_id, created_at);

CREATE TABLE dependency_profiles (
              profile_id TEXT NOT NULL,
              profile_version INTEGER NOT NULL,
              profile_hash TEXT,
              environment_kind TEXT NOT NULL,
              spec_json TEXT NOT NULL CHECK (json_valid(spec_json)),
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              PRIMARY KEY (profile_id, profile_version)
            );

CREATE INDEX idx_dependency_profiles_hash
              ON dependency_profiles(profile_hash);

CREATE TABLE model_dependency_bindings (
              binding_id TEXT PRIMARY KEY,
              model_id TEXT NOT NULL,
              profile_id TEXT NOT NULL,
              profile_version INTEGER NOT NULL,
              binding_kind TEXT NOT NULL,
              backend_key TEXT,
              platform_selector TEXT,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'deprecated')),
              priority INTEGER NOT NULL DEFAULT 100,
              attached_by TEXT,
              attached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
              FOREIGN KEY (profile_id, profile_version) REFERENCES dependency_profiles(profile_id, profile_version)
            );

CREATE INDEX idx_model_dependency_bindings_model
              ON model_dependency_bindings(model_id, status, binding_kind, backend_key, priority, binding_id);

CREATE TABLE dependency_binding_history (
              event_id INTEGER PRIMARY KEY AUTOINCREMENT,
              binding_id TEXT NOT NULL,
              model_id TEXT NOT NULL,
              actor TEXT NOT NULL,
              action TEXT NOT NULL,
              old_value_json TEXT,
              new_value_json TEXT,
              reason TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (binding_id) REFERENCES model_dependency_bindings(binding_id) ON DELETE CASCADE,
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

CREATE TABLE model_package_facts_cache (
              model_id TEXT NOT NULL,
              selected_artifact_id TEXT NOT NULL DEFAULT '',
              cache_scope TEXT NOT NULL CHECK (cache_scope IN ('summary', 'detail')),
              package_facts_contract_version INTEGER NOT NULL,
              producer_revision TEXT,
              source_fingerprint TEXT NOT NULL,
              facts_json TEXT NOT NULL CHECK (json_valid(facts_json)),
              cached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              PRIMARY KEY (model_id, selected_artifact_id, cache_scope),
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

CREATE INDEX idx_model_package_facts_cache_model
              ON model_package_facts_cache(model_id, cache_scope, selected_artifact_id);

CREATE TABLE model_library_update_events (
              event_id INTEGER PRIMARY KEY AUTOINCREMENT,
              model_id TEXT NOT NULL,
              selected_artifact_id TEXT,
              change_kind TEXT NOT NULL CHECK (change_kind IN (
                'model_added',
                'model_removed',
                'metadata_modified',
                'package_facts_modified',
                'stale_facts_invalidated',
                'dependency_binding_modified'
              )),
              fact_family TEXT NOT NULL CHECK (fact_family IN (
                'model_record',
                'metadata',
                'package_facts',
                'dependency_bindings',
                'validation',
                'search_index'
              )),
              refresh_scope TEXT NOT NULL CHECK (refresh_scope IN (
                'summary',
                'detail',
                'summary_and_detail'
              )),
              producer_revision TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

CREATE INDEX idx_model_library_update_events_event
              ON model_library_update_events(event_id);

CREATE INDEX idx_model_library_update_events_model
              ON model_library_update_events(model_id, event_id);

CREATE TABLE intent_schema_meta (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                schema_version INTEGER NOT NULL CHECK (schema_version > 0)
             );

CREATE TABLE intent_declarations (
                declaration_id TEXT PRIMARY KEY,
                consumer_key TEXT NOT NULL,
                original_requirement_json TEXT NOT NULL CHECK (json_valid(original_requirement_json)),
                generation TEXT NOT NULL,
                model_id TEXT,
                bound_target_json TEXT CHECK (bound_target_json IS NULL OR json_valid(bound_target_json)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );

CREATE INDEX idx_intent_declarations_model
                ON intent_declarations(model_id);

CREATE INDEX idx_intent_declarations_consumer
                ON intent_declarations(consumer_key);

CREATE TABLE intent_deletion_claims (
                model_id TEXT PRIMARY KEY,
                claim_token TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
             );

CREATE VIRTUAL TABLE model_search USING fts5(
                id,
                official_name,
                cleaned_name,
                model_type,
                tags,
                family,
                description,
                tokenize='unicode61 remove_diacritics 1'
            );

CREATE TRIGGER model_search_ai AFTER INSERT ON models BEGIN
                INSERT INTO model_search (
                    id, official_name, cleaned_name, model_type,
                    tags, family, description
                ) VALUES (
                    NEW.id,
                    NEW.official_name,
                    NEW.cleaned_name,
                    NEW.model_type,
                    (SELECT GROUP_CONCAT(value, ' ') FROM json_each(NEW.tags_json)),
                    COALESCE(
                        json_extract(NEW.metadata_json, '$.architecture_family'),
                        json_extract(NEW.metadata_json, '$.family')
                    ),
                    json_extract(NEW.metadata_json, '$.description')
                );
            END;

CREATE TRIGGER model_search_au AFTER UPDATE ON models BEGIN
                DELETE FROM model_search WHERE id = OLD.id;
                INSERT INTO model_search (
                    id, official_name, cleaned_name, model_type,
                    tags, family, description
                ) VALUES (
                    NEW.id,
                    NEW.official_name,
                    NEW.cleaned_name,
                    NEW.model_type,
                    (SELECT GROUP_CONCAT(value, ' ') FROM json_each(NEW.tags_json)),
                    COALESCE(
                        json_extract(NEW.metadata_json, '$.architecture_family'),
                        json_extract(NEW.metadata_json, '$.family')
                    ),
                    json_extract(NEW.metadata_json, '$.description')
                );
            END;

CREATE TRIGGER model_search_ad AFTER DELETE ON models BEGIN
                DELETE FROM model_search WHERE id = OLD.id;
            END;

INSERT INTO intent_schema_meta (singleton, schema_version) VALUES (1, 1);
