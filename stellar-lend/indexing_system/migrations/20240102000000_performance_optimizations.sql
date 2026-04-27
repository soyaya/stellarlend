-- Performance optimizations and index maintenance
-- Migration: 20240102000000_performance_optimizations.sql

-- Create a partial index for high-frequency events to reduce index bloat
-- Only index events from the last 7 days for faster recent queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_recent_high_freq
ON events (contract_address, event_name, block_number DESC)
WHERE indexed_at > NOW() - INTERVAL '7 days';

-- Create covering index for common query patterns
-- This allows index-only scans for queries that only need these columns
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_covering_contract_name_block
ON events (contract_address, event_name, block_number DESC, transaction_hash, log_index)
WHERE indexed_at > NOW() - INTERVAL '90 days';

-- Create index for event statistics queries
-- Optimized for counting events by type and contract
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_stats
ON events (contract_address, event_name, block_number DESC)
WHERE block_number > 0; -- Exclude any potential invalid blocks

-- Create a BRIN index for block_number for large datasets
-- BRIN indexes are smaller and more efficient for sequential data like block numbers
-- Only create if the table is expected to be very large (> 100M rows)
-- Uncomment the following lines if you have massive data:
-- CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_block_brin
-- ON events USING BRIN (block_number)
-- WHERE block_number > 1000000; -- Only for higher block numbers

-- Create function for index maintenance and statistics
CREATE OR REPLACE FUNCTION get_event_index_stats()
RETURNS TABLE (
    index_name TEXT,
    index_size TEXT,
    table_size TEXT,
    index_usage BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        i.indexname::TEXT,
        pg_size_pretty(pg_relation_size(i.indexname::regclass))::TEXT,
        pg_size_pretty(pg_relation_size('events'))::TEXT,
        COALESCE(s.idx_scan, 0)::BIGINT
    FROM pg_indexes i
    LEFT JOIN pg_stat_user_indexes s ON i.indexname = s.indexname
    WHERE i.tablename = 'events'
    ORDER BY s.idx_scan DESC NULLS LAST;
END;
$$ LANGUAGE plpgsql;

-- Create function to analyze query performance
CREATE OR REPLACE FUNCTION analyze_event_queries()
RETURNS TABLE (
    query_pattern TEXT,
    estimated_rows BIGINT,
    actual_rows BIGINT,
    execution_time INTERVAL
) AS $$
BEGIN
    -- This function would be used with EXPLAIN ANALYZE
    -- For now, return empty result set
    RETURN QUERY SELECT
        'contract_events'::TEXT,
        0::BIGINT,
        0::BIGINT,
        '0 seconds'::INTERVAL
    WHERE FALSE;
END;
$$ LANGUAGE plpgsql;

-- Create view for event statistics (materialized for performance)
CREATE MATERIALIZED VIEW IF NOT EXISTS event_statistics AS
SELECT
    contract_address,
    event_name,
    COUNT(*) as event_count,
    MIN(block_number) as first_block,
    MAX(block_number) as last_block,
    MIN(indexed_at) as first_indexed,
    MAX(indexed_at) as last_indexed
FROM events
GROUP BY contract_address, event_name
WITH DATA;

-- Create indexes on the materialized view
CREATE INDEX IF NOT EXISTS idx_event_stats_contract
ON event_statistics (contract_address);

CREATE INDEX IF NOT EXISTS idx_event_stats_name
ON event_statistics (event_name);

CREATE INDEX IF NOT EXISTS idx_event_stats_count
ON event_statistics (event_count DESC);

-- Create function to refresh event statistics
CREATE OR REPLACE FUNCTION refresh_event_statistics()
RETURNS VOID AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY event_statistics;
END;
$$ LANGUAGE plpgsql;

-- Create a maintenance function for index cleanup
CREATE OR REPLACE FUNCTION cleanup_old_event_indexes()
RETURNS VOID AS $$
DECLARE
    cutoff_date TIMESTAMPTZ := NOW() - INTERVAL '1 year';
BEGIN
    -- Reindex concurrently to reduce bloat
    REINDEX TABLE CONCURRENTLY events;

    -- Analyze table for query planner
    ANALYZE events;

    -- Log maintenance operation
    RAISE NOTICE 'Event index maintenance completed at %', NOW();
END;
$$ LANGUAGE plpgsql;

-- Create trigger to automatically refresh statistics (throttled)
-- This runs periodically, not on every insert
CREATE OR REPLACE FUNCTION trigger_refresh_event_stats()
RETURNS TRIGGER AS $$
DECLARE
    last_refresh TIMESTAMPTZ;
BEGIN
    -- Get last refresh time
    SELECT last_indexed_at INTO last_refresh
    FROM indexing_metadata
    WHERE contract_address = 'stats_refresh'
    LIMIT 1;

    -- Only refresh if it's been more than 1 hour
    IF last_refresh IS NULL OR last_refresh < NOW() - INTERVAL '1 hour' THEN
        PERFORM refresh_event_statistics();

        -- Update refresh timestamp
        INSERT INTO indexing_metadata (contract_address, last_indexed_block, last_indexed_at)
        VALUES ('stats_refresh', 0, NOW())
        ON CONFLICT (contract_address)
        DO UPDATE SET last_indexed_at = NOW();
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on events table (with throttling via function logic)
CREATE TRIGGER refresh_event_statistics_trigger
    AFTER INSERT ON events
    FOR EACH STATEMENT EXECUTE FUNCTION trigger_refresh_event_stats();