//! Headless Vertex AI Core: configuration, command dispatch, and lifecycle boundaries.

mod command;
mod config;
mod error;
mod logging;
mod service;

pub use command::{Command, CommandResponse};
pub use config::{CoreConfig, RuntimeEnvironment};
pub use error::CoreError;
pub use logging::init_logging;
pub use service::VertexAiCore;
