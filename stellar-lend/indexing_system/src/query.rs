use crate::cache::CacheService;
use crate::error::IndexerResult;
use crate::models::{Event, EventQuery, EventStats};
use crate::repository::EventRepository;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Service for querying events with intelligent caching
pub struct QueryService {
    /// Database repository
    repository: EventRepository,

    /// Cache service
    cache: Arc<RwLock<CacheService>>,
}

impl QueryService {
    /// Create a new query service
    ///
    /// # Arguments
    /// * `repository` - Database repository
    /// * `cache` - Cache service
    pub fn new(repository: EventRepository, cache: CacheService) -> Self {
        Self {
            repository,
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    /// Query events with caching
    ///
    /// This method first checks the cache, then falls back to database
    /// and caches the results for future queries.
    ///
    /// # Arguments
    /// * `query` - Query parameters
    ///
    /// # Returns
    /// Vector of matching events
    pub async fn query_events(&self, query: EventQuery) -> IndexerResult<Vec<Event>> {
        // Generate cache key from query parameters
        let cache_key = self.generate_query_hash(&query);

        // Try to get from cache first
        let mut cache = self.cache.write().await;
        if let Some(cached_events) = cache.get_cached_query(&cache_key).await? {
            debug!("Cache hit for query: {}", cache_key);
            return Ok(cached_events);
        }
        drop(cache);

        // Cache miss - query database
        debug!("Cache miss for query: {}", cache_key);
        let events = self.repository.query_events(query).await?;

        // Cache the results
        let mut cache = self.cache.write().await;
        cache.cache_query_results(&cache_key, &events).await?;

        Ok(events)
    }

    /// Get event by ID with caching
    ///
    /// # Arguments
    /// * `id` - Event UUID
    ///
    /// # Returns
    /// The event if found
    pub async fn get_event(&self, id: Uuid) -> IndexerResult<Option<Event>> {
        let id_str = id.to_string();

        // Try cache first
        let mut cache = self.cache.write().await;
        if let Some(event) = cache.get_cached_event(&id_str).await? {
            debug!("Cache hit for event: {}", id_str);
            return Ok(Some(event));
        }
        drop(cache);

        // Cache miss - query database
        debug!("Cache miss for event: {}", id_str);
        if let Some(event) = self.repository.get_event(id).await? {
            // Cache the event
            let mut cache = self.cache.write().await;
            cache.cache_event(&event).await?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// Get events by transaction hash
    ///
    /// # Arguments
    /// * `tx_hash` - Transaction hash
    ///
    /// # Returns
    /// Vector of events from this transaction
    pub async fn get_events_by_transaction(&self, tx_hash: &str) -> IndexerResult<Vec<Event>> {
        // For transaction lookups, we query directly without caching
        // as these are typically one-time lookups
        self.repository.get_events_by_transaction(tx_hash).await
    }

    /// Get event statistics with caching
    ///
    /// Statistics are heavily cached as they're expensive to compute
    /// and frequently accessed by dashboards.
    ///
    /// # Returns
    /// Aggregated event statistics
    pub async fn get_statistics(&self) -> IndexerResult<EventStats> {
        // Try cache first
        let mut cache = self.cache.write().await;
        if let Some(stats) = cache.get_cached_stats().await? {
            debug!("Cache hit for statistics");
            return Ok(stats);
        }
        drop(cache);

        // Cache miss - compute from database
        debug!("Cache miss for statistics");
        let stats = self.repository.get_event_stats().await?;

        // Cache the statistics
        let mut cache = self.cache.write().await;
        cache.cache_stats(&stats).await?;

        Ok(stats)
    }

    /// Get events for a specific contract with pagination
    ///
    /// # Arguments
    /// * `contract_address` - Contract address
    /// * `limit` - Maximum number of events to return
    /// * `offset` - Number of events to skip
    ///
    /// # Returns
    /// Vector of events
    pub async fn get_contract_events(
        &self,
        contract_address: &str,
        limit: i64,
        offset: i64,
    ) -> IndexerResult<Vec<Event>> {
        let query = EventQuery::new()
            .with_contract(contract_address.to_string())
            .with_pagination(limit, offset);

        self.query_events(query).await
    }

    /// Get recent events across all contracts
    ///
    /// # Arguments
    /// * `limit` - Maximum number of events to return
    ///
    /// # Returns
    /// Most recent events
    pub async fn get_recent_events(&self, limit: i64) -> IndexerResult<Vec<Event>> {
        let query = EventQuery::new().with_pagination(limit, 0);
        self.query_events(query).await
    }

    /// Get events by name with block range
    ///
    /// # Arguments
    /// * `event_name` - Name of the event (e.g., "Transfer")
    /// * `from_block` - Start block
    /// * `to_block` - End block
    ///
    /// # Returns
    /// Matching events
    pub async fn get_events_by_name(
        &self,
        event_name: &str,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> IndexerResult<Vec<Event>> {
        let mut query = EventQuery::new().with_event_name(event_name.to_string());

        if let (Some(from), Some(to)) = (from_block, to_block) {
            query = query.with_block_range(from, to);
        }

        self.query_events(query).await
    }

    /// Get latest indexed block number from cache
    ///
    /// This is a fast way to check indexing progress without
    /// hitting the database.
    ///
    /// # Returns
    /// Latest block number if cached
    pub async fn get_latest_block(&self) -> IndexerResult<Option<u64>> {
        let mut cache = self.cache.write().await;
        cache.get_latest_block().await
    }

    /// Generate a hash for query parameters to use as cache key
    ///
    /// This creates a unique, deterministic key based on query params
    fn generate_query_hash(&self, query: &EventQuery) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash all query parameters
        if let Some(ref addr) = query.contract_address {
            addr.hash(&mut hasher);
        }
        if let Some(ref name) = query.event_name {
            name.hash(&mut hasher);
        }
        query.from_block.hash(&mut hasher);
        query.to_block.hash(&mut hasher);
        query.limit.hash(&mut hasher);
        query.offset.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Invalidate all query caches
    ///
    /// This should be called when new events are indexed
    pub async fn invalidate_cache(&self) -> IndexerResult<()> {
        let mut cache = self.cache.write().await;
        cache.invalidate_queries().await?;
        cache.invalidate_stats().await?;
        Ok(())
    }

    /// Get event count for a contract
    ///
    /// # Arguments
    /// * `contract_address` - Contract address
    ///
    /// # Returns
    /// Number of events for this contract
    pub async fn get_event_count(&self, contract_address: &str) -> IndexerResult<i64> {
        // This would typically use a COUNT query for efficiency
        // For now, we'll fetch and count (not optimal for production)
        let query = EventQuery::new()
            .with_contract(contract_address.to_string())
            .with_pagination(10000, 0); // Large limit

        let events = self.query_events(query).await?;
        Ok(events.len() as i64)
    }

    /// Prefetch and cache commonly accessed data
    ///
    /// This can be called periodically to warm up the cache
    pub async fn warm_cache(&self) -> IndexerResult<()> {
        info!("Warming up cache");

        // Cache recent events
        let recent = self.get_recent_events(100).await?;
        debug!("Cached {} recent events", recent.len());

        // Cache statistics
        let _stats = self.get_statistics().await?;
        debug!("Cached statistics");

        Ok(())
    }

    /// Get recent events for a specific contract (optimized for performance)
    ///
    /// Uses the covering index for recent events to minimize database load.
    ///
    /// # Arguments
    /// * `contract_address` - Contract address
    /// * `limit` - Maximum number of events to return (default: 50)
    ///
    /// # Returns
    /// Recent events for the contract
    pub async fn get_recent_contract_events(
        &self,
        contract_address: &str,
        limit: Option<u32>,
    ) -> IndexerResult<Vec<Event>> {
        let limit = limit.unwrap_or(50);
        let cache_key = format!("recent_contract:{}:{}", contract_address, limit);

        // Try cache first
        let mut cache = self.cache.write().await;
        if let Some(events) = cache.get_cached_query(&cache_key).await? {
            debug!("Cache hit for recent contract events: {}", contract_address);
            return Ok(events);
        }
        drop(cache);

        // Cache miss - use optimized query
        debug!("Cache miss for recent contract events: {}", contract_address);
        let events = self.repository.get_recent_events_by_contract(contract_address, limit).await?;

        // Cache the results
        let mut cache = self.cache.write().await;
        cache.cache_query_results(&cache_key, &events).await?;

        Ok(events)
    }

    /// Get events by type and block range (optimized query)
    ///
    /// Uses composite indexes for efficient filtering.
    ///
    /// # Arguments
    /// * `contract_address` - Contract address
    /// * `event_name` - Event name
    /// * `from_block` - Start block
    /// * `to_block` - End block
    ///
    /// # Returns
    /// Matching events
    pub async fn get_typed_events_in_range(
        &self,
        contract_address: &str,
        event_name: &str,
        from_block: u64,
        to_block: u64,
    ) -> IndexerResult<Vec<Event>> {
        let cache_key = format!("typed_range:{}:{}:{}-{}",
                               contract_address, event_name, from_block, to_block);

        // Try cache first
        let mut cache = self.cache.write().await;
        if let Some(events) = cache.get_cached_query(&cache_key).await? {
            debug!("Cache hit for typed events in range: {} {}", contract_address, event_name);
            return Ok(events);
        }
        drop(cache);

        // Cache miss - use optimized query
        debug!("Cache miss for typed events in range: {} {}", contract_address, event_name);
        let events = self.repository.get_events_by_type_and_range(
            contract_address, event_name, from_block, to_block
        ).await?;

        // Cache the results
        let mut cache = self.cache.write().await;
        cache.cache_query_results(&cache_key, &events).await?;

        Ok(events)
    }

    /// Get event count statistics using materialized view (fast)
    ///
    /// # Arguments
    /// * `contract_address` - Optional contract filter
    ///
    /// # Returns
    /// Event count statistics
    pub async fn get_event_count_statistics(
        &self,
        contract_address: Option<&str>,
    ) -> IndexerResult<Vec<(String, String, i64)>> {
        let cache_key = format!("count_stats:{}", contract_address.unwrap_or("all"));

        // Try cache first
        let mut cache = self.cache.write().await;
        if let Some(stats) = cache.get_cached_count_stats(&cache_key).await? {
            debug!("Cache hit for event count statistics");
            return Ok(stats);
        }
        drop(cache);

        // Cache miss - use materialized view
        debug!("Cache miss for event count statistics");
        let stats = self.repository.get_event_count_stats(contract_address).await?;

        // Cache the results
        let mut cache = self.cache.write().await;
        cache.cache_count_stats(&cache_key, &stats).await?;

        Ok(stats)
    }

    /// Analyze index performance and usage
    ///
    /// This provides insights into which indexes are being used effectively.
    ///
    /// # Returns
    /// Index performance statistics
    pub async fn analyze_index_usage(&self) -> IndexerResult<Vec<(String, String, i64)>> {
        // This is a diagnostic query, not cached as it should reflect current state
        self.repository.analyze_index_performance().await
    }

    /// Optimize query performance by warming up frequently accessed data
    ///
    /// This method pre-loads commonly accessed data into cache.
    pub async fn optimize_performance(&self) -> IndexerResult<()> {
        info!("Running performance optimization");

        // Warm up recent events for popular contracts
        // In a real implementation, you'd determine popular contracts from usage patterns
        let popular_contracts = vec!["contract1", "contract2"]; // Placeholder

        for contract in popular_contracts {
            let _events = self.get_recent_contract_events(contract, Some(20)).await?;
            debug!("Warmed up cache for contract: {}", contract);
        }

        // Refresh materialized view statistics
        self.repository.refresh_event_statistics().await?;

        // Analyze table for query planner
        sqlx::query("ANALYZE events")
            .execute(&self.repository.pool)
            .await?;

        info!("Performance optimization completed");
        Ok(())
    }
}
