# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

---

## [Unreleased]

### Added
- Multi-stage Dockerization for API and Oracle services
- OpenAPI/Swagger documentation for API
- Price staleness detection and alerting in oracle service
- Retry logic with exponential backoff for transaction submission
- Monorepo root package configuration
- Circuit breaker implementation for system stability
- LRU batch cache eviction mechanism
- E2E integration tests for Oracle–Contract–API pipeline
- Multi-asset collateral support
- WebSocket endpoint for real-time price updates
- Health factor and position query entrypoints
- Protocol fee collection and treasury management
- Stellar address validation and enhanced middleware support
- Protocol governance system
- Oracle contract updater

### Changed
- Refactored duplicated validation logic using factory functions
- Switched transaction polling to fixed interval strategy
- Improved project structure and test organization
- Expanded full lifecycle integration test coverage
- Enhanced oracle performance and rate-limit handling
- Updated CI pipeline to enforce blocking contract checks
- Added contributing documentation

### Fixed
- Borrow interest overflow error
- Integration test placeholders and inconsistencies
- Default debt asset initialization for borrow/repay
- Flash loan reentrancy locks and VM execution aborts
- Masked admin secret key in oracle logs and outputs
- Enforced HTTPS and HSTS security headers
- Strengthened JWT secret validation and removed insecure defaults
- Validated CONTRACT_ID environment variable on API startup
- Fixed Rust CI issues (formatting, clippy, tests)

---

## 📌 Notes

- The project is under active development and changes are tracked under the Unreleased section.
- A formal version release will be added once versioning is standardized across the project.
