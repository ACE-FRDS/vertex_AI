use crate::{CreateMemory, MemoryError, MemoryId, MemoryQuery, MemoryRecord, UpdateMemory};
use async_trait::async_trait;
use chrono::Utc;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn find_equivalent(
        &self,
        memory: &CreateMemory,
    ) -> Result<Option<MemoryRecord>, MemoryError>;
    async fn create(&self, memory: CreateMemory) -> Result<MemoryRecord, MemoryError>;
    async fn get(
        &self,
        id: MemoryId,
        scope: &crate::MemoryScope,
    ) -> Result<Option<MemoryRecord>, MemoryError>;
    async fn update(
        &self,
        id: MemoryId,
        scope: &crate::MemoryScope,
        update: UpdateMemory,
    ) -> Result<MemoryRecord, MemoryError>;
    async fn delete(&self, id: MemoryId, scope: &crate::MemoryScope) -> Result<bool, MemoryError>;
    async fn search(&self, query: MemoryQuery) -> Result<Vec<MemoryRecord>, MemoryError>;
}

#[derive(Debug, Default)]
pub struct InMemoryMemoryRepository {
    records: RwLock<HashMap<MemoryId, MemoryRecord>>,
}

impl InMemoryMemoryRepository {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl MemoryRepository for InMemoryMemoryRepository {
    async fn find_equivalent(
        &self,
        memory: &CreateMemory,
    ) -> Result<Option<MemoryRecord>, MemoryError> {
        memory.validate()?;
        let normalized = crate::domain::normalize_content(&memory.content);
        Ok(self
            .records
            .read()
            .await
            .values()
            .find(|record| {
                record.scope == memory.scope
                    && record.category == memory.category
                    && crate::domain::normalize_content(&record.content) == normalized
            })
            .cloned())
    }

    async fn create(&self, memory: CreateMemory) -> Result<MemoryRecord, MemoryError> {
        memory.validate()?;
        let normalized = crate::domain::normalize_content(&memory.content);
        let mut records = self.records.write().await;
        if let Some(record) = records.values().find(|record| {
            record.scope == memory.scope
                && record.category == memory.category
                && crate::domain::normalize_content(&record.content) == normalized
        }) {
            return Ok(record.clone());
        }
        let now = Utc::now();
        let record = MemoryRecord {
            memory_id: MemoryId::new(),
            category: memory.category,
            scope: memory.scope,
            owner_id: memory.owner_id,
            content: memory.content,
            structured_content: memory.structured_content,
            priority: memory.priority,
            confidence: memory.confidence,
            source: memory.source,
            created_at: now,
            updated_at: now,
            expires_at: memory.expires_at,
            privacy: memory.privacy,
            metadata: memory.metadata,
            version: 1,
        };
        records.insert(record.memory_id, record.clone());
        Ok(record)
    }

    async fn get(
        &self,
        id: MemoryId,
        scope: &crate::MemoryScope,
    ) -> Result<Option<MemoryRecord>, MemoryError> {
        scope.validate()?;
        Ok(self
            .records
            .read()
            .await
            .get(&id)
            .filter(|record| &record.scope == scope)
            .cloned())
    }

    async fn update(
        &self,
        id: MemoryId,
        scope: &crate::MemoryScope,
        update: UpdateMemory,
    ) -> Result<MemoryRecord, MemoryError> {
        scope.validate()?;
        update.validate()?;
        let mut records = self.records.write().await;
        let record = records
            .get_mut(&id)
            .filter(|record| &record.scope == scope)
            .ok_or(MemoryError::NotFound)?;
        if record.version != update.expected_version {
            return Err(MemoryError::Conflict);
        }
        record.content = update.content;
        record.structured_content = update.structured_content;
        record.priority = update.priority;
        record.confidence = update.confidence;
        record.expires_at = update.expires_at;
        record.privacy = update.privacy;
        record.metadata = update.metadata;
        record.updated_at = Utc::now();
        record.version += 1;
        Ok(record.clone())
    }

    async fn delete(&self, id: MemoryId, scope: &crate::MemoryScope) -> Result<bool, MemoryError> {
        scope.validate()?;
        let mut records = self.records.write().await;
        if records
            .get(&id)
            .is_some_and(|record| &record.scope == scope)
        {
            records.remove(&id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn search(&self, query: MemoryQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        query.validate()?;
        let now = Utc::now();
        let search_text = query.text.as_deref().map(str::to_lowercase);
        let mut records: Vec<_> = self
            .records
            .read()
            .await
            .values()
            .filter(|record| record.scope == query.scope)
            .filter(|record| {
                query
                    .category
                    .is_none_or(|category| record.category == category)
            })
            .filter(|record| {
                query.include_expired || record.expires_at.is_none_or(|expires_at| expires_at > now)
            })
            .filter(|record| {
                search_text
                    .as_ref()
                    .is_none_or(|text| record.content.to_lowercase().contains(text))
            })
            .cloned()
            .collect();
        records.sort_by(|left, right| {
            right
                .priority
                .total_cmp(&left.priority)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        records.truncate(query.limit as usize);
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryCategory, MemoryPrivacy, MemoryScope};
    use serde_json::json;
    use uuid::Uuid;

    fn create(scope: MemoryScope, content: &str) -> CreateMemory {
        CreateMemory {
            category: MemoryCategory::Knowledge,
            scope,
            owner_id: None,
            content: content.to_owned(),
            structured_content: json!({}),
            priority: 0.8,
            confidence: 0.9,
            source: "test".to_owned(),
            expires_at: None,
            privacy: MemoryPrivacy::default(),
            metadata: json!({}),
        }
    }

    #[tokio::test]
    async fn records_do_not_cross_project_scope() {
        let repository = InMemoryMemoryRepository::default();
        let application_id = Uuid::new_v4();
        let alpha = MemoryScope::project(application_id, Uuid::new_v4());
        let beta = MemoryScope::project(application_id, Uuid::new_v4());
        let record = repository
            .create(create(alpha.clone(), "Project Alpha uses PostgreSQL"))
            .await
            .unwrap();

        assert!(
            repository
                .get(record.memory_id, &alpha)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            repository
                .get(record.memory_id, &beta)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            repository
                .search(MemoryQuery {
                    scope: beta,
                    text: Some("PostgreSQL".to_owned()),
                    category: None,
                    include_expired: false,
                    limit: 10,
                })
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn update_uses_optimistic_versioning() {
        let repository = InMemoryMemoryRepository::default();
        let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
        let record = repository
            .create(create(scope.clone(), "v1"))
            .await
            .unwrap();
        let update = UpdateMemory {
            expected_version: 1,
            content: "v2".to_owned(),
            structured_content: json!({}),
            priority: 0.8,
            confidence: 0.9,
            expires_at: None,
            privacy: MemoryPrivacy::default(),
            metadata: json!({}),
        };
        let updated = repository
            .update(record.memory_id, &scope, update.clone())
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert!(matches!(
            repository.update(record.memory_id, &scope, update).await,
            Err(MemoryError::Conflict)
        ));
    }

    #[tokio::test]
    async fn exact_content_is_deduplicated_within_scope_and_category() {
        let repository = InMemoryMemoryRepository::default();
        let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
        let first = repository
            .create(create(scope.clone(), "Project Alpha uses PostgreSQL"))
            .await
            .unwrap();
        let duplicate = repository
            .create(create(scope, "  project   alpha USES postgresql "))
            .await
            .unwrap();
        assert_eq!(first.memory_id, duplicate.memory_id);
    }
}
