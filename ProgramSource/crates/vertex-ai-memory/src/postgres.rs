use crate::{
    CreateMemory, MemoryCategory, MemoryError, MemoryId, MemoryPrivacy, MemoryQuery, MemoryRecord,
    MemoryRepository, MemoryScope, ScopeType, UpdateMemory,
};
use async_trait::async_trait;
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::time::Duration;
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct PostgresMemoryRepository {
    pool: PgPool,
}

impl PostgresMemoryRepository {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, MemoryError> {
        if database_url.trim().is_empty() || max_connections == 0 {
            return Err(MemoryError::Invalid(
                "database URL and positive connection count are required".to_owned(),
            ));
        }
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    /// Connects without constructing a URL, so managed credentials never need
    /// to be serialized into configuration or logs.
    pub async fn connect_with_options(
        options: PgConnectOptions,
        max_connections: u32,
    ) -> Result<Self, MemoryError> {
        if max_connections == 0 {
            return Err(MemoryError::Invalid(
                "positive connection count is required".to_owned(),
            ));
        }
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<(), MemoryError> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl MemoryRepository for PostgresMemoryRepository {
    async fn find_equivalent(
        &self,
        memory: &CreateMemory,
    ) -> Result<Option<MemoryRecord>, MemoryError> {
        memory.validate()?;
        let normalized = crate::domain::normalize_content(&memory.content);
        let row = sqlx::query(
            r#"
            SELECT * FROM vertex_ai_memory.memories
            WHERE scope_type = $1
              AND organization_id IS NOT DISTINCT FROM $2
              AND user_id IS NOT DISTINCT FROM $3
              AND application_id IS NOT DISTINCT FROM $4
              AND project_id IS NOT DISTINCT FROM $5
              AND session_id IS NOT DISTINCT FROM $6
              AND category = $7
              AND normalized_content = $8
            LIMIT 1
            "#,
        )
        .bind(memory.scope.scope_type.as_str())
        .bind(memory.scope.organization_id)
        .bind(memory.scope.user_id)
        .bind(memory.scope.application_id)
        .bind(memory.scope.project_id)
        .bind(memory.scope.session_id)
        .bind(memory.category.as_str())
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(map_record).transpose()
    }

    async fn create(&self, memory: CreateMemory) -> Result<MemoryRecord, MemoryError> {
        memory.validate()?;
        let memory_id = MemoryId::new();
        let normalized_content = crate::domain::normalize_content(&memory.content);
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            INSERT INTO vertex_ai_memory.memories (
                memory_id, category, scope_type, organization_id, user_id,
                application_id, project_id, session_id, owner_id, content,
                structured_content, priority, confidence, source, expires_at,
                local_only, cloud_allowed, sensitive, share_scope, metadata,
                normalized_content
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
            )
            ON CONFLICT (scope_type, organization_id, user_id, application_id,
                         project_id, session_id, category, normalized_content)
            DO UPDATE SET updated_at = vertex_ai_memory.memories.updated_at
            RETURNING *, (xmax = 0) AS inserted
            "#,
        )
        .bind(memory_id.0)
        .bind(memory.category.as_str())
        .bind(memory.scope.scope_type.as_str())
        .bind(memory.scope.organization_id)
        .bind(memory.scope.user_id)
        .bind(memory.scope.application_id)
        .bind(memory.scope.project_id)
        .bind(memory.scope.session_id)
        .bind(memory.owner_id)
        .bind(memory.content)
        .bind(memory.structured_content)
        .bind(memory.priority)
        .bind(memory.confidence)
        .bind(memory.source)
        .bind(memory.expires_at)
        .bind(memory.privacy.local_only)
        .bind(memory.privacy.cloud_allowed)
        .bind(memory.privacy.sensitive)
        .bind(memory.privacy.share_scope)
        .bind(memory.metadata)
        .bind(normalized_content)
        .fetch_one(&mut *transaction)
        .await?;
        let returned_id = MemoryId(row.try_get("memory_id")?);
        let action = if row.try_get::<bool, _>("inserted")? {
            "created"
        } else {
            "deduplicated"
        };
        insert_audit(&mut transaction, returned_id, action, memory.owner_id).await?;
        transaction.commit().await?;
        map_record(&row)
    }

    async fn get(
        &self,
        id: MemoryId,
        scope: &MemoryScope,
    ) -> Result<Option<MemoryRecord>, MemoryError> {
        scope.validate()?;
        let row = sqlx::query(
            r#"
            SELECT * FROM vertex_ai_memory.memories
            WHERE memory_id = $1 AND scope_type = $2
              AND organization_id IS NOT DISTINCT FROM $3
              AND user_id IS NOT DISTINCT FROM $4
              AND application_id IS NOT DISTINCT FROM $5
              AND project_id IS NOT DISTINCT FROM $6
              AND session_id IS NOT DISTINCT FROM $7
            "#,
        )
        .bind(id.0)
        .bind(scope.scope_type.as_str())
        .bind(scope.organization_id)
        .bind(scope.user_id)
        .bind(scope.application_id)
        .bind(scope.project_id)
        .bind(scope.session_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(map_record).transpose()
    }

    async fn update(
        &self,
        id: MemoryId,
        scope: &MemoryScope,
        update: UpdateMemory,
    ) -> Result<MemoryRecord, MemoryError> {
        scope.validate()?;
        update.validate()?;
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            UPDATE vertex_ai_memory.memories SET
                content = $9, structured_content = $10, priority = $11,
                confidence = $12, expires_at = $13, local_only = $14,
                cloud_allowed = $15, sensitive = $16, share_scope = $17,
                metadata = $18, updated_at = now(), version = version + 1
            WHERE memory_id = $1 AND scope_type = $2
              AND organization_id IS NOT DISTINCT FROM $3
              AND user_id IS NOT DISTINCT FROM $4
              AND application_id IS NOT DISTINCT FROM $5
              AND project_id IS NOT DISTINCT FROM $6
              AND session_id IS NOT DISTINCT FROM $7
              AND version = $8
            RETURNING *
            "#,
        )
        .bind(id.0)
        .bind(scope.scope_type.as_str())
        .bind(scope.organization_id)
        .bind(scope.user_id)
        .bind(scope.application_id)
        .bind(scope.project_id)
        .bind(scope.session_id)
        .bind(update.expected_version)
        .bind(update.content)
        .bind(update.structured_content)
        .bind(update.priority)
        .bind(update.confidence)
        .bind(update.expires_at)
        .bind(update.privacy.local_only)
        .bind(update.privacy.cloud_allowed)
        .bind(update.privacy.sensitive)
        .bind(update.privacy.share_scope)
        .bind(update.metadata)
        .fetch_optional(&mut *transaction)
        .await?;
        let row = match row {
            Some(row) => row,
            None => {
                let exists = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM vertex_ai_memory.memories
                        WHERE memory_id = $1 AND scope_type = $2
                          AND organization_id IS NOT DISTINCT FROM $3
                          AND user_id IS NOT DISTINCT FROM $4
                          AND application_id IS NOT DISTINCT FROM $5
                          AND project_id IS NOT DISTINCT FROM $6
                          AND session_id IS NOT DISTINCT FROM $7
                    )
                    "#,
                )
                .bind(id.0)
                .bind(scope.scope_type.as_str())
                .bind(scope.organization_id)
                .bind(scope.user_id)
                .bind(scope.application_id)
                .bind(scope.project_id)
                .bind(scope.session_id)
                .fetch_one(&mut *transaction)
                .await?;
                return Err(if exists {
                    MemoryError::Conflict
                } else {
                    MemoryError::NotFound
                });
            }
        };
        insert_audit(&mut transaction, id, "updated", None).await?;
        transaction.commit().await?;
        map_record(&row)
    }

    async fn delete(&self, id: MemoryId, scope: &MemoryScope) -> Result<bool, MemoryError> {
        scope.validate()?;
        let mut transaction = self.pool.begin().await?;
        let deleted = sqlx::query_scalar::<_, Uuid>(
            r#"
            DELETE FROM vertex_ai_memory.memories
            WHERE memory_id = $1 AND scope_type = $2
              AND organization_id IS NOT DISTINCT FROM $3
              AND user_id IS NOT DISTINCT FROM $4
              AND application_id IS NOT DISTINCT FROM $5
              AND project_id IS NOT DISTINCT FROM $6
              AND session_id IS NOT DISTINCT FROM $7
            RETURNING memory_id
            "#,
        )
        .bind(id.0)
        .bind(scope.scope_type.as_str())
        .bind(scope.organization_id)
        .bind(scope.user_id)
        .bind(scope.application_id)
        .bind(scope.project_id)
        .bind(scope.session_id)
        .fetch_optional(&mut *transaction)
        .await?
        .is_some();
        if deleted {
            insert_audit(&mut transaction, id, "deleted", None).await?;
        }
        transaction.commit().await?;
        Ok(deleted)
    }

    async fn search(&self, query: MemoryQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        query.validate()?;
        let rows = sqlx::query(
            r#"
            SELECT * FROM vertex_ai_memory.memories
            WHERE scope_type = $1
              AND organization_id IS NOT DISTINCT FROM $2
              AND user_id IS NOT DISTINCT FROM $3
              AND application_id IS NOT DISTINCT FROM $4
              AND project_id IS NOT DISTINCT FROM $5
              AND session_id IS NOT DISTINCT FROM $6
              AND ($7::text IS NULL OR category = $7)
              AND ($8::text IS NULL OR search_vector @@ plainto_tsquery('simple', $8))
              AND ($9 OR expires_at IS NULL OR expires_at > now())
            ORDER BY
              CASE WHEN $8::text IS NULL THEN 0
                   ELSE ts_rank(search_vector, plainto_tsquery('simple', $8)) END DESC,
              priority DESC, confidence DESC, updated_at DESC
            LIMIT $10
            "#,
        )
        .bind(query.scope.scope_type.as_str())
        .bind(query.scope.organization_id)
        .bind(query.scope.user_id)
        .bind(query.scope.application_id)
        .bind(query.scope.project_id)
        .bind(query.scope.session_id)
        .bind(query.category.map(MemoryCategory::as_str))
        .bind(query.text)
        .bind(query.include_expired)
        .bind(i64::from(query.limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_record).collect()
    }
}

async fn insert_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    memory_id: MemoryId,
    action: &str,
    actor_id: Option<Uuid>,
) -> Result<(), MemoryError> {
    sqlx::query(
        "INSERT INTO vertex_ai_memory.memory_audit (memory_id, action, actor_id) VALUES ($1, $2, $3)",
    )
    .bind(memory_id.0)
    .bind(action)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn map_record(row: &sqlx::postgres::PgRow) -> Result<MemoryRecord, MemoryError> {
    Ok(MemoryRecord {
        memory_id: MemoryId(row.try_get("memory_id")?),
        category: MemoryCategory::parse(row.try_get("category")?)?,
        scope: MemoryScope {
            scope_type: ScopeType::parse(row.try_get("scope_type")?)?,
            organization_id: row.try_get("organization_id")?,
            user_id: row.try_get("user_id")?,
            application_id: row.try_get("application_id")?,
            project_id: row.try_get("project_id")?,
            session_id: row.try_get("session_id")?,
        },
        owner_id: row.try_get("owner_id")?,
        content: row.try_get("content")?,
        structured_content: row.try_get("structured_content")?,
        priority: row.try_get("priority")?,
        confidence: row.try_get("confidence")?,
        source: row.try_get("source")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        expires_at: row.try_get("expires_at")?,
        privacy: MemoryPrivacy {
            local_only: row.try_get("local_only")?,
            cloud_allowed: row.try_get("cloud_allowed")?,
            sensitive: row.try_get("sensitive")?,
            share_scope: row.try_get("share_scope")?,
        },
        metadata: row.try_get("metadata")?,
        version: row.try_get("version")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    #[ignore = "requires VERTEX_AI_TEST_DATABASE_URL"]
    async fn postgres_migration_and_crud_when_test_database_is_configured() {
        let database_url = std::env::var("VERTEX_AI_TEST_DATABASE_URL")
            .expect("VERTEX_AI_TEST_DATABASE_URL must be configured");
        let repository = PostgresMemoryRepository::connect(&database_url, 2)
            .await
            .expect("connect test PostgreSQL");
        repository.migrate().await.expect("run migrations");
        let scope = MemoryScope::project(Uuid::new_v4(), Uuid::new_v4());
        let record = repository
            .create(CreateMemory {
                category: MemoryCategory::Knowledge,
                scope: scope.clone(),
                owner_id: None,
                content: "Project Alpha uses PostgreSQL as its Memory Engine.".to_owned(),
                structured_content: json!({}),
                priority: 0.9,
                confidence: 1.0,
                source: "integration-test".to_owned(),
                expires_at: None,
                privacy: MemoryPrivacy::default(),
                metadata: json!({}),
            })
            .await
            .expect("create memory");
        let results = repository
            .search(MemoryQuery {
                scope: scope.clone(),
                text: Some("PostgreSQL".to_owned()),
                category: None,
                include_expired: false,
                limit: 10,
            })
            .await
            .expect("search memory");
        assert_eq!(results.len(), 1);
        assert!(repository.delete(record.memory_id, &scope).await.unwrap());
    }
}
