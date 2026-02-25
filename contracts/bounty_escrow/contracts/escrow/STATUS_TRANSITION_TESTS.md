# Escrow Status Transitions Test Suite

## Overview
This test suite validates all allowed and disallowed status transitions in the bounty escrow contract.

## Status States
The escrow contract has four possible states:
- **Locked**: Initial state after funds are locked
- **Released**: Funds have been released to contributor
- **Refunded**: Funds have been fully refunded to depositor
- **PartiallyRefunded**: Funds have been partially refunded

## Valid Transitions

| From | To | Test | Description |
|------|-----|------|-------------|
| Locked | Released | `test_locked_to_released` | Standard release flow |
| Locked | Refunded | `test_locked_to_refunded` | Full refund after deadline |
| Locked | PartiallyRefunded | `test_locked_to_partially_refunded` | Partial refund with admin approval |
| PartiallyRefunded | Refunded | `test_partially_refunded_to_refunded` | Complete remaining refund |

## Invalid Transitions

All other transitions are invalid and properly rejected with appropriate errors:

### From Released State
- Released → Locked: `test_released_to_locked_fails` (Error #3: BountyExists)
- Released → Released: `test_released_to_released_fails` (Error #5: FundsNotLocked)
- Released → Refunded: `test_released_to_refunded_fails` (Error #5: FundsNotLocked)
- Released → PartiallyRefunded: `test_released_to_partially_refunded_fails` (Error #5: FundsNotLocked)

### From Refunded State
- Refunded → Locked: `test_refunded_to_locked_fails` (Error #3: BountyExists)
- Refunded → Released: `test_refunded_to_released_fails` (Error #5: FundsNotLocked)
- Refunded → Refunded: `test_refunded_to_refunded_fails` (Error #5: FundsNotLocked)
- Refunded → PartiallyRefunded: `test_refunded_to_partially_refunded_fails` (Error #5: FundsNotLocked)

### From PartiallyRefunded State
- PartiallyRefunded → Locked: `test_partially_refunded_to_locked_fails` (Error #3: BountyExists)
- PartiallyRefunded → Released: `test_partially_refunded_to_released_fails` (Error #5: FundsNotLocked)

## Test Implementation
- **File**: `src/test_status_transitions.rs`
- **Total Tests**: 14 (4 valid transitions + 10 invalid transitions)
- **Test Pattern**: Table-driven approach with one test per transition
- **Error Validation**: Each invalid transition test validates the specific error code returned

## Running Tests
```bash
cargo test test_status_transitions --lib
```

## Test Results
All 14 tests pass successfully, confirming that:
1. Valid state transitions work as expected
2. Invalid state transitions are properly blocked with appropriate error codes
3. The contract maintains state integrity throughout its lifecycle
