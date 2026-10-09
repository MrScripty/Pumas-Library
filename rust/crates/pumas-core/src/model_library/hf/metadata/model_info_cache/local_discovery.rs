//! Explicit offline browse/search of admitted public anonymous detail observations.
use super::{budget, budget_slots, invalid, parse_body};
use crate::{model_library::HfSearchParams, models::HuggingFaceModel, Result};
use chrono::Utc;

#[derive(serde::Deserialize)]
struct Visibility {
    private: Option<bool>,
    gated: Option<bool>,
    disabled: Option<bool>,
}

impl super::HuggingFaceClient {
    /// Search or browse retained public, ungated details without HTTP or hydration.
    /// Empty query browses; all whitespace-separated tokens must match the local
    /// repository/name/task/tags projection. Results are ordered by repository ID.
    /// `model_card.pumas_discovery` states source, observation freshness and revision;
    /// that revision is advisory and never supplies acquisition authority.
    pub async fn search_cached_model_details(
        &self,
        params: &HfSearchParams,
    ) -> Result<Vec<HuggingFaceModel>> {
        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);
        if params.query.len() > 256
            || params.kind.as_ref().is_some_and(|kind| kind.len() > 128)
            || params
                .format
                .as_ref()
                .is_some_and(|format| format.len() > 128)
            || limit > 100
            || offset > budget::RECORD_LIMIT
        {
            return Err(invalid("HF detail discovery query exceeds capacity"));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let params = params.clone();
        let source_prefix = format!("{}/models/", self.api_base_url());
        let root = self.cache_dir.clone();
        let guard = budget_slots().acquire(&root)?.lock_owned().await;
        self.store_lifetime
            .spawn_blocking(move || {
                let _admission = guard;
                let tokens: Vec<_> = params.query.to_lowercase().split_whitespace()
                    .map(str::to_owned).collect();
                let now = Utc::now();
                let mut models = Vec::new();
                budget::visit_observations(&root, |observation| {
                    let Some(repo) = observation.url.strip_prefix(&source_prefix) else {
                        return Ok(());
                    };
                    let (Some(body), Some(observed_at)) = (&observation.body, observation.validated_at) else {
                        return Ok(());
                    };
                    // Legacy detail records did not record visibility explicitly.
                    // Unknown/private/gated/disabled records cannot enter discovery.
                    // Typed fields reject duplicates instead of last-field-wins.
                    let Ok(visibility) = serde_json::from_str::<Visibility>(body.get()) else {
                        return Ok(());
                    };
                    if visibility.private != Some(false)
                        || visibility.gated != Some(false)
                        || visibility.disabled == Some(true)
                    {
                        return Ok(());
                    }
                    let parsed = parse_body(body, repo)?;
                    let tags = parsed.model.tags.join(" ");
                    let mut model = Self::convert_search_result(parsed.model);
                    let searchable = format!("{} {} {} {} {}", model.repo_id, model.name,
                        model.developer, model.kind, tags).to_lowercase();
                    let kind = params.kind.as_deref().map(|kind| match kind {
                        "llm" => "text-generation", "reranker" => "text-ranking",
                        "diffusion" => "text-to-image", "audio" => "automatic-speech-recognition",
                        other => other,
                    });
                    if tokens.iter().any(|token| !searchable.contains(token))
                        || kind.is_some_and(|kind| model.kind != kind)
                        || params.format.as_ref().is_some_and(|format| !model.formats.contains(format))
                    {
                        return Ok(());
                    }
                    let fresh = observed_at <= now && (now - observed_at).num_seconds() < observation.ttl_seconds as i64;
                    model.model_card.get_or_insert_with(Default::default).insert(
                        "pumas_discovery".into(),
                        serde_json::json!({
                            "source": "anonymous-hf-detail-cache",
                            "source_url": observation.url,
                            "observed_at": observed_at,
                            "freshness": if fresh { "fresh" } else { "stale" },
                            "fresh_until": observed_at.checked_add_signed(chrono::TimeDelta::seconds(observation.ttl_seconds as i64)),
                            "revision_observed": parsed.sha,
                            "discovery_only": true,
                            "visibility_observed": "public-ungated",
                        }),
                    );
                    model.compatible_engines.clear();
                    // Conversion is shared; no tree hydration or inferred runtime grant.
                    models.push(model);
                    Ok(())
                })?;
                models.sort_by(|a, b| a.repo_id.cmp(&b.repo_id));
                Ok(models.into_iter().skip(offset).take(limit).collect())
            })
            .await
            .map_err(|_| invalid("HF detail discovery effect failed"))?
    }
}
