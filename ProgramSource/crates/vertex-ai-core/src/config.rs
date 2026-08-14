use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEnvironment {
    Development,
    Test,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreConfig {
    pub service_name: String,
    pub environment: RuntimeEnvironment,
    pub log_filter: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            service_name: "vertex-ai-core".to_owned(),
            environment: RuntimeEnvironment::Development,
            log_filter: "info".to_owned(),
        }
    }
}

impl CoreConfig {
    pub fn from_env() -> Self {
        let environment = match std::env::var("VERTEX_AI_ENV").as_deref() {
            Ok("production") => RuntimeEnvironment::Production,
            Ok("test") => RuntimeEnvironment::Test,
            _ => RuntimeEnvironment::Development,
        };
        Self {
            service_name: std::env::var("VERTEX_AI_SERVICE_NAME")
                .unwrap_or_else(|_| "vertex-ai-core".to_owned()),
            environment,
            log_filter: std::env::var("VERTEX_AI_LOG").unwrap_or_else(|_| "info".to_owned()),
        }
    }
}
