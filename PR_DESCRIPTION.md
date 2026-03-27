# Fix #36: Add per-user rate limiting for lending endpoints

## Summary
This PR implements per-user rate limiting for the Stellarlend API to address the security concern where multiple users behind the same IP share limits, and a single user can bypass limits using different IPs.

## Changes Made

### 1. Enhanced Rate Limiting Architecture (`api/src/app.ts`)
- **Added secondary user-based rate limiter**: 10 requests per minute per `userAddress`
- **Maintained existing IP-based limiter**: Continues to serve as the outer layer for all API endpoints
- **Smart key generation**: Extracts `userAddress` from request body first, then query parameters, falling back to IP
- **Targeted application**: User rate limiter applied specifically to `/api/lending` routes
- **Clear error messages**: Returns structured JSON response with `success: false` and descriptive error

### 2. Comprehensive Test Suite (`api/src/__tests__/integration.test.ts`)
- **Independent user limits**: Verifies different users can make requests independently
- **Query parameter support**: Tests rate limiting with `userAddress` in URL query params
- **Request body support**: Tests rate limiting with `userAddress` in POST request body
- **Fallback behavior**: Ensures IP-based limiting when `userAddress` is missing
- **Window reset**: Validates rate limit window expires correctly after 60 seconds
- **Endpoint isolation**: Confirms non-lending endpoints are unaffected by user rate limiting
- **Mixed source handling**: Tests consistent behavior across query and body sources
- **Outer layer verification**: Ensures IP-based limiting still applies to all endpoints

## Technical Implementation Details

### Rate Limiter Configuration
```typescript
const userRateLimiter = rateLimit({
  windowMs: 60 * 1000, // 1 minute window
  max: 10, // 10 requests per minute per user
  keyGenerator: (req) => {
    const userAddress = req.body?.userAddress || req.query?.userAddress || req.ip;
    return userAddress;
  },
  message: { success: false, error: 'Too many requests for this account' },
  standardHeaders: true,
  legacyHeaders: false,
});
```

### Application Order
1. **IP-based limiter** → Applied to all `/api/` routes (outer layer)
2. **User-based limiter** → Applied to `/api/lending` routes (inner layer)

## Security Benefits

- **Fair usage**: Each user gets their own rate limit regardless of IP sharing
- **DoS prevention**: Single users cannot bypass limits using multiple IPs
- **Backward compatibility**: Existing IP-based protection remains intact
- **Graceful degradation**: Falls back to IP limiting when user identification is unavailable

## Test Coverage

The implementation includes 7 new test cases covering:
- ✅ Independent user rate limiting
- ✅ Query parameter rate limiting
- ✅ Request body rate limiting
- ✅ Fallback to IP-based limiting
- ✅ Rate limit window reset
- ✅ Endpoint isolation
- ✅ Mixed request source handling
- ✅ IP-based outer layer verification

## Acceptance Criteria Met

- ✅ **Per-user rate limit enforced**: 10 req/min per address
- ✅ **IP-based limit still applies**: Outer layer maintained
- ✅ **Returns 429 with clear message**: Structured JSON error response
- ✅ **Tests verify both limits**: Comprehensive test suite included

## Breaking Changes

None. This is a purely additive security enhancement that maintains full backward compatibility.

## Configuration

The user rate limit parameters are hardcoded as specified:
- Window: 60 seconds (1 minute)
- Max requests: 10 per user per window
- Applied to: `/api/lending/*` endpoints only

These can be easily moved to environment variables if needed in future iterations.
