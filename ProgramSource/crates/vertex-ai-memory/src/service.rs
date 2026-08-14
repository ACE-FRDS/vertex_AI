use crate::{CreateMemory, MemoryError, MemoryRecord, MemoryRepository, MemoryScope};
use std::sync::Arc;
use uuid::Uuid;

/// An untrusted candidate. Model output remains a proposal until policy approval.
#[derive(Debug, Clone)]
pub struct MemoryProposal {
    pub candidate: CreateMemory,
}

/// Capability issued by a trusted authorization boundary, never by an LLM.
#[derive(Debug, Clone)]
pub struct MemoryWritePermit {
    pub actor_id: Option<Uuid>,
    pub scope: MemoryScope,
    pub allow_sensitive: bool,
}

pub struct MemoryService {
    repository: Arc<dyn MemoryRepository>,
}

impl MemoryService {
    pub fn new(repository: Arc<dyn MemoryRepository>) -> Self {
        Self { repository }
    }

    pub async fn approve_and_store(
        &self,
        proposal: MemoryProposal,
        permit: &MemoryWritePermit,
    ) -> Result<MemoryRecord, MemoryError> {
        proposal.candidate.validate()?;
        permit.scope.validate()?;
        if proposal.candidate.scope != permit.scope {
            return Err(MemoryError::Invalid(
                "memory proposal exceeds permitted scope".to_owned(),
            ));
        }
        if proposal.candidate.privacy.sensitive && !permit.allow_sensitive {
            return Err(MemoryError::Invalid(
                "memory proposal lacks sensitive-memory permission".to_owned(),
            ));
        }
        let mut candidate = proposal.candidate;
        candidate.owner_id = permit.actor_id;
        if let Some(existing) = self.repository.find_equivalent(&candidate).await? {
            return Ok(existing);
        }
        self.repository.create(candidate).await
    }

    pub fn repository(&self) -> &Arc<dyn MemoryRepository> {
        &self.repository
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryMemoryRepository, MemoryCategory, MemoryPrivacy};
    use serde_json::json;

    #[tokio::test]
    async fn proposal_cannot_cross_permitted_scope() {
        let service = MemoryService::new(InMemoryMemoryRepository::shared());
        let application_id = Uuid::new_v4();
        let permitted = MemoryScope::project(application_id, Uuid::new_v4());
        let different = MemoryScope::project(application_id, Uuid::new_v4());
        let result = service
            .approve_and_store(
                MemoryProposal {
                    candidate: CreateMemory {
                        category: MemoryCategory::Knowledge,
                        scope: different,
                        owner_id: None,
                        content: "must stay isolated".to_owned(),
                        structured_content: json!({}),
                        priority: 0.5,
                        confidence: 0.5,
                        source: "test".to_owned(),
                        expires_at: None,
                        privacy: MemoryPrivacy::default(),
                        metadata: json!({}),
                    },
                },
                &MemoryWritePermit {
                    actor_id: None,
                    scope: permitted,
                    allow_sensitive: false,
                },
            )
            .await;
        assert!(matches!(result, Err(MemoryError::Invalid(_))));
    }
}
