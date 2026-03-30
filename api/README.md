# StellarLend REST API

REST API for StellarLend core lending operations (deposit, borrow, repay, withdraw) with Stellar Horizon and Soroban RPC integration.

## Features

- Unsigned transaction preparation for deposit, borrow, repay, withdraw operations
- Signed transaction submission endpoint
- Request validation and error handling
- Transaction submission and monitoring
- Rate limiting and security middleware
- 95%+ test coverage

## Quick Start

```bash
cd api
npm install
cp .env.example .env
# Edit .env with your configuration
npm run dev
```

## Configuration

Required environment variables in `.env`:

```env
PORT=3000
STELLAR_NETWORK=testnet
HORIZON_URL=https://horizon-testnet.stellar.org
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
CONTRACT_ID=<your_deployed_contract_id>
JWT_SECRET=<your_secret_key>
```

### Security Configuration

```env
# Request body size limit (prevents DoS attacks)
# Examples: 100kb, 1mb, 1gb. Defaults to 100kb if not set.
# Returns 413 Payload Too Large when exceeded.
BODY_SIZE_LIMIT=100kb

# Rate limiting (already set by default, configurable)
RATE_LIMIT_WINDOW_MS=900000
RATE_LIMIT_MAX_REQUESTS=100
```

## API Endpoints

### Health Check
`GET /api/health` - Check service status

### Prepare Transaction
`GET /api/lending/prepare/:operation`
```json
{
  "userAddress": "G...",
  "amount": "10000000"
}
```

Response:
```json
{
  "unsignedXdr": "AAAA...",
  "operation": "deposit",
  "expiresAt": "2026-03-28T12:34:56.000Z"
}
```

### Submit Signed Transaction
`POST /api/lending/submit`
```json
{
  "signedXdr": "AAAA..."
}
```

Response:
```json
{
  "success": true,
  "transactionHash": "abc123...",
  "status": "success",
  "ledger": 12345
}
```

All amounts in stroops (1 XLM = 10,000,000 stroops)

Clients must sign the returned XDR locally. The API does not accept Stellar secret keys.

## Testing

```bash
npm test              # Run all tests
npm test -- --coverage  # With coverage report
```

Test coverage: 95%+ (branches, functions, lines, statements)

## Production Build

```bash
npm run build
npm start
```

## Project Structure

```
api/src/
├── __tests__/      # Test files
├── config/         # Configuration
├── controllers/    # Request handlers
├── middleware/     # Validation, auth, errors
├── routes/         # API routes
├── services/       # Stellar integration
├── types/          # TypeScript types
└── utils/          # Logger, errors
```

## License

MIT
