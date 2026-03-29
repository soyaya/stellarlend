# StellarLend – Deployment Guide
This document provides step-by-step instructions for deploying all components of `StellarLend`: the landing page, API, Oracle service, and Soroban smart contracts. It also includes rollback procedures and verification steps. Both testnet and mainnet procedures are covered.

# Prerequisite

| Tool                     | Minimum Version                         | Install                                                                                        |
| ------------------------ | --------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Rust + Cargo             | 1.78+                                   | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`                              |
| `wasm32-unknown-unknown` | —                                       | `rustup target add wasm32-unknown-unknown`                                                     |
| Stellar CLI              | v21+                                    | [https://developers.stellar.org/docs/tools/cli](https://developers.stellar.org/docs/tools/cli) |
| Node.js / npm            | 18+                                     | [https://nodejs.org/](https://nodejs.org/)                                                     |
| Vercel CLI               | latest                                  | `npm i -g vercel`                                                                              |
| PM2                      | latest                                  | `npm i -g pm2`                                                                                 |
| Funded Stellar account   | ≥ 10 XLM (testnet) / ≥ 20 XLM (mainnet) | Friendbot (testnet) or exchange (mainnet)                                                      |


# Repository Layout
```
stellarlend/
├── contracts/                  # Soroban smart contracts
│   ├── hello-world/
│   └── amm/
├── api/                        # Backend API
│   ├── package.json
│   └── src/
├── oracle/                     # Off-chain oracle service
│   ├── package.json
│   └── src/
├── landing/                    # Vercel front-end
│   └── ...
├── scripts/
│   ├── build.sh
│   ├── deploy.sh
│   └── init.sh
└── docs/
    └── DEPLOYMENT.md
```

# Environment Variable

| Component       | Key                    | Description                         | Example (Testnet)                     |
| --------------- | ---------------------- | ----------------------------------- | ------------------------------------- |
| Smart Contracts | `ADMIN_SECRET_KEY`     | Deployer secret key                 | `SXXXXXXXXXXXXXXXX`                   |
| Smart Contracts | `ADMIN_ADDRESS`        | Admin control address               | `GXXXXXXXXXXXXXXXX`                   |
| API             | `NODE_ENV`             | `development` / `production`        | `production`                          |
| API             | `PORT`                 | Server listening port               | `3000`                                |
| API             | `STELLAR_RPC_URL`      | Soroban RPC endpoint                | `https://soroban-testnet.stellar.org` |
| Oracle          | `ORACLE_RPC_URL`       | RPC endpoint                        | `https://soroban-testnet.stellar.org` |
| Oracle          | `ORACLE_POLL_INTERVAL` | Update frequency in ms              | `30000`                               |
| Oracle          | `ORACLE_KEY`           | Signing key for oracle transactions | `SXXXXXXXXXXXXXXXX`                   |

>Tip: Use `.env` files per environment (testnet/mainnet) and do not commit secrets.

# Build

## Smart Contracts
```bash
./scripts/build.sh --release
```
- Checks for tools, formats code, runs Clippy, builds WASM, optimizes, tests.

## API & Landing Page
```bash
cd api
npm install
npm run build

cd ../landing
npm install
npm run build
```

## Oracle Service
```bash
cd oracle
npm install
npm run build
```

# Deploy

## Deploy Smart Contracts
```bash
export ADMIN_SECRET_KEY="S..."
export ADMIN_ADDRESS="G..."
./scripts/deploy.sh --network testnet --amm
```
- Contract IDs stored in `scripts/deployed/<network>/`.

## Deploy API & Landing Page (Vercel)
```bash
cd landing
vercel login
vercel --prod
```
- Make sure environment variables are set via Vercel dashboard for production.
- For staging/testnet, deploy with `vercel --prebuilt --confirm`.

## Deploy Oracle Service
```bash
cd oracle
pm2 start dist/index.js --name stellarlend-oracle
pm2 save
```
- Runs as a background service.
- Monitor logs: `pm2 logs stellarlend-oracle`.

# Initialize Smart Contracts
```bash
./scripts/init.sh --network testnet --init-amm
```

- Must be called exactly once.
- AMM initialization optional but recommended.
- Verify:
```bash
stellar contract invoke --id "$LENDING_CONTRACT_ID" \
  --source "$ADMIN_SECRET_KEY" --network testnet \
  -- get_min_collateral_ratio
```

# Rollback Procedures

## Smart Contracts
- Redeploy previous `.optimized.wasm` to revert to last known good state:
```bash
stellar contract deploy --wasm previous_version.optimized.wasm \
  --source "$ADMIN_SECRET_KEY" --network <network>
```

## API & Landing Page
- Rollback via Vercel:
```bash
vercel rollback <deployment-url>
```

## Oracle Service
- Restore previous working build:
```bash
pm2 stop stellarlend-oracle
pm2 delete stellarlend-oracle
git checkout <stable-commit>
npm run build
pm2 start dist/index.js --name stellarlend-oracle
```

# Post-deployment Verification
1. Check contract parameters (`min_collateral_ratio`, `is_emergency_paused`)
2. API endpoint health: `curl https://api.stellarlend.com/health`
3. Landing page loads over HTTPS
4. Oracle updates verified via logs or transaction history

# Testnet Walkthrough
```bash
git clone https://github.com/Smartdevs17/stellarlend.git
cd stellarlend

# Fund testnet account
stellar keys generate deployer --network testnet
ADMIN_ADDRESS="$(stellar keys address deployer)"
curl "https://friendbot.stellar.org?addr=$ADMIN_ADDRESS"

# Build & Deploy
./scripts/build.sh --release
export ADMIN_SECRET_KEY="$(stellar keys show deployer --secret-key)"
./scripts/deploy.sh --network testnet --amm

# Initialize
./scripts/init.sh --network testnet --init-amm

# API & Landing Page
cd landing
vercel --prebuilt --confirm

# Oracle
cd oracle
pm2 start dist/index.js --name stellarlend-oracle
```

# Mainnet Checklist
- All unit tests pass (`cargo test --verbose`)
- `cargo audit` shows no critical vulnerabilities
- Optimized WASM built
- Deployer account funded (≥ 20 XLM)
- Secrets stored securely (not shell history)
- Admin transferred to multisig
- API and landing page deployed to production with HTTPS
- Oracle service running and polling successfully
- Rollback procedures tested


# Troubleshooting
- **AlreadyInitialized** → expected behavior, do not retry
- **Deployment fails / empty contract ID** → check account balance
- **Oracle not updating** → verify `ORACLE_KEY` and poll interval
- **API landing page not loading** → check Vercel environment variables and HTTPS
- **Clippy/fmt errors** → `cargo fmt --all && cargo clippy --fix`





