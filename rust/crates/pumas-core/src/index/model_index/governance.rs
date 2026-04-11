use super::*;

impl ModelIndex {
    /// Create metadata v2/additional governance tables.
    pub(super) fn ensure_metadata_v2_schema(conn: &Connection) -> Result<()> {
        Self::migrate_model_type_rule_constraints(conn)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS task_signature_mappings (
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

            CREATE INDEX IF NOT EXISTS idx_task_signature_mappings_lookup
              ON task_signature_mappings(status, signature_key, priority, mapping_version DESC);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_task_signature_mappings_one_active
              ON task_signature_mappings(signature_key)
              WHERE status = 'active';

            CREATE UNIQUE INDEX IF NOT EXISTS idx_task_signature_mappings_one_pending
              ON task_signature_mappings(signature_key)
              WHERE status = 'pending';

            CREATE TABLE IF NOT EXISTS model_type_arch_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              pattern TEXT NOT NULL,
              match_style TEXT NOT NULL CHECK (match_style IN ('exact', 'prefix', 'suffix', 'wildcard')),
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'vlm', 'reranker', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_type_arch_rules_active_unique
              ON model_type_arch_rules(pattern, match_style)
              WHERE status = 'active';

            CREATE INDEX IF NOT EXISTS idx_model_type_arch_rules_lookup
              ON model_type_arch_rules(status, priority, pattern, match_style);

            CREATE TABLE IF NOT EXISTS model_type_config_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              config_model_type TEXT NOT NULL,
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'vlm', 'reranker', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_type_config_rules_active_unique
              ON model_type_config_rules(config_model_type)
              WHERE status = 'active';

            CREATE INDEX IF NOT EXISTS idx_model_type_config_rules_lookup
              ON model_type_config_rules(status, priority, config_model_type);

            CREATE TABLE IF NOT EXISTS model_metadata_baselines (
              model_id TEXT PRIMARY KEY,
              schema_version INTEGER NOT NULL,
              baseline_json TEXT NOT NULL CHECK (json_valid(baseline_json)),
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              created_by TEXT NOT NULL DEFAULT 'pumas-library',
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

            CREATE TRIGGER IF NOT EXISTS trg_model_metadata_baselines_no_update
            BEFORE UPDATE ON model_metadata_baselines
            FOR EACH ROW
            BEGIN
              SELECT RAISE(ABORT, 'model_metadata_baselines is immutable');
            END;

            CREATE TABLE IF NOT EXISTS model_metadata_overlays (
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

            CREATE INDEX IF NOT EXISTS idx_model_metadata_overlays_model
              ON model_metadata_overlays(model_id, created_at);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_metadata_overlays_one_active
              ON model_metadata_overlays(model_id)
              WHERE status = 'active';

            CREATE TABLE IF NOT EXISTS model_metadata_history (
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

            CREATE INDEX IF NOT EXISTS idx_model_metadata_history_model
              ON model_metadata_history(model_id, created_at);

            CREATE TABLE IF NOT EXISTS dependency_profiles (
              profile_id TEXT NOT NULL,
              profile_version INTEGER NOT NULL,
              profile_hash TEXT,
              environment_kind TEXT NOT NULL,
              spec_json TEXT NOT NULL CHECK (json_valid(spec_json)),
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              PRIMARY KEY (profile_id, profile_version)
            );

            CREATE INDEX IF NOT EXISTS idx_dependency_profiles_hash
              ON dependency_profiles(profile_hash);

            CREATE TABLE IF NOT EXISTS model_dependency_bindings (
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

            CREATE INDEX IF NOT EXISTS idx_model_dependency_bindings_model
              ON model_dependency_bindings(model_id, status, binding_kind, backend_key, priority, binding_id);

            CREATE TABLE IF NOT EXISTS dependency_binding_history (
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
            ",
        )?;

        Ok(())
    }

    fn migrate_model_type_rule_constraints(conn: &Connection) -> Result<()> {
        let arch_sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'table' AND name = 'model_type_arch_rules'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if arch_sql.as_deref().is_some_and(|sql| {
            let sql = sql.to_lowercase();
            !sql.contains("'reranker'") || !sql.contains("'vlm'")
        }) {
            Self::rebuild_model_type_arch_rules_table(conn)?;
        }

        let config_sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'table' AND name = 'model_type_config_rules'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if config_sql.as_deref().is_some_and(|sql| {
            let sql = sql.to_lowercase();
            !sql.contains("'reranker'") || !sql.contains("'vlm'")
        }) {
            Self::rebuild_model_type_config_rules_table(conn)?;
        }

        if config_sql.is_some() {
            let stale_multimodal_hint_rules: i64 = conn.query_row(
                "SELECT COUNT(*)
                 FROM model_type_config_rules
                 WHERE status = 'active'
                   AND (
                     (lower(config_model_type) = 'image-to-text' AND lower(model_type) != 'vlm')
                     OR (lower(config_model_type) = 'image-text-to-text' AND lower(model_type) != 'vlm')
                     OR (lower(config_model_type) = 'visual-question-answering' AND lower(model_type) != 'vlm')
                     OR (lower(config_model_type) = 'document-question-answering' AND lower(model_type) != 'vlm')
                     OR (lower(config_model_type) = 'video-text-to-text' AND lower(model_type) != 'vlm')
                   )",
                [],
                |row| row.get(0),
            )?;
            if stale_multimodal_hint_rules > 0 {
                Self::repair_multimodal_hint_rule_rows(conn)?;
            }
        }

        Ok(())
    }

    fn rebuild_model_type_arch_rules_table(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            ALTER TABLE model_type_arch_rules RENAME TO model_type_arch_rules_legacy;

            CREATE TABLE model_type_arch_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              pattern TEXT NOT NULL,
              match_style TEXT NOT NULL CHECK (match_style IN ('exact', 'prefix', 'suffix', 'wildcard')),
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'vlm', 'reranker', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            INSERT INTO model_type_arch_rules (
              id,
              pattern,
              match_style,
              model_type,
              priority,
              status,
              source,
              notes,
              created_at,
              updated_at
            )
            SELECT
              id,
              pattern,
              match_style,
              model_type,
              priority,
              status,
              source,
              notes,
              created_at,
              updated_at
            FROM model_type_arch_rules_legacy;

            DROP TABLE model_type_arch_rules_legacy;
            ",
        )?;

        Ok(())
    }

    fn rebuild_model_type_config_rules_table(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            ALTER TABLE model_type_config_rules RENAME TO model_type_config_rules_legacy;

            CREATE TABLE model_type_config_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              config_model_type TEXT NOT NULL,
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'vlm', 'reranker', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            INSERT INTO model_type_config_rules (
              id,
              config_model_type,
              model_type,
              priority,
              status,
              source,
              notes,
              created_at,
              updated_at
            )
            SELECT
              id,
              config_model_type,
              model_type,
              priority,
              status,
              source,
              notes,
              created_at,
              updated_at
            FROM model_type_config_rules_legacy;

            DROP TABLE model_type_config_rules_legacy;
            ",
        )?;

        Ok(())
    }

    fn repair_multimodal_hint_rule_rows(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            UPDATE model_type_config_rules
            SET model_type = 'vlm',
                priority = 60,
                status = 'active',
                source = 'system'
            WHERE lower(config_model_type) IN (
              'image-to-text',
              'image-text-to-text',
              'visual-question-answering',
              'document-question-answering',
              'video-text-to-text'
            );

            INSERT OR IGNORE INTO model_type_config_rules (config_model_type, model_type, priority, status, source) VALUES
              ('image-to-text', 'vlm', 60, 'active', 'system'),
              ('image-text-to-text', 'vlm', 60, 'active', 'system'),
              ('visual-question-answering', 'vlm', 60, 'active', 'system'),
              ('document-question-answering', 'vlm', 60, 'active', 'system'),
              ('video-text-to-text', 'vlm', 60, 'active', 'system');
            ",
        )?;

        Ok(())
    }

    /// Seed idempotent baseline rows for mapping/rule tables.
    pub(super) fn seed_metadata_v2_rows(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            INSERT OR IGNORE INTO task_signature_mappings (
              signature_key, mapping_version, input_modalities_json, output_modalities_json, task_type_primary, priority, status, source
            ) VALUES
              ('text->text', 1, '[\"text\"]', '[\"text\"]', 'text-generation', 100, 'active', 'system'),
              ('text->image', 1, '[\"text\"]', '[\"image\"]', 'text-to-image', 100, 'active', 'system'),
              ('image->image', 1, '[\"image\"]', '[\"image\"]', 'image-to-image', 100, 'active', 'system'),
              ('text+image->image', 1, '[\"text\",\"image\"]', '[\"image\"]', 'text-image-to-image', 100, 'active', 'system'),
              ('text->audio', 1, '[\"text\"]', '[\"audio\"]', 'text-to-audio', 100, 'active', 'system'),
              ('audio->audio', 1, '[\"audio\"]', '[\"audio\"]', 'audio-to-audio', 100, 'active', 'system'),
              ('audio->text', 1, '[\"audio\"]', '[\"text\"]', 'audio-to-text', 100, 'active', 'system'),
              ('text->embedding', 1, '[\"text\"]', '[\"embedding\"]', 'text-embedding', 100, 'active', 'system'),
              ('image->embedding', 1, '[\"image\"]', '[\"embedding\"]', 'image-embedding', 100, 'active', 'system'),
              ('audio->embedding', 1, '[\"audio\"]', '[\"embedding\"]', 'audio-embedding', 100, 'active', 'system'),
              ('image->text', 1, '[\"image\"]', '[\"text\"]', 'image-to-text', 100, 'active', 'system'),
              ('image->mask', 1, '[\"image\"]', '[\"mask\"]', 'image-segmentation', 100, 'active', 'system'),
              ('image->depth', 1, '[\"image\"]', '[\"depth\"]', 'depth-estimation', 100, 'active', 'system'),
              ('image->bbox', 1, '[\"image\"]', '[\"bbox\"]', 'object-detection', 100, 'active', 'system'),
              ('video->text', 1, '[\"video\"]', '[\"text\"]', 'video-to-text', 100, 'active', 'system'),
              ('text+document->text', 1, '[\"text\",\"document\"]', '[\"text\"]', 'text-ranking', 100, 'active', 'system'),
              ('text+image->text', 1, '[\"text\",\"image\"]', '[\"text\"]', 'visual-question-answering', 100, 'active', 'system'),
              ('text+video->text', 1, '[\"text\",\"video\"]', '[\"text\"]', 'video-question-answering', 100, 'active', 'system'),
              ('text->video', 1, '[\"text\"]', '[\"video\"]', 'text-to-video', 100, 'active', 'system'),
              ('text->3d', 1, '[\"text\"]', '[\"3d\"]', 'text-to-3d', 100, 'active', 'system'),
              ('image->3d', 1, '[\"image\"]', '[\"3d\"]', 'image-to-3d', 100, 'active', 'system');

            INSERT OR IGNORE INTO model_type_arch_rules (pattern, match_style, model_type, priority, status, source) VALUES
              ('ForCausalLM', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForMaskedLM', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForConditionalGeneration', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForSequenceClassification', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForTokenClassification', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForQuestionAnswering', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForSpeechSeq2Seq', 'suffix', 'audio', 100, 'active', 'system'),
              ('ForAudioClassification', 'suffix', 'audio', 100, 'active', 'system'),
              ('Whisper', 'prefix', 'audio', 100, 'active', 'system'),
              ('Encodec', 'prefix', 'audio', 100, 'active', 'system'),
              ('MossTTSDelayModel', 'exact', 'audio', 100, 'active', 'system'),
              ('ForImageClassification', 'suffix', 'vision', 100, 'active', 'system'),
              ('ForObjectDetection', 'suffix', 'vision', 100, 'active', 'system'),
              ('ForSemanticSegmentation', 'suffix', 'vision', 100, 'active', 'system'),
              ('ForImageSegmentation', 'suffix', 'vision', 100, 'active', 'system'),
              ('CLIPVisionModel', 'prefix', 'vision', 100, 'active', 'system'),
              ('VisionEncoderDecoderModel', 'exact', 'vlm', 90, 'active', 'system'),
              ('Florence', 'prefix', 'vlm', 90, 'active', 'system'),
              ('PaliGemma', 'prefix', 'vlm', 90, 'active', 'system'),
              ('Idefics', 'prefix', 'vlm', 90, 'active', 'system'),
              ('BLIP', 'prefix', 'vlm', 90, 'active', 'system'),
              ('Llava', 'prefix', 'vlm', 90, 'active', 'system'),
              ('UNet2DConditionModel', 'exact', 'diffusion', 100, 'active', 'system'),
              ('UNet2DModel', 'exact', 'diffusion', 100, 'active', 'system'),
              ('AutoencoderKL', 'exact', 'diffusion', 100, 'active', 'system'),
              ('VQModel', 'exact', 'diffusion', 100, 'active', 'system'),
              ('StableDiffusion*Pipeline', 'wildcard', 'diffusion', 100, 'active', 'system'),
              ('DiffusionPipeline', 'exact', 'diffusion', 100, 'active', 'system');

            INSERT OR IGNORE INTO model_type_config_rules (config_model_type, model_type, priority, status, source) VALUES
              ('llm', 'llm', 50, 'active', 'system'),
              ('vlm', 'vlm', 50, 'active', 'system'),
              ('reranker', 'reranker', 50, 'active', 'system'),
              ('diffusion', 'diffusion', 50, 'active', 'system'),
              ('embedding', 'embedding', 50, 'active', 'system'),
              ('audio', 'audio', 50, 'active', 'system'),
              ('vision', 'vision', 50, 'active', 'system'),
              ('unknown', 'unknown', 50, 'active', 'system'),
              ('text-generation', 'llm', 60, 'active', 'system'),
              ('text2text-generation', 'llm', 60, 'active', 'system'),
              ('question-answering', 'llm', 60, 'active', 'system'),
              ('token-classification', 'llm', 60, 'active', 'system'),
              ('text-classification', 'llm', 60, 'active', 'system'),
              ('fill-mask', 'llm', 60, 'active', 'system'),
              ('translation', 'llm', 60, 'active', 'system'),
              ('summarization', 'llm', 60, 'active', 'system'),
              ('conversational', 'llm', 60, 'active', 'system'),
              ('text-ranking', 'reranker', 60, 'active', 'system'),
              ('text-to-image', 'diffusion', 60, 'active', 'system'),
              ('image-to-image', 'diffusion', 60, 'active', 'system'),
              ('unconditional-image-generation', 'diffusion', 60, 'active', 'system'),
              ('image-inpainting', 'diffusion', 60, 'active', 'system'),
              ('text-to-video', 'diffusion', 60, 'active', 'system'),
              ('video-classification', 'vision', 60, 'active', 'system'),
              ('image-text-to-text', 'vlm', 60, 'active', 'system'),
              ('mask-generation', 'vision', 60, 'active', 'system'),
              ('text-to-3d', 'diffusion', 60, 'active', 'system'),
              ('image-to-3d', 'diffusion', 60, 'active', 'system'),
              ('text-to-audio', 'audio', 60, 'active', 'system'),
              ('text-to-speech', 'audio', 60, 'active', 'system'),
              ('automatic-speech-recognition', 'audio', 60, 'active', 'system'),
              ('audio-classification', 'audio', 60, 'active', 'system'),
              ('audio-to-audio', 'audio', 60, 'active', 'system'),
              ('voice-activity-detection', 'audio', 60, 'active', 'system'),
              ('image-classification', 'vision', 60, 'active', 'system'),
              ('image-segmentation', 'vision', 60, 'active', 'system'),
              ('object-detection', 'vision', 60, 'active', 'system'),
              ('zero-shot-image-classification', 'vision', 60, 'active', 'system'),
              ('depth-estimation', 'vision', 60, 'active', 'system'),
              ('image-feature-extraction', 'vision', 60, 'active', 'system'),
              ('zero-shot-object-detection', 'vision', 60, 'active', 'system'),
              ('image-to-text', 'vlm', 60, 'active', 'system'),
              ('visual-question-answering', 'vlm', 60, 'active', 'system'),
              ('document-question-answering', 'vlm', 60, 'active', 'system'),
              ('video-text-to-text', 'vlm', 60, 'active', 'system'),
              ('feature-extraction', 'embedding', 60, 'active', 'system'),
              ('sentence-similarity', 'embedding', 60, 'active', 'system'),
              ('llama', 'llm', 100, 'active', 'system'),
              ('mistral', 'llm', 100, 'active', 'system'),
              ('mixtral', 'llm', 100, 'active', 'system'),
              ('gpt2', 'llm', 100, 'active', 'system'),
              ('gpt_neo', 'llm', 100, 'active', 'system'),
              ('gpt_neox', 'llm', 100, 'active', 'system'),
              ('gptj', 'llm', 100, 'active', 'system'),
              ('phi', 'llm', 100, 'active', 'system'),
              ('phi3', 'llm', 100, 'active', 'system'),
              ('qwen2', 'llm', 100, 'active', 'system'),
              ('qwen2_vl', 'vlm', 95, 'active', 'system'),
              ('qwen2_5_vl', 'vlm', 95, 'active', 'system'),
              ('qwen3', 'llm', 100, 'active', 'system'),
              ('qwen3_vl', 'vlm', 95, 'active', 'system'),
              ('qwen3_5_vl', 'vlm', 95, 'active', 'system'),
              ('florence2', 'vlm', 95, 'active', 'system'),
              ('paligemma', 'vlm', 95, 'active', 'system'),
              ('idefics', 'vlm', 95, 'active', 'system'),
              ('gemma', 'llm', 100, 'active', 'system'),
              ('gemma2', 'llm', 100, 'active', 'system'),
              ('gemma3', 'llm', 100, 'active', 'system'),
              ('gemma3n', 'vlm', 95, 'active', 'system'),
              ('deepseek_v2', 'llm', 100, 'active', 'system'),
              ('deepseek_v3', 'llm', 100, 'active', 'system'),
              ('falcon', 'llm', 100, 'active', 'system'),
              ('mpt', 'llm', 100, 'active', 'system'),
              ('bloom', 'llm', 100, 'active', 'system'),
              ('opt', 'llm', 100, 'active', 'system'),
              ('codegen', 'llm', 100, 'active', 'system'),
              ('starcoder2', 'llm', 100, 'active', 'system'),
              ('rwkv', 'llm', 100, 'active', 'system'),
              ('rwkv5', 'llm', 100, 'active', 'system'),
              ('rwkv6', 'llm', 100, 'active', 'system'),
              ('mamba', 'llm', 100, 'active', 'system'),
              ('mamba2', 'llm', 100, 'active', 'system'),
              ('jamba', 'llm', 100, 'active', 'system'),
              ('dbrx', 'llm', 100, 'active', 'system'),
              ('stablelm', 'llm', 100, 'active', 'system'),
              ('stable_diffusion', 'diffusion', 100, 'active', 'system'),
              ('sdxl', 'diffusion', 100, 'active', 'system'),
              ('kandinsky', 'diffusion', 100, 'active', 'system'),
              ('pixart', 'diffusion', 100, 'active', 'system'),
              ('whisper', 'audio', 100, 'active', 'system'),
              ('wav2vec2', 'audio', 100, 'active', 'system'),
              ('hubert', 'audio', 100, 'active', 'system'),
              ('wavlm', 'audio', 100, 'active', 'system'),
              ('seamless_m4t', 'audio', 100, 'active', 'system'),
              ('bark', 'audio', 100, 'active', 'system'),
              ('musicgen', 'audio', 100, 'active', 'system'),
              ('encodec', 'audio', 100, 'active', 'system'),
              ('speecht5', 'audio', 100, 'active', 'system'),
              ('mms', 'audio', 100, 'active', 'system'),
              ('moss_tts_delay', 'audio', 100, 'active', 'system'),
              ('vit', 'vision', 100, 'active', 'system'),
              ('swin', 'vision', 100, 'active', 'system'),
              ('convnext', 'vision', 100, 'active', 'system'),
              ('deit', 'vision', 100, 'active', 'system'),
              ('beit', 'vision', 100, 'active', 'system'),
              ('dinov2', 'vision', 100, 'active', 'system'),
              ('clip', 'vision', 100, 'active', 'system'),
              ('siglip', 'vision', 100, 'active', 'system'),
              ('blip', 'vlm', 95, 'active', 'system'),
              ('blip2', 'vlm', 95, 'active', 'system'),
              ('sentence-transformers', 'embedding', 100, 'active', 'system'),
              ('bge', 'embedding', 100, 'active', 'system'),
              ('e5', 'embedding', 100, 'active', 'system'),
              ('gte', 'embedding', 100, 'active', 'system'),
              ('jina-embeddings', 'embedding', 100, 'active', 'system');
            ",
        )?;

        conn.execute_batch(
            "
            INSERT OR IGNORE INTO model_metadata_baselines (model_id, schema_version, baseline_json, created_at, created_by)
            SELECT
              m.id,
              COALESCE(CAST(json_extract(m.metadata_json, '$.schema_version') AS INTEGER), 1),
              m.metadata_json,
              m.updated_at,
              'pumas-library'
            FROM models m;

            INSERT INTO model_metadata_history (
              model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
            )
            SELECT
              b.model_id,
              NULL,
              'pumas-library',
              'baseline_created',
              NULL,
              NULL,
              b.baseline_json,
              'migration-backfill',
              b.created_at
            FROM model_metadata_baselines b
            WHERE NOT EXISTS (
              SELECT 1
              FROM model_metadata_history h
              WHERE h.model_id = b.model_id AND h.action = 'baseline_created'
            );
            ",
        )?;

        Ok(())
    }

    /// Set whether a model is excluded from linking for a given app.
    pub fn set_link_exclusion(&self, model_id: &str, app_id: &str, excluded: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        if excluded {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT OR IGNORE INTO model_link_exclusions (model_id, app_id, excluded_at)
                 VALUES (?1, ?2, ?3)",
                params![model_id, app_id, now],
            )?;
        } else {
            conn.execute(
                "DELETE FROM model_link_exclusions WHERE model_id = ?1 AND app_id = ?2",
                params![model_id, app_id],
            )?;
        }

        Ok(())
    }

    /// Check if a model is excluded from linking for a given app.
    pub fn is_link_excluded(&self, model_id: &str, app_id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM model_link_exclusions WHERE model_id = ?1 AND app_id = ?2",
            params![model_id, app_id],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    /// Get all excluded model IDs for a given app.
    pub fn get_excluded_model_ids(&self, app_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt =
            conn.prepare("SELECT model_id FROM model_link_exclusions WHERE app_id = ?1")?;
        let rows = stmt.query_map(params![app_id], |row| row.get(0))?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }

        Ok(ids)
    }

    /// Resolve an active task-signature mapping row.
    pub fn get_active_task_signature_mapping(
        &self,
        signature_key: &str,
    ) -> Result<Option<TaskSignatureMapping>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let row = conn
            .query_row(
                "SELECT
                    id,
                    signature_key,
                    mapping_version,
                    input_modalities_json,
                    output_modalities_json,
                    task_type_primary,
                    priority,
                    status,
                    source
                 FROM task_signature_mappings
                 WHERE status = 'active' AND signature_key = ?1
                 ORDER BY priority ASC, mapping_version DESC
                 LIMIT 1",
                params![signature_key],
                |row| {
                    let input_json: String = row.get(3)?;
                    let output_json: String = row.get(4)?;
                    Ok(TaskSignatureMapping {
                        id: row.get(0)?,
                        signature_key: row.get(1)?,
                        mapping_version: row.get(2)?,
                        input_modalities: serde_json::from_str(&input_json).unwrap_or_default(),
                        output_modalities: serde_json::from_str(&output_json).unwrap_or_default(),
                        task_type_primary: row.get(5)?,
                        priority: row.get(6)?,
                        status: row.get(7)?,
                        source: row.get(8)?,
                    })
                },
            )
            .optional()?;

        Ok(row)
    }

    /// Upsert the per-signature pending mapping row used for runtime discovery.
    pub fn upsert_pending_task_signature_mapping(
        &self,
        signature_key: &str,
        input_modalities: &[String],
        output_modalities: &[String],
    ) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let tx = conn.transaction()?;

        let existing_pending_id: Option<i64> = tx
            .query_row(
                "SELECT id
                 FROM task_signature_mappings
                 WHERE signature_key = ?1 AND status = 'pending'
                 LIMIT 1",
                params![signature_key],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(pending_id) = existing_pending_id {
            tx.execute(
                "UPDATE task_signature_mappings
                 SET
                   input_modalities_json = ?1,
                   output_modalities_json = ?2,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = ?3",
                params![
                    serde_json::to_string(input_modalities)?,
                    serde_json::to_string(output_modalities)?,
                    pending_id
                ],
            )?;
        } else {
            let next_version: i64 = tx.query_row(
                "SELECT COALESCE(MAX(mapping_version), 0) + 1
                 FROM task_signature_mappings
                 WHERE signature_key = ?1",
                params![signature_key],
                |row| row.get(0),
            )?;

            tx.execute(
                "INSERT INTO task_signature_mappings (
                   signature_key,
                   mapping_version,
                   input_modalities_json,
                   output_modalities_json,
                   task_type_primary,
                   priority,
                   status,
                   source,
                   notes
                 ) VALUES (?1, ?2, ?3, ?4, 'unknown', 100, 'pending', 'runtime-discovered', 'auto-staged unknown signature')",
                params![
                    signature_key,
                    next_version,
                    serde_json::to_string(input_modalities)?,
                    serde_json::to_string(output_modalities)?,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// List active architecture/class model-type resolver rules.
    pub fn list_active_model_type_arch_rules(&self) -> Result<Vec<ModelTypeArchRule>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT pattern, match_style, model_type, priority
             FROM model_type_arch_rules
             WHERE status = 'active'
             ORDER BY priority ASC, pattern ASC, match_style ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ModelTypeArchRule {
                pattern: row.get(0)?,
                match_style: row.get(1)?,
                model_type: row.get(2)?,
                priority: row.get(3)?,
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    /// List active config.model_type resolver rules.
    pub fn list_active_model_type_config_rules(&self) -> Result<Vec<ModelTypeConfigRule>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT config_model_type, model_type, priority
             FROM model_type_config_rules
             WHERE status = 'active'
             ORDER BY priority ASC, config_model_type ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ModelTypeConfigRule {
                config_model_type: row.get(0)?,
                model_type: row.get(1)?,
                priority: row.get(2)?,
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    /// Translate a raw model-type hint (HF pipeline tag, config model_type, or canonical type)
    /// to a canonical Pumas model type using active SQLite config rules.
    pub fn resolve_model_type_hint(&self, raw_hint: &str) -> Result<Option<String>> {
        let hint = raw_hint.trim().to_lowercase();
        if hint.is_empty() {
            return Ok(None);
        }

        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT model_type
             FROM model_type_config_rules
             WHERE status = 'active'
               AND lower(config_model_type) = ?1
             ORDER BY priority ASC
             LIMIT 1",
        )?;

        let mapped: Option<String> = stmt.query_row(params![hint], |row| row.get(0)).optional()?;

        Ok(mapped)
    }
}
