pub mod cache;
pub mod config;
pub mod error;
pub mod indexer;
pub mod models;
pub mod parser;
pub mod query;
pub mod repository;

pub use cache::CacheService;
pub use config::*;
pub use error::{IndexerError, IndexerResult};
pub use indexer::IndexerService;
pub use models::{
    CreateEvent, Event, EventQuery, EventStats, EventUpdate, IndexingMetadata, UpdateType,
};
pub use parser::{create_erc20_abi, EventParser};
pub use query::QueryService;
pub use repository::EventRepository;

pub fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Health check for all services
pub struct HealthCheck {
    pub database: bool,
    pub cache: bool,
    pub blockchain: bool,
}

impl HealthCheck {
    /// Check if all services are healthy
    pub fn is_healthy(&self) -> bool {
        self.database && self.cache && self.blockchain
    }
}

/// Run database migrations
///
/// This should be called before starting the indexer service
pub async fn run_migrations(database_url: &str) -> IndexerResult<()> {
    use sqlx::postgres::PgPoolOptions;

    info!("Running database migrations...");

    // Create connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .map_err(|e| IndexerError::Database(e))?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| IndexerError::Database(e))?;

    info!("Database migrations completed successfully");
    Ok(())
}

/// Initialize all services with proper setup
///
/// This is the main initialization function that should be called
/// to set up the entire indexing system.
pub async fn initialize_system(config: &Config) -> IndexerResult<(
    EventRepository,
    CacheService,
    QueryService,
    IndexerService,
)> {
    info!("Initializing indexing system...");

    // Run database migrations first
    run_migrations(&config.database.url).await?;

    // Create database connection pool
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .connect(&config.database.url)
        .await
        .map_err(|e| IndexerError::Database(e))?;

    // Create repository
    let repository = EventRepository::new(pool);

    // Create cache service
    let cache = CacheService::new(
        &config.cache.url,
        config.cache.event_ttl,
        config.cache.stats_ttl,
        config.cache.query_ttl,
    )
    .await?;

    // Create query service
    let query_service = QueryService::new(repository.clone(), cache.clone()).await?;

    // Create indexer service
    let indexer = IndexerService::new(config.clone(), repository.clone(), cache.clone()).await?;

    info!("Indexing system initialized successfully");
    Ok((repository, cache, query_service, indexer))
}
