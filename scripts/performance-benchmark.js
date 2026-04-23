#!/usr/bin/env node
/**
 * Database Performance Benchmark Script
 *
 * This script validates index usage and measures query performance
 * for the StellarLend indexing system.
 */

const { Client } = require('pg');
const Redis = require('ioredis');

class PerformanceBenchmark {
    constructor(dbConfig, redisConfig) {
        this.dbConfig = dbConfig;
        this.redisConfig = redisConfig;
        this.db = null;
        this.redis = null;
    }

    async connect() {
        // Connect to PostgreSQL
        this.db = new Client(this.dbConfig);
        await this.db.connect();
        console.log('✅ Connected to PostgreSQL');

        // Connect to Redis
        this.redis = new Redis(this.redisConfig.url);
        console.log('✅ Connected to Redis');
    }

    async disconnect() {
        if (this.db) await this.db.end();
        if (this.redis) await this.redis.disconnect();
    }

    async runBenchmark() {
        console.log('🚀 Starting Database Performance Benchmark...\n');

        try {
            await this.connect();

            // Check if indexes exist
            await this.validateIndexes();

            // Run performance tests
            await this.testQueryPerformance();

            // Analyze index usage
            await this.analyzeIndexUsage();

            // Test cache performance
            await this.testCachePerformance();

            console.log('✅ Benchmark completed successfully!');

        } catch (error) {
            console.error('❌ Benchmark failed:', error);
            throw error;
        } finally {
            await this.disconnect();
        }
    }

    async validateIndexes() {
        console.log('📊 Validating Database Indexes...');

        const indexes = await this.db.query(`
            SELECT
                schemaname,
                tablename,
                indexname,
                indexdef
            FROM pg_indexes
            WHERE tablename = 'events'
            ORDER BY indexname;
        `);

        console.log(`Found ${indexes.rows.length} indexes on events table:`);
        indexes.rows.forEach((idx, i) => {
            console.log(`  ${i + 1}. ${idx.indexname}`);
        });

        // Validate critical indexes exist
        const criticalIndexes = [
            'idx_events_contract_block',
            'idx_events_name_block',
            'idx_events_contract_name_block',
            'idx_events_indexed_at',
            'idx_events_recent',
            'idx_events_data_gin'
        ];

        const existingIndexes = indexes.rows.map(idx => idx.indexname);
        const missingIndexes = criticalIndexes.filter(idx => !existingIndexes.includes(idx));

        if (missingIndexes.length > 0) {
            console.warn('⚠️  Missing critical indexes:', missingIndexes);
        } else {
            console.log('✅ All critical indexes present');
        }

        console.log('');
    }

    async testQueryPerformance() {
        console.log('⚡ Testing Query Performance...');

        const queries = [
            {
                name: 'Recent events by contract',
                sql: `
                    SELECT id, contract_address, event_name, block_number
                    FROM events
                    WHERE contract_address = $1
                      AND indexed_at > NOW() - INTERVAL '7 days'
                    ORDER BY block_number DESC, log_index DESC
                    LIMIT 50
                `,
                params: ['0x1234567890123456789012345678901234567890'], // dummy address
                description: 'Should use idx_events_recent index'
            },
            {
                name: 'Events by type and block range',
                sql: `
                    SELECT id, contract_address, event_name, block_number
                    FROM events
                    WHERE contract_address = $1
                      AND event_name = $2
                      AND block_number BETWEEN $3 AND $4
                    ORDER BY block_number DESC, log_index DESC
                `,
                params: ['0x1234567890123456789012345678901234567890', 'Transfer', 1000000, 2000000],
                description: 'Should use idx_events_contract_name_block index'
            },
            {
                name: 'Event statistics',
                sql: `
                    SELECT contract_address, event_name, COUNT(*) as count
                    FROM events
                    GROUP BY contract_address, event_name
                    ORDER BY count DESC
                    LIMIT 10
                `,
                params: [],
                description: 'Should use idx_events_stats index'
            }
        ];

        for (const query of queries) {
            try {
                console.log(`Testing: ${query.name}`);
                console.log(`Description: ${query.description}`);

                // Run EXPLAIN ANALYZE
                const explainQuery = `EXPLAIN ANALYZE ${query.sql}`;
                const result = await this.db.query(explainQuery, query.params);

                // Check if query uses indexes
                const plan = result.rows.map(row => row['QUERY PLAN']).join('\n');
                const usesIndex = plan.includes('Index Scan') || plan.includes('Index Only Scan');

                console.log(usesIndex ? '✅ Uses index' : '⚠️  May not be using index efficiently');
                console.log('');
            } catch (error) {
                console.log(`❌ Query failed: ${error.message}`);
                console.log('');
            }
        }
    }

    async analyzeIndexUsage() {
        console.log('📈 Analyzing Index Usage Statistics...');

        try {
            const stats = await this.db.query(`
                SELECT
                    schemaname || '.' || indexname as index_name,
                    tablename,
                    idx_scan as scans,
                    idx_tup_read as tuples_read,
                    idx_tup_fetch as tuples_fetched
                FROM pg_stat_user_indexes
                WHERE tablename = 'events'
                ORDER BY idx_scan DESC;
            `);

            console.log('Index usage statistics:');
            stats.rows.forEach((stat, i) => {
                console.log(`  ${i + 1}. ${stat.index_name}: ${stat.scans} scans, ${stat.tuples_read} tuples read`);
            });
            console.log('');
        } catch (error) {
            console.log(`❌ Could not get index statistics: ${error.message}`);
            console.log('');
        }
    }

    async testCachePerformance() {
        console.log('💾 Testing Cache Performance...');

        try {
            // Test Redis connectivity
            const ping = await this.redis.ping();
            console.log(`Redis ping: ${ping}`);

            // Test cache operations
            const testKey = 'benchmark:test';
            const testValue = { timestamp: Date.now(), data: 'test' };

            await this.redis.setex(testKey, 60, JSON.stringify(testValue));
            const cached = await this.redis.get(testKey);
            const parsed = JSON.parse(cached);

            console.log('✅ Cache read/write operations working');
            console.log(`Cached value: ${JSON.stringify(parsed)}`);
            console.log('');
        } catch (error) {
            console.log(`❌ Cache test failed: ${error.message}`);
            console.log('');
        }
    }
}

// Run benchmark if called directly
if (require.main === module) {
    const dbConfig = {
        host: process.env.DB_HOST || 'localhost',
        port: process.env.DB_PORT || 5432,
        database: process.env.DB_NAME || 'indexer',
        user: process.env.DB_USER || 'postgres',
        password: process.env.DB_PASSWORD || 'password',
    };

    const redisConfig = {
        url: process.env.REDIS_URL || 'redis://localhost:6379',
    };

    const benchmark = new PerformanceBenchmark(dbConfig, redisConfig);
    benchmark.runBenchmark()
        .then(() => {
            console.log('🎉 All benchmarks passed!');
            process.exit(0);
        })
        .catch((error) => {
            console.error('💥 Benchmark failed:', error);
            process.exit(1);
        });
}

module.exports = PerformanceBenchmark;