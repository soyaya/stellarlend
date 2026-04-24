-- Create events table with optimized indexes for query performance
-- Migration: 20240101000000_create_events_table_with_indexes.sql

-- Create the events table
CREATE TABLE IF NOT EXISTS events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_address VARCHAR(64) NOT NULL,
    event_name VARCHAR(255) NOT NULL,
    block_number BIGINT NOT NULL,
    transaction_hash VARCHAR(64) NOT NULL,
    log_index INTEGER NOT NULL,
    event_data JSONB NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure uniqueness of events within a transaction
    UNIQUE(transaction_hash, log_index)
);

-- Create indexes for optimal query performance

-- Primary query pattern: events by contract address and block range
-- Used for filtering events by contract and time period
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_contract_block
ON events (contract_address, block_number DESC);

-- Query pattern: events by event name and block range
-- Used for filtering specific event types within time periods
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_name_block
ON events (event_name, block_number DESC);

-- Query pattern: events by transaction hash (for transaction details)
-- Used when looking up all events from a specific transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_tx_hash
ON events (transaction_hash);

-- Composite index for contract + event name + block range queries
-- Used for complex filtering (e.g., "all Transfer events from contract X in blocks Y-Z")
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_contract_name_block
ON events (contract_address, event_name, block_number DESC);

-- Index for time-based queries (recent events, time ranges)
-- Used for chronological event browsing and cleanup operations
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_indexed_at
ON events (indexed_at DESC);

-- Partial index for recent events (last 30 days) - optimizes common "recent activity" queries
-- Reduces index size and improves performance for frequently accessed recent data
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_recent
ON events (block_number DESC, indexed_at DESC)
WHERE indexed_at > NOW() - INTERVAL '30 days';

-- JSONB index for event data queries (if needed for specific field searches)
-- Allows efficient querying of JSON fields within event_data
-- Example: events where event_data->>'from' = 'some_address'
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_data_gin
ON events USING GIN (event_data);

-- Index for pagination queries (block_number + log_index for consistent ordering)
-- Ensures stable ordering for pagination across blocks
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_block_log
ON events (block_number DESC, log_index DESC);

-- Create indexing metadata table for tracking sync progress
CREATE TABLE IF NOT EXISTS indexing_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_address VARCHAR(64) NOT NULL UNIQUE,
    last_indexed_block BIGINT NOT NULL DEFAULT 0,
    last_indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for metadata queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_indexing_metadata_contract
ON indexing_metadata (contract_address);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_indexing_metadata_block
ON indexing_metadata (last_indexed_block DESC);

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create trigger for automatic updated_at updates
CREATE TRIGGER update_indexing_metadata_updated_at
    BEFORE UPDATE ON indexing_metadata
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();