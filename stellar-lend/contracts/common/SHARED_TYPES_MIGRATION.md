# Shared Types Migration Guide

This crate now provides a versioned shared type layer in `shared_types.rs` for protocol contracts.

## What moved to shared types

- `AssetConfigV1`
- `AssetRiskParamsV1`
- `PositionV1`
- `PositionSummaryV1`
- `SharedTypesVersion`

## Versioning strategy

- Each schema version is explicit (`SharedTypesVersion::V1`).
- On-chain or off-chain message payloads should include `version`.
- New versions should add `V2`, `V3`, etc. without mutating V1 layouts.

## Migration steps

1. Import shared structures from `stellarlend_common::shared_types`.
2. Convert legacy local structs to `*V1` structs at module boundaries.
3. Keep legacy fields temporarily behind adapter functions for backwards compatibility.
4. Emit events containing version metadata where applicable.
5. Remove deprecated local types only after all consumers are upgraded.

## Deprecation path

- Mark local duplicated types as deprecated.
- Keep adapters for one release cycle.
- Remove adapters after all contracts and indexers consume `*V1` shared types.
