//! Durable, provider-neutral Vertex Memory domain and PostgreSQL repository.

mod domain;
mod error;
mod postgres;
mod repository;
mod service;

pub use domain::{
    CreateMemory, MemoryCategory, MemoryId, MemoryPrivacy, MemoryQuery, MemoryRecord, MemoryScope,
    ScopeType, UpdateMemory,
};
pub use error::MemoryError;
pub use postgres::PostgresMemoryRepository;
pub use repository::{InMemoryMemoryRepository, MemoryRepository};
pub use service::{MemoryProposal, MemoryService, MemoryWritePermit};
