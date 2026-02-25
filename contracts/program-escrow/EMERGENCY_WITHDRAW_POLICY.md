# Emergency Withdraw Security Policy

## Overview

The `emergency_withdraw` function in the Program Escrow contract provides a critical safety mechanism to drain contract funds in emergency situations. This document outlines the security model, access controls, and operational constraints that govern emergency withdrawals.

## Access Control (RBAC)

### Admin-Only Authority
- **Only the Admin role can execute emergency withdrawals**
- The Admin address is set during contract initialization via `initialize_contract(admin)`
- Non-admin roles (e.g., payout operators) are explicitly rejected with `Error(Auth, InvalidAction)`
- All emergency_withdraw calls internally call `admin.require_auth()` to enforce this constraint

### Role Hierarchy
```
Admin <-- Can execute emergency_withdraw
  ├── Payout Operator  <-- CANNOT execute emergency_withdraw
  └── Circuit Admin    <-- CANNOT execute emergency_withdraw (separate role)
```

## Operational Constraints

### Paused State Requirement
Emergency withdraw can **only be executed when the contract is paused** (specifically when `lock_paused = true`).

**Rationale**: Ensures emergency drain is only used in genuine crisis scenarios, preventing accidental fund losses.

#### Pause Flags Details
```rust
pub struct PauseFlags {
    pub lock_paused: bool,        // Required for emergency_withdraw
    pub release_paused: bool,     // Independent; NOT required for emergency_withdraw
    pub refund_paused: bool,      // Independent; NOT required for emergency_withdraw
    pub pause_reason: Option<String>,  // Optional reason (e.g., "Hacked")
    pub paused_at: u64,          // Timestamp when paused
}
```

**Critical**: Only `lock_paused` gates emergency_withdraw. A contract with only `release_paused=true` or `refund_paused=true` cannot execute emergency_withdraw.

### Panic Scenarios
Emergency withdraw will panic if:
1. Contract is not initialized (`"Not initialized"`)
2. `lock_paused != true` (`"Not paused"`)
3. Caller is not admin (`"Error(Auth, InvalidAction)"` from auth framework)

## Functional Behavior

### Fund Draining
When executed successfully, emergency_withdraw:
1. Queries contract's current token balance
2. Transfers entire balance to the specified target address
3. Emits an event with admin, target, amount, and timestamp
4. Does NOT modify pause state (pause flags remain unchanged)

### Idempotency
- **Emergency withdraw is idempotent**: calling it multiple times on an empty contract does not panic
- Provides safety for retry scenarios during disasters
- Recommended: Only call once, but safe if called multiple times

### Event Emission
Each successful emergency withdrawal emits an event:
```rust
Event topic:    "em_wtd" (symbol)
Event data:     (admin_address, target_address, amount, timestamp)
```

## Security Considerations

### Threat Model
Emergency withdraw protects against:
- **Hacked contract state**: Drain funds if contract is compromised
- **Bug exploitation**: Rapid fund extraction to minimize loss
- **Front-running**: Admin can pause and withdraw before attacker exploits pending transactions

### Attack Prevention
1. **Unauthorized withdrawal**: Admin auth check prevents any non-admin caller
2. **Accidental withdrawal**: Pause requirement prevents casual draining
3. **Repeated draining**: Only executes once (idempotent, but balance is 0 after first call)

### Post-Withdrawal Recovery
After emergency withdrawal:
- Contract remains in paused state (can be unpaused by admin)
- Contract is reusable: admin can unpause and resume operations
- All balances are gone: new liquidity must be provided

## Test Coverage

### RBAC Tests
1. ✅ **Admin can emergency_withdraw when paused**
   - Verifies successful withdrawal by authorized admin
   - Validates funds transferred and balance cleared

2. ✅ **Non-admin cannot emergency_withdraw**
   - Operator role explicitly rejected
   - Auth framework blocks unauthorized access

3. ✅ **Admin cannot emergency_withdraw when unpaused**
   - Ensures paused state is mandatory
   - Prevents accidental fund drains

### Pause State Tests
4. ✅ **Emergency withdraw requires lock_paused specifically**
   - Partial pause (only release/refund) is insufficient
   - Validates gate logic is correct

5. ✅ **Pause state preserved after withdrawal**
   - Contract remains paused after withdraw
   - Prevents accidental operation resumption

### Functional Tests
6. ✅ **Events emitted with correct data**
   - Topic, admin, target, amount, timestamp verified

7. ✅ **Idempotency and drain completeness**
   - Multiple withdrawals on empty contract safe
   - All funds drained on first call

8. ✅ **Recovery and reusability**
   - Contract can be unpaused after withdrawal
   - New operations can resume after unpause

## Usage Guidelines

### When to Use Emergency Withdraw
- ✅ **Security incident**: Malicious exploit detected
- ✅ **Major bug**: Critical logic error found in operations
- ✅ **External threat**: Related contract/chain attacked
- ✅ **Regulatory requirement**: Emergency fund isolation

### When NOT to Use Emergency Withdraw
- ❌ **Normal operations**: Use standard payout mechanisms
- ❌ **Partial funds**: Use single/batch payout instead
- ❌ **Testing**: Use testnet, not mainnet

### Recommended Workflow
1. Detect emergency condition
2. Call `set_paused(&Some(true), ...)` to lock operations
3. Review and verify fund amounts
4. Call `emergency_withdraw(&target)` with safe target address
5. Verify withdrawal succeeded (check events/balance)
6. Notify stakeholders
7. Conduct post-incident review
8. Decide whether to unpause or migrate to new contract

## Configuration

### Admin Assignment
```rust
// One-time initialization
contract.initialize_contract(&admin_address);
```

### Pause Lock
```rust
// Required before emergency_withdraw
let reason = String::from_str(&env, "Security incident");
contract.set_paused(&Some(true), &None, &None, &Some(reason));
```

### Execute Withdrawal
```rust
// Must be called by admin, when lock_paused = true
contract.emergency_withdraw(&recovery_address);
```

## Monitoring and Alerting

### Event Monitoring
- Monitor for `em_wtd` events
- Alert if emergency_withdraw is executed
- Track source admin and target address
- Log timestamp for incident investigation

### Audit Trail
- Review pause history: when was `lock_paused` set?
- Check payout history: what was last operation before pause?
- Analyze transaction logs around emergency_withdraw event
- Compare against token balance movements

## Future Enhancements

Potential improvements for consideration:
1. **Timelock**: Delay between pause and emergency_withdraw
2. **Multi-sig**: Require multiple admins to approve withdrawal
3. **Rate limiting**: Limit withdrawal frequency
4. **Destination whitelist**: Only allow known recovery addresses
5. **Graduated pause**: Pause individual operations, not full drain

## References

- **Contract**: `contracts/program-escrow/src/lib.rs`
- **Tests**: `contracts/program-escrow/src/test_pause.rs`
- **RBAC Tests**: Lines 323-621 of `test_pause.rs`
- **Events**: Emitted with topic `symbol_short!("em_wtd")`

## Compliance Notes

This emergency withdraw mechanism:
- ✅ Complies with Soroban SDK best practices
- ✅ Follows Stellar contract security guidelines
- ✅ Respects RBAC separation of concerns
- ✅ Includes comprehensive test coverage
- ✅ Provides full audit trail via events
