# Governance Security Quick Reference

## Flash Loan Attack Prevention - Quick Guide

### Core Protection: Snapshot-Based Voting

**Problem:** Attacker borrows 1M tokens via flash loan, votes, returns tokens in same transaction.

**Solution:** Voting power is determined by token balance at proposal creation time, not current balance.

```rust
// When proposal is created
take_vote_power_snapshot(env, proposal_id, &proposer, &vote_token);

// When user votes
let voting_power = get_snapshotted_vote_power(env, proposal_id, &voter, &vote_token);
// If voter had 0 tokens at proposal creation → voting_power = 0
```

**Result:** ✅ Tokens acquired after proposal creation have ZERO voting power.

---

## Security Checklist for Governance Operations

### Creating a Proposal

```rust
// ✅ DO: Ensure proposer holds tokens BEFORE creating proposal
mint_tokens(&proposer, 1000);
let proposal_id = gov_create_proposal(&proposer, proposal_type, description, None);

// ❌ DON'T: Create proposal then acquire tokens
let proposal_id = gov_create_proposal(&proposer, ...);
mint_tokens(&proposer, 1000); // Too late! Snapshot already taken
```

### Delegating Votes

```rust
// ✅ DO: Delegate at least 24h before proposal
delegate_vote(&delegator, &delegatee);
wait_24_hours();
create_proposal(...);

// ❌ DON'T: Delegate right before proposal
delegate_vote(&delegator, &delegatee);
create_proposal(...); // Delegation won't count!
```

### Voting on Proposals

```rust
// ✅ DO: Vote with tokens held at proposal creation
// Tokens are automatically locked until voting ends
gov_vote(&voter, proposal_id, VoteType::For);

// ❌ DON'T: Try to transfer tokens after voting
gov_vote(&voter, proposal_id, VoteType::For);
transfer_tokens(&voter, &recipient, amount); // Will fail if lock enforced!
```

---

## Attack Scenarios & Defenses

### Scenario 1: Flash Loan Vote Manipulation

**Attack:**
```
1. Borrow 10M tokens via flash loan
2. Vote on malicious proposal
3. Return tokens
4. Proposal passes with borrowed voting power
```

**Defense:** ✅ Snapshot-based voting
- Borrowed tokens have 0 voting power (acquired after proposal creation)
- Attack fails at step 2

---

### Scenario 2: Last-Minute Delegation

**Attack:**
```
1. Attacker creates proposal
2. Immediately delegates 5M tokens from accomplice
3. Votes with combined power
4. Revokes delegation after voting
```

**Defense:** ✅ 24-hour delegation deadline
- Delegation established <24h before proposal doesn't count
- Attack fails at step 3

---

### Scenario 3: Low-Participation Takeover

**Attack:**
```
1. Wait for low governance participation
2. Create malicious proposal
3. Vote with small amount (e.g., 1000 tokens)
4. Proposal passes due to low participation
```

**Defense:** ✅ Quorum requirements
- Proposal requires 40% of voting power to participate
- Attack fails at step 4 (quorum not met)

---

### Scenario 4: Rushed Malicious Proposal

**Attack:**
```
1. Attacker gains temporary voting majority
2. Creates and passes malicious proposal
3. Executes immediately before community can react
```

**Defense:** ✅ 2-day execution delay
- Mandatory 48-hour delay between queuing and execution
- Community has time to detect and cancel malicious proposal
- Attack fails at step 3

---

### Scenario 5: Delegation Chain Amplification

**Attack:**
```
1. A delegates to B
2. B delegates to C
3. C delegates to D
4. D delegates to E
5. E votes with amplified power
```

**Defense:** ✅ Max delegation depth = 3
- Delegation chain limited to 3 levels
- Attack fails at step 4 (depth exceeded)

---

### Scenario 6: Governance Spam

**Attack:**
```
1. Create 100 proposals to flood governance
2. Hide malicious proposal in noise
3. Community misses malicious proposal
```

**Defense:** ✅ Rate limiting (5 proposals per 24h)
- Each address limited to 5 proposals per day
- Attack fails at proposal #6

---

## Configuration Parameters

### Security vs. Usability Trade-offs

| Parameter | Default | Security Impact | Usability Impact |
|-----------|---------|-----------------|------------------|
| `DELEGATION_DEADLINE` | 24h | Higher = More secure | Higher = Less flexible |
| `VOTE_LOCK_PERIOD` | 7 days | Must ≥ voting period | Longer = Less liquidity |
| `PROPOSAL_RATE_LIMIT` | 5/day | Lower = More secure | Lower = Less governance activity |
| `DEFAULT_QUORUM_BPS` | 40% | Higher = More secure | Higher = Harder to pass proposals |
| `DEFAULT_EXECUTION_DELAY` | 2 days | Longer = More secure | Longer = Slower execution |

### Recommended Settings by Use Case

**High Security (DeFi Protocol with Large TVL):**
```rust
DELEGATION_DEADLINE: 48h
VOTE_LOCK_PERIOD: 14 days
PROPOSAL_RATE_LIMIT: 3/day
DEFAULT_QUORUM_BPS: 50%
DEFAULT_EXECUTION_DELAY: 3 days
```

**Balanced (Standard Governance):**
```rust
DELEGATION_DEADLINE: 24h  // Current default
VOTE_LOCK_PERIOD: 7 days  // Current default
PROPOSAL_RATE_LIMIT: 5/day  // Current default
DEFAULT_QUORUM_BPS: 40%  // Current default
DEFAULT_EXECUTION_DELAY: 2 days  // Current default
```

**High Activity (DAO with Frequent Proposals):**
```rust
DELEGATION_DEADLINE: 12h
VOTE_LOCK_PERIOD: 7 days
PROPOSAL_RATE_LIMIT: 10/day
DEFAULT_QUORUM_BPS: 30%
DEFAULT_EXECUTION_DELAY: 1 day
```

---

## Monitoring & Alerts

### Critical Events to Monitor

```rust
// 1. Suspicious voting activity
SuspiciousGovActivityEvent {
    proposal_id,
    voter,
    voter_power,
    total_supply_estimate,
    reason: "large_single_voter"
}
// Alert if: voter_power > 33% of supply

// 2. Proposal rate limit hit
GovernanceError::ProposalRateLimitExceeded
// Alert if: Same address hits limit multiple times

// 3. Large delegation chains
DelegationRecord { depth: 3 }
// Alert if: Multiple max-depth delegations

// 4. Emergency proposals
ProposalCreatedEvent { /* emergency proposal */ }
// Alert always: Requires immediate review
```

### Analytics Dashboard Metrics

```rust
let analytics = gov_get_analytics();

// Track over time:
analytics.total_proposals  // Governance activity
analytics.total_votes  // Participation rate
analytics.suspicious_proposals  // Attack attempts
analytics.max_single_voter_power  // Whale concentration
analytics.last_suspicious_at  // Recent attack attempts
```

---

## Emergency Response Procedures

### Scenario: Malicious Proposal Detected

**Step 1: Verify Threat**
```rust
let proposal = gov_get_proposal(proposal_id);
// Check: proposal.status, for_votes, against_votes
// Assess: Is this actually malicious?
```

**Step 2: Cancel if Possible**
```rust
// Admin can cancel any proposal
gov_cancel_proposal(&admin, proposal_id);
```

**Step 3: If Already Queued**
```rust
// Use execution delay window (2 days)
// Coordinate multisig to cancel
// Or prepare counter-proposal
```

**Step 4: Emergency Governance**
```rust
// If critical, use emergency proposal
gov_create_emergency_proposal(
    &multisig_admin,
    ProposalType::EmergencyPause(true),
    "Emergency pause due to attack"
);
// Executes immediately, no delay
```

---

## Integration Guide

### Token Contract Integration

To enforce vote locks, integrate with governance contract:

```rust
// In your token contract's transfer function:
pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), TokenError> {
    from.require_auth();
    
    // Check governance vote lock
    let gov_contract = get_governance_contract(&env);
    if gov_contract.gov_is_vote_locked(&from) {
        return Err(TokenError::VoteLocked);
    }
    
    // Proceed with transfer
    // ...
}
```

### Frontend Integration

```typescript
// Check if user can vote
async function canUserVote(proposalId: number, userAddress: string): Promise<boolean> {
    // Get snapshot
    const snapshot = await contract.gov_get_vote_power_snapshot(proposalId, userAddress);
    
    if (!snapshot) {
        return false; // No tokens at proposal creation
    }
    
    // Check if already voted
    const vote = await contract.gov_get_vote(proposalId, userAddress);
    if (vote) {
        return false; // Already voted
    }
    
    return snapshot.balance > 0;
}

// Check if tokens are locked
async function areTokensLocked(userAddress: string): Promise<boolean> {
    return await contract.gov_is_vote_locked(userAddress);
}

// Get voting power for proposal
async function getVotingPower(proposalId: number, userAddress: string): Promise<number> {
    const snapshot = await contract.gov_get_vote_power_snapshot(proposalId, userAddress);
    return snapshot?.balance || 0;
}
```

---

## Testing Checklist

Before deploying governance changes:

- [ ] Test flash loan attack scenario (tokens acquired after proposal)
- [ ] Test vote locking (tokens locked during vote)
- [ ] Test delegation deadline (recent delegations ignored)
- [ ] Test delegation depth limit (max 3 levels)
- [ ] Test quorum requirements (low participation blocked)
- [ ] Test execution delay (2-day minimum enforced)
- [ ] Test proposal rate limiting (5 per day max)
- [ ] Test emergency governance (immediate execution)
- [ ] Test proposal cancellation (admin can cancel)
- [ ] Test analytics tracking (suspicious activity flagged)
- [ ] Test legitimate large voters (not blocked)
- [ ] Test delegation during lock (prevented)

---

## Common Pitfalls

### ❌ Pitfall 1: Assuming Current Balance = Voting Power

```rust
// WRONG
let balance = token.balance(&voter);
let voting_power = balance; // ❌ Vulnerable to flash loans!

// CORRECT
let voting_power = get_snapshotted_vote_power(env, proposal_id, &voter, &vote_token);
```

### ❌ Pitfall 2: Not Checking Vote Locks

```rust
// WRONG
token.transfer(&from, &to, amount); // ❌ Allows circumventing vote locks!

// CORRECT
if gov_is_vote_locked(&from) {
    return Err(Error::VoteLocked);
}
token.transfer(&from, &to, amount);
```

### ❌ Pitfall 3: Immediate Execution

```rust
// WRONG
queue_proposal(proposal_id);
execute_proposal(proposal_id); // ❌ Bypasses execution delay!

// CORRECT
queue_proposal(proposal_id);
wait(2 * 24 * 3600); // Wait 2 days
execute_proposal(proposal_id);
```

### ❌ Pitfall 4: Ignoring Analytics

```rust
// WRONG
// Never check governance analytics
// Miss attack attempts

// CORRECT
let analytics = gov_get_analytics();
if analytics.suspicious_proposals > threshold {
    alert_security_team();
}
```

---

## Quick Reference: Function Call Order

### Normal Proposal Flow

```
1. gov_create_proposal()
   ↓ (snapshot taken automatically)
2. gov_vote() (multiple voters)
   ↓ (tokens locked automatically)
3. wait(voting_period)
   ↓
4. gov_queue_proposal()
   ↓ (quorum checked)
5. wait(execution_delay)
   ↓
6. gov_execute_proposal()
```

### Emergency Proposal Flow

```
1. gov_create_emergency_proposal()
   ↓ (immediately queued)
2. gov_execute_proposal()
   ↓ (no delay)
3. Action executed immediately
```

### Delegation Flow

```
1. gov_delegate_vote()
   ↓
2. wait(24h)
   ↓
3. Delegation now valid for new proposals
```

---

## Support & Resources

- **Full Documentation:** `GOVERNANCE_ATTACK_PREVENTION_SUMMARY.md`
- **Implementation:** `src/governance.rs`
- **Tests:** `src/tests/flash_loan_governance_test.rs`, `src/tests/governance_attack_prevention_test.rs`
- **Types:** `src/types.rs`
- **Events:** `src/events.rs`

---

**Last Updated:** 2026-04-24
**Version:** 1.0
**Status:** Production Ready
