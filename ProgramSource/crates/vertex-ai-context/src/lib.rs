//! Provider-neutral context construction with strict privacy and token budgets.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use thiserror::Error;
use vertex_ai_memory::{MemoryCategory, MemoryQuery, MemoryRepository, MemoryScope};
use vertex_ai_types::{PrivacyPolicy, VertexContext};

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("invalid context request: {0}")]
    Invalid(String),
    #[error("memory retrieval failed")]
    MemoryUnavailable,
    #[error("base context exceeds the available token budget")]
    BudgetExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetLocation {
    Local,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBuildRequest {
    pub base: VertexContext,
    pub scope: MemoryScope,
    pub query: String,
    pub target_location: TargetLocation,
    pub allow_sensitive: bool,
    pub max_context_tokens: u32,
    pub reserved_output_tokens: u32,
    pub memory_candidate_limit: u32,
}

impl ContextBuildRequest {
    fn validate(&self) -> Result<(), ContextError> {
        self.scope
            .validate()
            .map_err(|_| ContextError::Invalid("invalid memory scope".to_owned()))?;
        if self.query.trim().is_empty() {
            return Err(ContextError::Invalid("query cannot be empty".to_owned()));
        }
        if self.max_context_tokens == 0
            || self.reserved_output_tokens >= self.max_context_tokens
            || self.memory_candidate_limit == 0
            || self.memory_candidate_limit > 100
        {
            return Err(ContextError::Invalid(
                "invalid context or candidate budget".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBuildReport {
    pub candidate_count: usize,
    pub included_count: usize,
    pub excluded_privacy_count: usize,
    pub excluded_budget_count: usize,
    pub estimated_context_tokens: u32,
    pub available_context_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuiltContext {
    pub prepared: PreparedContext,
    pub report: ContextBuildReport,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreparedContext {
    context: VertexContext,
    target_location: TargetLocation,
}

impl PreparedContext {
    /// Creates a context that is guaranteed to stay on the local machine.
    pub fn local(mut context: VertexContext) -> Self {
        context.privacy_policy = PrivacyPolicy {
            local_only: true,
            cloud_allowed: false,
            sensitive: context.privacy_policy.sensitive,
            share_scope: None,
        };
        Self {
            context,
            target_location: TargetLocation::Local,
        }
    }

    pub fn context(&self) -> &VertexContext {
        &self.context
    }

    pub fn target_location(&self) -> TargetLocation {
        self.target_location
    }

    pub fn into_context(self) -> VertexContext {
        self.context
    }
}

pub struct ContextBuilder {
    memories: Arc<dyn MemoryRepository>,
}

impl ContextBuilder {
    pub fn new(memories: Arc<dyn MemoryRepository>) -> Self {
        Self { memories }
    }

    pub async fn build(&self, request: ContextBuildRequest) -> Result<BuiltContext, ContextError> {
        request.validate()?;
        if request.target_location == TargetLocation::Cloud
            && base_contains_payload(&request.base)
            && (request.base.privacy_policy.local_only
                || !request.base.privacy_policy.cloud_allowed)
        {
            return Err(ContextError::Invalid(
                "base context privacy policy forbids cloud processing".to_owned(),
            ));
        }
        let mut context = request.base;
        context.memories.clear();
        context.decisions.clear();

        let available_context_tokens = request
            .max_context_tokens
            .saturating_sub(request.reserved_output_tokens);
        let base_tokens = estimate_json_tokens(&context);
        if base_tokens > available_context_tokens {
            return Err(ContextError::BudgetExceeded);
        }

        let candidates = self
            .memories
            .search(MemoryQuery {
                scope: request.scope,
                text: Some(request.query),
                category: None,
                include_expired: false,
                limit: request.memory_candidate_limit,
            })
            .await
            .map_err(|_| ContextError::MemoryUnavailable)?;
        let candidate_count = candidates.len();
        let mut used_tokens = base_tokens;
        let mut included_count = 0;
        let mut excluded_privacy_count = 0;
        let mut excluded_budget_count = 0;

        for memory in candidates {
            if !privacy_allows(
                &memory.privacy,
                request.target_location,
                request.allow_sensitive,
            ) {
                excluded_privacy_count += 1;
                continue;
            }
            let value = json!({
                "memory_id": memory.memory_id.0,
                "type": memory.category,
                "content": memory.content,
                "structured_content": memory.structured_content,
                "priority": memory.priority,
                "confidence": memory.confidence,
                "source": memory.source,
                "updated_at": memory.updated_at,
                "metadata": memory.metadata
            });
            let item_tokens = estimate_json_tokens(&value);
            if used_tokens.saturating_add(item_tokens) > available_context_tokens {
                excluded_budget_count += 1;
                continue;
            }
            used_tokens += item_tokens;
            if memory.category == MemoryCategory::Decision {
                context.decisions.push(value);
            } else {
                context.memories.push(value);
            }
            included_count += 1;
        }

        context.privacy_policy = match request.target_location {
            TargetLocation::Cloud => PrivacyPolicy {
                local_only: false,
                cloud_allowed: true,
                sensitive: request.allow_sensitive,
                share_scope: None,
            },
            TargetLocation::Local => PrivacyPolicy {
                local_only: true,
                cloud_allowed: false,
                sensitive: request.allow_sensitive,
                share_scope: None,
            },
        };
        Ok(BuiltContext {
            prepared: PreparedContext {
                context,
                target_location: request.target_location,
            },
            report: ContextBuildReport {
                candidate_count,
                included_count,
                excluded_privacy_count,
                excluded_budget_count,
                estimated_context_tokens: used_tokens,
                available_context_tokens,
            },
        })
    }
}

fn privacy_allows(
    privacy: &vertex_ai_memory::MemoryPrivacy,
    target: TargetLocation,
    allow_sensitive: bool,
) -> bool {
    if privacy.sensitive && !allow_sensitive {
        return false;
    }
    match target {
        TargetLocation::Local => true,
        TargetLocation::Cloud => !privacy.local_only && privacy.cloud_allowed,
    }
}

fn estimate_json_tokens(value: &impl Serialize) -> u32 {
    let bytes = serde_json::to_vec(value).map_or(0, |json| json.len());
    // Conservative provider-neutral estimate. Provider tokenizers can refine this later.
    u32::try_from(bytes.div_ceil(4)).unwrap_or(u32::MAX)
}

fn base_contains_payload(context: &VertexContext) -> bool {
    let non_empty_object =
        |value: &serde_json::Value| value.as_object().is_none_or(|object| !object.is_empty());
    non_empty_object(&context.task)
        || non_empty_object(&context.application)
        || non_empty_object(&context.project)
        || non_empty_object(&context.user_context)
        || !context.memories.is_empty()
        || !context.decisions.is_empty()
        || !context.constraints.is_empty()
        || non_empty_object(&context.schema)
        || !context.tools.is_empty()
        || non_empty_object(&context.permissions)
        || non_empty_object(&context.runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;
    use vertex_ai_memory::{CreateMemory, InMemoryMemoryRepository, MemoryPrivacy};

    async fn add_memory(
        repository: &InMemoryMemoryRepository,
        scope: MemoryScope,
        content: &str,
        privacy: MemoryPrivacy,
    ) {
        repository
            .create(CreateMemory {
                category: MemoryCategory::Knowledge,
                scope,
                owner_id: None,
                content: content.to_owned(),
                structured_content: json!({}),
                priority: 0.8,
                confidence: 0.9,
                source: "test".to_owned(),
                expires_at: None,
                privacy,
                metadata: json!({}),
            })
            .await
            .unwrap();
    }

    fn request(scope: MemoryScope, target_location: TargetLocation) -> ContextBuildRequest {
        ContextBuildRequest {
            base: VertexContext::default(),
            scope,
            query: "PostgreSQL".to_owned(),
            target_location,
            allow_sensitive: false,
            max_context_tokens: 2_000,
            reserved_output_tokens: 500,
            memory_candidate_limit: 10,
        }
    }

    #[tokio::test]
    async fn cloud_context_excludes_local_only_memory() {
        let repository = Arc::new(InMemoryMemoryRepository::default());
        let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
        add_memory(
            &repository,
            scope.clone(),
            "PostgreSQL local secret",
            MemoryPrivacy {
                local_only: true,
                ..MemoryPrivacy::default()
            },
        )
        .await;
        add_memory(
            &repository,
            scope.clone(),
            "PostgreSQL public architecture",
            MemoryPrivacy {
                cloud_allowed: true,
                ..MemoryPrivacy::default()
            },
        )
        .await;

        let built = ContextBuilder::new(repository)
            .build(request(scope, TargetLocation::Cloud))
            .await
            .unwrap();
        assert_eq!(built.report.included_count, 1);
        assert_eq!(built.report.excluded_privacy_count, 1);
        assert!(built.prepared.context().privacy_policy.cloud_allowed);
        assert!(
            !built.prepared.context().memories[0]["content"]
                .as_str()
                .unwrap()
                .contains("secret")
        );
    }

    #[tokio::test]
    async fn memory_items_never_exceed_context_budget() {
        let repository = Arc::new(InMemoryMemoryRepository::default());
        let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
        add_memory(
            &repository,
            scope.clone(),
            &format!("PostgreSQL {}", "x".repeat(2_000)),
            MemoryPrivacy::default(),
        )
        .await;
        let mut build_request = request(scope, TargetLocation::Local);
        build_request.max_context_tokens = 400;
        build_request.reserved_output_tokens = 100;
        let built = ContextBuilder::new(repository)
            .build(build_request)
            .await
            .unwrap();
        assert_eq!(built.report.included_count, 0);
        assert_eq!(built.report.excluded_budget_count, 1);
        assert!(built.report.estimated_context_tokens <= built.report.available_context_tokens);
    }

    #[tokio::test]
    async fn cloud_build_cannot_launder_protected_base_context() {
        let repository = Arc::new(InMemoryMemoryRepository::default());
        let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
        let mut build_request = request(scope, TargetLocation::Cloud);
        build_request.base.project = json!({"private": "local project state"});
        let result = ContextBuilder::new(repository).build(build_request).await;
        assert!(matches!(result, Err(ContextError::Invalid(_))));
    }
}
#[test]
fn local_prepared_context_forces_a_local_only_boundary() {
    let mut context = VertexContext::default();
    context.privacy_policy.cloud_allowed = true;
    let prepared = PreparedContext::local(context);
    assert_eq!(prepared.target_location(), TargetLocation::Local);
    assert!(prepared.context().privacy_policy.local_only);
    assert!(!prepared.context().privacy_policy.cloud_allowed);
}
