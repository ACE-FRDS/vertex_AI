use tracing_subscriber::EnvFilter;

pub fn init_logging(
    default_filter: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init()
}
