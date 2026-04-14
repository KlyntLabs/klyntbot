use std::sync::Arc;

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skills_marketplace::AdaptedSkillsRepo;
use skills_registry::{SkillPackage, TemplateFile};

use crate::prompt::render_prompt;

pub const SUPPORTED_FIELD_TYPES: &[&str] = &[
    "text",
    "number",
    "select",
    "multi_select",
    "date",
    "checkbox",
    "url",
    "email",
    "phone",
    "relation",
    "rollup",
    "formula",
    "created_time",
    "last_edited",
    "files",
    "person",
];

#[async_trait::async_trait]
pub trait AdapterProvider: Send + Sync {
    /// Expected: returns the JSON string matching the prompt output schema.
    async fn generate(&self, prompt: &str) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AdapterOutput {
    pub adaptable: bool,
    #[serde(default)]
    pub adapted_skill_md: String,
    #[serde(default)]
    pub generated_templates: Vec<TemplateFile>,
    pub rationale: String,
}

pub struct AdaptedSkill {
    pub adapted_skill_md: String,
    pub generated_templates: Vec<TemplateFile>,
    pub rationale: String,
    pub adapter_model: String,
}

pub struct Adapter {
    pub provider: Arc<dyn AdapterProvider>,
    pub cache: AdaptedSkillsRepo,
    pub model_name: String,
}

impl Adapter {
    pub async fn adapt(
        &self,
        pkg: &SkillPackage,
        current_databases: &[(String, String)],
    ) -> Result<AdaptedSkill> {
        let cache_key = compute_cache_key(pkg, &self.model_name);
        if let Some(cached) = self.cache.get(&cache_key).await? {
            tracing::debug!(skill = %pkg.name, "adapter cache hit");
            return Ok(AdaptedSkill {
                adapted_skill_md: cached.adapted_skill_md,
                generated_templates: serde_json::from_value(cached.generated_templates)
                    .map_err(|e| KlyntbotError::Storage(e.to_string()))?,
                rationale: cached.rationale,
                adapter_model: cached.adapter_model,
            });
        }
        let prompt = render_prompt(
            &pkg.skill_md_content,
            SUPPORTED_FIELD_TYPES,
            current_databases,
        );
        let raw = self.provider.generate(&prompt).await?;
        let parsed: AdapterOutput = serde_json::from_str(&raw)
            .map_err(|e| KlyntbotError::Storage(format!("adapter JSON parse: {e}")))?;
        if !parsed.adaptable {
            return Err(KlyntbotError::Storage(format!(
                "skill not adaptable: {}",
                parsed.rationale
            )));
        }
        validate_templates(&parsed.generated_templates)?;
        self.cache
            .upsert(
                &cache_key,
                &parsed.adapted_skill_md,
                &serde_json::to_value(&parsed.generated_templates).unwrap(),
                &parsed.rationale,
                &self.model_name,
            )
            .await?;
        Ok(AdaptedSkill {
            adapted_skill_md: parsed.adapted_skill_md,
            generated_templates: parsed.generated_templates,
            rationale: parsed.rationale,
            adapter_model: self.model_name.clone(),
        })
    }
}

fn compute_cache_key(pkg: &SkillPackage, model: &str) -> String {
    let mut h = Sha256::new();
    h.update(pkg.resolved_sha.as_bytes());
    h.update(b"|");
    h.update(model.as_bytes());
    hex::encode(h.finalize())
}

fn validate_templates(templates: &[TemplateFile]) -> Result<()> {
    for t in templates {
        let dbs = t
            .manifest
            .get("databases")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                KlyntbotError::Storage(format!("template {} missing databases[]", t.name))
            })?;
        if dbs.len() > 3 {
            return Err(KlyntbotError::Storage(format!(
                "template {}: max 3 databases",
                t.name
            )));
        }
        for db in dbs {
            let fields = db
                .get("fields")
                .and_then(|v| v.as_array())
                .ok_or_else(|| KlyntbotError::Storage("database missing fields[]".into()))?;
            for f in fields {
                let ft = f
                    .get("fieldType")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| KlyntbotError::Storage("field missing fieldType".into()))?;
                if !SUPPORTED_FIELD_TYPES.contains(&ft) {
                    return Err(KlyntbotError::Storage(format!(
                        "unsupported fieldType: {ft}"
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use skill_system::store::SkillFrontmatter;
    use skills_marketplace::{AdaptedSkillsRepo, SkillsMarketplaceFeature};
    use skills_registry::source::{GitRef, SkillSource};

    struct FixedProvider(&'static str);

    #[async_trait::async_trait]
    impl AdapterProvider for FixedProvider {
        async fn generate(&self, _prompt: &str) -> Result<String> {
            Ok(self.0.into())
        }
    }

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &SkillsMarketplaceFeature::migrations(),
        )
        .await
        .unwrap();
        pool.inner().clone()
    }

    fn make_pkg() -> SkillPackage {
        SkillPackage {
            name: "x".into(),
            source: SkillSource::Github {
                owner: "a".into(),
                repo: "b".into(),
                subpath: "c".into(),
                r#ref: GitRef::Latest,
            },
            resolved_sha: "sha".into(),
            semver: None,
            skill_md_content: "---\nname: x\ndescription: d\n---\nbody".into(),
            frontmatter: SkillFrontmatter {
                name: "x".into(),
                description: "d".into(),
                when_to_use: None,
                references: vec![],
            },
            klyntbot_meta: None,
            references: vec![],
            templates: vec![],
        }
    }

    #[tokio::test]
    async fn adapts_and_caches() {
        let pool: sqlx::SqlitePool = setup_pool().await;
        let adapter = Adapter {
            provider: Arc::new(FixedProvider(
                r#"{"adaptable":true,"adapted_skill_md":"---\nname: x\n---\nbody","generated_templates":[],"rationale":"ok"}"#,
            )),
            cache: AdaptedSkillsRepo::new(pool.clone()),
            model_name: "test".into(),
        };
        let out = adapter.adapt(&make_pkg(), &[]).await.unwrap();
        assert_eq!(out.rationale, "ok");

        // Second call: cache hit, same output
        let out2 = adapter.adapt(&make_pkg(), &[]).await.unwrap();
        assert_eq!(out2.rationale, "ok");
    }

    #[tokio::test]
    async fn rejects_unsupported_field_type() {
        let bad = json!([{"name":"t.json","manifest":{"databases":[{"fields":[{"fieldType":"whatever"}]}]}}]);
        let output =
            json!({"adaptable":true,"adapted_skill_md":"x","generated_templates": bad, "rationale":""})
                .to_string();
        let pool: sqlx::SqlitePool = setup_pool().await;
        let adapter = Adapter {
            provider: Arc::new(FixedProvider(Box::leak(output.into_boxed_str()))),
            cache: AdaptedSkillsRepo::new(pool),
            model_name: "test".into(),
        };
        assert!(adapter.adapt(&make_pkg(), &[]).await.is_err());
    }
}
