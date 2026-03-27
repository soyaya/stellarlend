# Audit Logging for Financial Operations

## Overview

StellarLend now includes comprehensive audit logging for all financial operations to ensure compliance, debugging capabilities, and dispute resolution. This feature provides a structured audit trail of all user actions including deposits, borrows, repayments, and withdrawals.

## Features

- ✅ **Structured JSON Logging**: All audit entries are formatted as structured JSON for easy parsing and analysis
- ✅ **Complete Coverage**: All 4 lending operations (deposit, borrow, repay, withdraw) are logged
- ✅ **Security First**: No secrets or private keys are ever logged
- ✅ **Flexible**: Audit data is optional - works with existing integrations
- ✅ **Comprehensive**: Includes transaction hash, timestamp, IP address, and operation details

## Audit Log Format

Each successful transaction generates an audit log entry with the following structure:

```json
{
  "action": "DEPOSIT",
  "userAddress": "GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2",
  "amount": "1000000",
  "assetAddress": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U",
  "txHash": "abc123def456...",
  "timestamp": "2024-01-01T12:00:00.000Z",
  "ip": "192.168.1.1",
  "status": "success",
  "ledger": 12345
}
```

### Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `action` | string | Operation type in uppercase | `"DEPOSIT"`, `"BORROW"`, `"REPAY"`, `"WITHDRAW"` |
| `userAddress` | string | User's Stellar public key | `"GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2"` |
| `amount` | string | Amount in stroops (smallest unit) | `"1000000"` (0.01 XLM) |
| `assetAddress` | string | Asset contract address (optional) | `"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U"` |
| `txHash` | string | Stellar transaction hash | `"abc123def456..."` |
| `timestamp` | string | ISO 8601 timestamp | `"2024-01-01T12:00:00.000Z"` |
| `ip` | string | Client IP address | `"192.168.1.1"` |
| `status` | string | Transaction status | `"success"`, `"failed"`, `"pending"` |
| `ledger` | number | Stellar ledger number | `12345` |

## API Usage

### Enhanced Submit Endpoint

The `/api/lending/submit` endpoint now accepts optional audit fields:

```typescript
interface SubmitRequest {
  signedXdr: string;                    // Required: Signed transaction XDR
  operation?: 'deposit' | 'borrow' | 'repay' | 'withdraw'; // Optional: Operation type
  userAddress?: string;                 // Optional: User address for audit
  amount?: string;                       // Optional: Amount for audit
  assetAddress?: string;                 // Optional: Asset address for audit
}
```

### Examples

#### 1. Full Audit Data (Recommended)

```javascript
const response = await fetch('/api/lending/submit', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    signedXdr: 'AAAAAgAAAABgAAAAAAAAAAA...',
    operation: 'deposit',
    userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
    amount: '1000000',
    assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U'
  })
});
```

#### 2. Minimal Request (Backward Compatible)

```javascript
const response = await fetch('/api/lending/submit', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    signedXdr: 'AAAAAgAAAABgAAAAAAAAAAA...'
  })
});
```

**Note**: Minimal requests will generate audit logs with redacted values (`"REDACTED"`) for missing fields.

## Security Considerations

### ✅ What We Log

- Transaction hashes (public blockchain data)
- User addresses (public blockchain data)  
- Amounts (public blockchain data)
- Asset addresses (public blockchain data)
- IP addresses (standard web logging)
- Timestamps (standard logging)

### ❌ What We Never Log

- Private keys or secrets
- Mnemonic phrases
- Passwords
- API keys
- Any sensitive authentication data

### Data Redaction

When audit fields are not provided, the system automatically redacts sensitive information:

```json
{
  "action": "TRANSACTION_EXECUTED",
  "userAddress": "REDACTED",
  "amount": "REDACTED", 
  "assetAddress": "REDACTED",
  "txHash": "abc123def456...",
  "timestamp": "2024-01-01T12:00:00.000Z",
  "ip": "192.168.1.1",
  "status": "success",
  "ledger": 12345
}
```

## Implementation Details

### Logging Infrastructure

The audit logging uses Winston logger with JSON formatting:

```typescript
logger.info('AUDIT', auditData);
```

This ensures:
- Structured JSON output
- Consistent timestamp handling  
- Easy integration with log aggregation systems
- Proper error handling

### Error Handling

- **Failed Transactions**: No audit log is generated for failed submissions
- **Missing Fields**: Automatic redaction with `"REDACTED"` values
- **Invalid Data**: Validation errors prevent audit log generation
- **System Errors**: Logged separately without exposing user data

### Validation

All optional audit fields are validated:

```typescript
// Operation validation
operation?.must be one of: ['deposit', 'borrow', 'repay', 'withdraw']

// Address validation  
userAddress?.must be valid Stellar public key

// Amount validation
amount?.must be positive integer string
```

## Testing

Comprehensive test suite verifies:

1. **Audit Log Generation**: All 4 operations generate correct audit entries
2. **Data Redaction**: Missing fields are properly redacted
3. **Security**: No secrets are ever logged
4. **Validation**: Invalid audit data is rejected
5. **Backward Compatibility**: Existing integrations continue to work

Run tests with:
```bash
npm test -- --testNamePattern="audit"
```

## Compliance & Use Cases

### Regulatory Compliance
- **FINRA**: Transaction reporting requirements
- **KYC/AML**: Customer activity monitoring  
- **Audit Trails**: Complete transaction history
- **Data Retention**: Structured logging for archival

### Operational Use Cases
- **Dispute Resolution**: Transaction verification
- **Debugging**: Failed transaction analysis
- **Monitoring**: Real-time activity tracking
- **Analytics**: User behavior patterns

### Integration Examples

#### Log Aggregation (ELK Stack)
```json
{
  "@timestamp": "2024-01-01T12:00:00.000Z",
  "level": "info",
  "message": "AUDIT",
  "action": "DEPOSIT",
  "userAddress": "GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2",
  "amount": "1000000",
  "txHash": "abc123def456...",
  "ip": "192.168.1.1",
  "status": "success"
}
```

#### SIEM Integration
```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "event_type": "financial_transaction",
  "severity": "info",
  "source": "stellarlend_api",
  "details": {
    "action": "DEPOSIT",
    "user": "GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2",
    "amount": "1000000",
    "transaction_id": "abc123def456...",
    "client_ip": "192.168.1.1"
  }
}
```

## Migration Guide

### For Existing Integrations

**No changes required** - existing integrations continue to work unchanged.

### To Enable Full Audit Logging

1. **Update Submit Requests**: Add optional audit fields
2. **Capture Operation Details**: Store operation type during prepare phase
3. **Forward User Data**: Include user address and amount in submit requests

### Example Migration

#### Before (Minimal)
```javascript
// Existing code - no changes needed
const result = await submitTransaction({ signedXdr });
```

#### After (Enhanced)
```javascript
// Enhanced with audit data
const result = await submitTransaction({
  signedXdr,
  operation: 'deposit',
  userAddress: storedUserAddress,
  amount: storedAmount,
  assetAddress: storedAssetAddress
});
```

## Troubleshooting

### Common Issues

1. **Missing Audit Data**: Fields show as "REDACTED"
   - **Solution**: Include optional audit fields in submit requests

2. **Validation Errors**: Invalid audit data rejected
   - **Solution**: Ensure addresses are valid Stellar public keys, amounts are positive integers

3. **No Audit Logs**: Failed transactions don't generate logs
   - **Solution**: Check transaction success status, audit logs only for successful transactions

### Debug Mode

Enable debug logging to see audit log generation:
```bash
DEBUG=audit npm run dev
```

## Support

For questions or issues with audit logging:

1. Check the [test suite](../api/src/__tests__/lending.controller.test.ts) for examples
2. Review the [implementation](../api/src/controllers/lending.controller.ts) for details
3. Open an issue with specific use case and error details

---

**Version**: 1.0.0  
**Last Updated**: 2024-01-01  
**Compatibility**: All existing integrations
