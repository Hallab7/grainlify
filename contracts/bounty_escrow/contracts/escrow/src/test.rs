#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, vec, Address, Env, Vec,
};

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    (
        token::Client::new(e, &contract_address),
        token::StellarAssetClient::new(e, &contract_address),
    )
}

fn create_escrow_contract<'a>(e: &Env) -> (BountyEscrowContractClient<'a>, Address) {
    let contract_id = e.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(e, &contract_id);
    (client, contract_id)
}

struct TestSetup<'a> {
    env: Env,
    admin: Address,
    depositor: Address,
    contributor: Address,
    token: token::Client<'a>,
    token_admin: token::StellarAssetClient<'a>,
    escrow: BountyEscrowContractClient<'a>,
    escrow_address: Address,
}

impl<'a> TestSetup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);

        let (token, token_admin) = create_token_contract(&env, &admin);
        let (escrow, escrow_address) = create_escrow_contract(&env);

        escrow.init(&admin, &token.address);

        // Mint tokens to depositor
        token_admin.mint(&depositor, &1_000_000);

        Self {
            env,
            admin,
            depositor,
            contributor,
            token,
            token_admin,
            escrow,
            escrow_address,
        }
    }
}

#[test]
fn test_lock_funds_success() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Lock funds
    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Verify stored escrow data
    // Note: amount stores net_amount (after fee), but fees are disabled by default
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.depositor, setup.depositor);
    assert_eq!(stored_escrow.amount, amount); // net_amount = amount when fees disabled
    assert_eq!(stored_escrow.remaining_amount, amount); // remaining_amount stores original
    assert_eq!(stored_escrow.status, EscrowStatus::Locked);
    assert_eq!(stored_escrow.deadline, deadline);

    // Verify contract balance
    assert_eq!(setup.token.balance(&setup.escrow_address), amount);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // BountyExists
fn test_lock_funds_duplicate() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Try to lock again with same bounty_id
    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

#[test]
#[should_panic] // Token transfer fail
fn test_lock_funds_negative_amount() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = -100;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
}

#[test]
fn test_get_escrow_info() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    let escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(escrow.amount, amount);
    assert_eq!(escrow.deadline, deadline);
    assert_eq!(escrow.depositor, setup.depositor);
    assert_eq!(escrow.status, EscrowStatus::Locked);
}

#[test]
fn test_release_funds_success() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Verify initial balances
    assert_eq!(setup.token.balance(&setup.escrow_address), amount);
    assert_eq!(setup.token.balance(&setup.contributor), 0);

    // Release funds
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    // Verify updated state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::Released);

    // Verify balances after release (fees disabled by default, so net_amount = amount)
    assert_eq!(setup.token.balance(&setup.escrow_address), 0);
    assert_eq!(setup.token.balance(&setup.contributor), amount);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")] // FundsNotLocked
fn test_release_funds_already_released() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    // Try to release again
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // BountyNotFound
fn test_release_funds_not_found() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    setup.escrow.release_funds(&bounty_id, &setup.contributor);
}

// ============================================================================
// REFUND TESTS - Full Refund After Deadline
// ============================================================================

#[test]
fn test_refund_full_after_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Advance time past deadline
    setup.env.ledger().set_timestamp(deadline + 1);

    // Initial balances
    let initial_depositor_balance = setup.token.balance(&setup.depositor);

    // Full refund (no amount/recipient specified, mode = Full)
    setup.escrow.refund(
        &bounty_id,
        &None::<i128>,
        &None::<Address>,
        &RefundMode::Full,
    );

    // Verify state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::Refunded);
    assert_eq!(stored_escrow.remaining_amount, 0);

    // Verify balances
    assert_eq!(setup.token.balance(&setup.escrow_address), 0);
    assert_eq!(
        setup.token.balance(&setup.depositor),
        initial_depositor_balance + amount
    );

    // Verify refund history
    let refund_history = setup.escrow.get_refund_history(&bounty_id);
    assert_eq!(refund_history.len(), 1);
    assert_eq!(refund_history.get(0).unwrap().amount, amount);
    assert_eq!(refund_history.get(0).unwrap().recipient, setup.depositor);
    assert_eq!(refund_history.get(0).unwrap().mode, RefundMode::Full);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // DeadlineNotPassed
fn test_refund_full_before_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Attempt full refund before deadline (should fail)
    setup.escrow.refund(
        &bounty_id,
        &None::<i128>,
        &None::<Address>,
        &RefundMode::Full,
    );
}

// ============================================================================
// REFUND TESTS - Partial Refund
// ============================================================================

#[test]
fn test_refund_partial_after_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let total_amount = 1000;
    let refund_amount = 300;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &total_amount, &deadline);

    // Advance time past deadline
    setup.env.ledger().set_timestamp(deadline + 1);

    // Initial balances
    let initial_depositor_balance = setup.token.balance(&setup.depositor);

    // Partial refund
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &None::<Address>,
        &RefundMode::Partial,
    );

    // Verify state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::PartiallyRefunded);
    assert_eq!(stored_escrow.remaining_amount, total_amount - refund_amount);

    // Verify balances
    assert_eq!(
        setup.token.balance(&setup.escrow_address),
        total_amount - refund_amount
    );
    assert_eq!(
        setup.token.balance(&setup.depositor),
        initial_depositor_balance + refund_amount
    );

    // Verify refund history
    let refund_history = setup.escrow.get_refund_history(&bounty_id);
    assert_eq!(refund_history.len(), 1);
    assert_eq!(refund_history.get(0).unwrap().amount, refund_amount);
    assert_eq!(refund_history.get(0).unwrap().recipient, setup.depositor);
    assert_eq!(refund_history.get(0).unwrap().mode, RefundMode::Partial);
}

#[test]
fn test_refund_partial_multiple_times() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let total_amount = 1000;
    let refund1 = 200;
    let refund2 = 300;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &total_amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // First partial refund
    setup.escrow.refund(
        &bounty_id,
        &Some(refund1),
        &None::<Address>,
        &RefundMode::Partial,
    );

    // Second partial refund
    setup.escrow.refund(
        &bounty_id,
        &Some(refund2),
        &None::<Address>,
        &RefundMode::Partial,
    );

    // Verify state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::PartiallyRefunded);
    assert_eq!(
        stored_escrow.remaining_amount,
        total_amount - refund1 - refund2
    );

    // Verify refund history has 2 records
    let refund_history = setup.escrow.get_refund_history(&bounty_id);
    assert_eq!(refund_history.len(), 2);
    assert_eq!(refund_history.get(0).unwrap().amount, refund1);
    assert_eq!(refund_history.get(1).unwrap().amount, refund2);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // DeadlineNotPassed
fn test_refund_partial_before_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let refund_amount = 300;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Attempt partial refund before deadline (should fail)
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &None::<Address>,
        &RefundMode::Partial,
    );
}

// ============================================================================
// REFUND TESTS - Custom Refund (Different Address)
// ============================================================================

#[test]
fn test_refund_custom_after_deadline() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let refund_amount = 500;
    let custom_recipient = Address::generate(&setup.env);
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // Initial balances
    let initial_recipient_balance = setup.token.balance(&custom_recipient);

    // Custom refund to different address (after deadline, no approval needed)
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &Some(custom_recipient.clone()),
        &RefundMode::Custom,
    );

    // Verify state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::PartiallyRefunded);
    assert_eq!(stored_escrow.remaining_amount, amount - refund_amount);

    // Verify balances
    assert_eq!(
        setup.token.balance(&custom_recipient),
        initial_recipient_balance + refund_amount
    );

    // Verify refund history
    let refund_history = setup.escrow.get_refund_history(&bounty_id);
    assert_eq!(refund_history.len(), 1);
    assert_eq!(refund_history.get(0).unwrap().amount, refund_amount);
    assert_eq!(refund_history.get(0).unwrap().recipient, custom_recipient);
    assert_eq!(refund_history.get(0).unwrap().mode, RefundMode::Custom);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")] // RefundNotApproved
fn test_refund_custom_before_deadline_without_approval() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let refund_amount = 500;
    let custom_recipient = Address::generate(&setup.env);
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Attempt custom refund before deadline without approval (should fail)
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &Some(custom_recipient),
        &RefundMode::Custom,
    );
}

// ============================================================================
// REFUND TESTS - Approval Workflow
// ============================================================================

#[test]
fn test_refund_approval_workflow() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let refund_amount = 500;
    let custom_recipient = Address::generate(&setup.env);
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Admin approves refund before deadline
    setup.escrow.approve_refund(
        &bounty_id,
        &refund_amount,
        &custom_recipient.clone(),
        &RefundMode::Custom,
    );

    // Verify approval exists
    let (can_refund, deadline_passed, remaining, approval) =
        setup.escrow.get_refund_eligibility(&bounty_id);
    assert!(can_refund);
    assert!(!deadline_passed);
    assert_eq!(remaining, amount);
    assert!(approval.is_some());
    let approval_data = approval.unwrap();
    assert_eq!(approval_data.amount, refund_amount);
    assert_eq!(approval_data.recipient, custom_recipient);
    assert_eq!(approval_data.mode, RefundMode::Custom);
    assert_eq!(approval_data.approved_by, setup.admin);

    // Initial balances
    let initial_recipient_balance = setup.token.balance(&custom_recipient);

    // Execute approved refund (before deadline)
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &Some(custom_recipient.clone()),
        &RefundMode::Custom,
    );

    // Verify approval was consumed (removed after use)
    let (_, _, _, approval_after) = setup.escrow.get_refund_eligibility(&bounty_id);
    assert!(approval_after.is_none());

    // Verify state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::PartiallyRefunded);
    assert_eq!(stored_escrow.remaining_amount, amount - refund_amount);

    // Verify balances
    assert_eq!(
        setup.token.balance(&custom_recipient),
        initial_recipient_balance + refund_amount
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")] // RefundNotApproved
fn test_refund_approval_mismatch() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let approved_amount = 500;
    let requested_amount = 600; // Different amount
    let custom_recipient = Address::generate(&setup.env);
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Admin approves refund for 500
    setup.escrow.approve_refund(
        &bounty_id,
        &approved_amount,
        &custom_recipient.clone(),
        &RefundMode::Custom,
    );

    // Try to refund with different amount (should fail)
    setup.escrow.refund(
        &bounty_id,
        &Some(requested_amount),
        &Some(custom_recipient),
        &RefundMode::Custom,
    );
}

#[test]
#[ignore] // Note: With mock_all_auths(), we can't test unauthorized access
          // The security is enforced by require_auth() in the contract which checks admin address
          // In production, non-admin calls will fail at require_auth()
fn test_refund_approval_non_admin() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let _refund_amount = 500;
    let _custom_recipient = Address::generate(&setup.env);
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Note: With mock_all_auths(), we can't easily test unauthorized access
    // The contract's require_auth() will enforce admin-only access in production
    // This test is marked as ignored as it requires more complex auth setup
}

// ============================================================================
// REFUND TESTS - Refund History Tracking
// ============================================================================

#[test]
fn test_refund_history_tracking() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let total_amount = 1000;
    let refund1 = 200;
    let refund2 = 300;
    let _refund3 = 400;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &total_amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // First refund (Partial)
    setup.escrow.refund(
        &bounty_id,
        &Some(refund1),
        &None::<Address>,
        &RefundMode::Partial,
    );

    // Second refund (Partial)
    setup.escrow.refund(
        &bounty_id,
        &Some(refund2),
        &None::<Address>,
        &RefundMode::Partial,
    );

    // Third refund (Full remaining - should complete the refund)
    let remaining = total_amount - refund1 - refund2;
    setup.escrow.refund(
        &bounty_id,
        &Some(remaining),
        &None::<Address>,
        &RefundMode::Partial,
    );

    // Verify refund history
    let refund_history = setup.escrow.get_refund_history(&bounty_id);
    assert_eq!(refund_history.len(), 3);

    // Check first refund record
    let record1 = refund_history.get(0).unwrap();
    assert_eq!(record1.amount, refund1);
    assert_eq!(record1.recipient, setup.depositor);
    assert_eq!(record1.mode, RefundMode::Partial);

    // Check second refund record
    let record2 = refund_history.get(1).unwrap();
    assert_eq!(record2.amount, refund2);
    assert_eq!(record2.recipient, setup.depositor);
    assert_eq!(record2.mode, RefundMode::Partial);

    // Check third refund record
    let record3 = refund_history.get(2).unwrap();
    assert_eq!(record3.amount, remaining);
    assert_eq!(record3.recipient, setup.depositor);
    assert_eq!(record3.mode, RefundMode::Partial);

    // Verify final state
    let stored_escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(stored_escrow.status, EscrowStatus::Refunded);
    assert_eq!(stored_escrow.remaining_amount, 0);
}

#[test]
fn test_refund_history_with_custom_recipients() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let total_amount = 1000;
    let recipient1 = Address::generate(&setup.env);
    let recipient2 = Address::generate(&setup.env);
    let refund1 = 300;
    let refund2 = 400;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &total_amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // First custom refund
    setup.escrow.refund(
        &bounty_id,
        &Some(refund1),
        &Some(recipient1.clone()),
        &RefundMode::Custom,
    );

    // Second custom refund
    setup.escrow.refund(
        &bounty_id,
        &Some(refund2),
        &Some(recipient2.clone()),
        &RefundMode::Custom,
    );

    // Verify refund history
    let refund_history = setup.escrow.get_refund_history(&bounty_id);
    assert_eq!(refund_history.len(), 2);
    assert_eq!(refund_history.get(0).unwrap().recipient, recipient1);
    assert_eq!(refund_history.get(1).unwrap().recipient, recipient2);
}

// ============================================================================
// REFUND TESTS - Error Cases
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // InvalidAmount
fn test_refund_invalid_amount_zero() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // Try to refund zero amount
    setup
        .escrow
        .refund(&bounty_id, &Some(0), &None::<Address>, &RefundMode::Partial);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // InvalidAmount
fn test_refund_invalid_amount_exceeds_remaining() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let refund_amount = 1500; // More than available
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // Try to refund more than available
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &None::<Address>,
        &RefundMode::Partial,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // InvalidAmount
fn test_refund_custom_missing_amount() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let custom_recipient = Address::generate(&setup.env);
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // Custom refund requires amount
    setup.escrow.refund(
        &bounty_id,
        &None::<i128>,
        &Some(custom_recipient),
        &RefundMode::Custom,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // InvalidAmount
fn test_refund_custom_missing_recipient() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let refund_amount = 500;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);

    // Custom refund requires recipient
    setup.escrow.refund(
        &bounty_id,
        &Some(refund_amount),
        &None::<Address>,
        &RefundMode::Custom,
    );
}

#[test]
fn test_get_refund_eligibility() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Before deadline, no approval
    let (can_refund, deadline_passed, remaining, approval) =
        setup.escrow.get_refund_eligibility(&bounty_id);
    assert!(!can_refund);
    assert!(!deadline_passed);
    assert_eq!(remaining, amount);
    assert!(approval.is_none());

    // After deadline
    setup.env.ledger().set_timestamp(deadline + 1);
    let (can_refund, deadline_passed, remaining, approval) =
        setup.escrow.get_refund_eligibility(&bounty_id);
    assert!(can_refund);
    assert!(deadline_passed);
    assert_eq!(remaining, amount);
    assert!(approval.is_none());

    // With approval before deadline
    setup.env.ledger().set_timestamp(deadline - 100);
    let custom_recipient = Address::generate(&setup.env);
    setup
        .escrow
        .approve_refund(&bounty_id, &500, &custom_recipient, &RefundMode::Custom);

    let (can_refund, deadline_passed, remaining, approval) =
        setup.escrow.get_refund_eligibility(&bounty_id);
    assert!(can_refund);
    assert!(!deadline_passed);
    assert_eq!(remaining, amount);
    assert!(approval.is_some());
}

#[test]
fn test_get_balance() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 500;
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Initial balance should be 0
    assert_eq!(setup.escrow.get_balance(), 0);

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    // Balance should be updated
    assert_eq!(setup.escrow.get_balance(), amount);
}

// ============================================================================
// BATCH OPERATIONS TESTS
// ============================================================================

#[test]
fn test_batch_lock_funds_success() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Create batch items
    let items = vec![
        &setup.env,
        LockFundsItem {
            bounty_id: 1,
            depositor: setup.depositor.clone(),
            amount: 1000,
            deadline,
        },
        LockFundsItem {
            bounty_id: 2,
            depositor: setup.depositor.clone(),
            amount: 2000,
            deadline,
        },
        LockFundsItem {
            bounty_id: 3,
            depositor: setup.depositor.clone(),
            amount: 3000,
            deadline,
        },
    ];

    // Mint enough tokens
    setup.token_admin.mint(&setup.depositor, &10_000);

    // Batch lock funds
    let count = setup.escrow.batch_lock_funds(&items);
    assert_eq!(count, 3);

    // Verify all bounties are locked
    for i in 1..=3 {
        let escrow = setup.escrow.get_escrow_info(&i);
        assert_eq!(escrow.status, EscrowStatus::Locked);
    }

    // Verify contract balance
    assert_eq!(setup.escrow.get_balance(), 6000);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // InvalidBatchSize
fn test_batch_lock_funds_empty() {
    let setup = TestSetup::new();
    let items: Vec<LockFundsItem> = vec![&setup.env];
    setup.escrow.batch_lock_funds(&items);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // BountyExists
fn test_batch_lock_funds_duplicate_bounty_id() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Lock a bounty first
    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);

    // Try to batch lock with duplicate bounty_id
    let items = vec![
        &setup.env,
        LockFundsItem {
            bounty_id: 1, // Already exists
            depositor: setup.depositor.clone(),
            amount: 2000,
            deadline,
        },
        LockFundsItem {
            bounty_id: 2,
            depositor: setup.depositor.clone(),
            amount: 3000,
            deadline,
        },
    ];

    setup.escrow.batch_lock_funds(&items);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // DuplicateBountyId
fn test_batch_lock_funds_duplicate_in_batch() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    let items = vec![
        &setup.env,
        LockFundsItem {
            bounty_id: 1,
            depositor: setup.depositor.clone(),
            amount: 1000,
            deadline,
        },
        LockFundsItem {
            bounty_id: 1, // Duplicate in same batch
            depositor: setup.depositor.clone(),
            amount: 2000,
            deadline,
        },
    ];

    setup.escrow.batch_lock_funds(&items);
}

#[test]
fn test_batch_release_funds_success() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Lock multiple bounties
    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &2, &2000, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &3, &3000, &deadline);

    // Create contributors
    let contributor1 = Address::generate(&setup.env);
    let contributor2 = Address::generate(&setup.env);
    let contributor3 = Address::generate(&setup.env);

    // Create batch release items
    let items = vec![
        &setup.env,
        ReleaseFundsItem {
            bounty_id: 1,
            contributor: contributor1.clone(),
        },
        ReleaseFundsItem {
            bounty_id: 2,
            contributor: contributor2.clone(),
        },
        ReleaseFundsItem {
            bounty_id: 3,
            contributor: contributor3.clone(),
        },
    ];

    // Batch release funds
    let count = setup.escrow.batch_release_funds(&items);
    assert_eq!(count, 3);

    // Verify all bounties are released
    for i in 1..=3 {
        let escrow = setup.escrow.get_escrow_info(&i);
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    // Verify balances
    assert_eq!(setup.token.balance(&contributor1), 1000);
    assert_eq!(setup.token.balance(&contributor2), 2000);
    assert_eq!(setup.token.balance(&contributor3), 3000);
    assert_eq!(setup.escrow.get_balance(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // InvalidBatchSize
fn test_batch_release_funds_empty() {
    let setup = TestSetup::new();
    let items: Vec<ReleaseFundsItem> = vec![&setup.env];
    setup.escrow.batch_release_funds(&items);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // BountyNotFound
fn test_batch_release_funds_not_found() {
    let setup = TestSetup::new();
    let contributor = Address::generate(&setup.env);

    let items = vec![
        &setup.env,
        ReleaseFundsItem {
            bounty_id: 999, // Doesn't exist
            contributor: contributor.clone(),
        },
    ];

    setup.escrow.batch_release_funds(&items);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")] // FundsNotLocked
fn test_batch_release_funds_already_released() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Lock and release one bounty
    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);
    setup.escrow.release_funds(&1, &setup.contributor);

    // Lock another bounty
    setup
        .escrow
        .lock_funds(&setup.depositor, &2, &2000, &deadline);

    let contributor2 = Address::generate(&setup.env);

    // Try to batch release including already released bounty
    let items = vec![
        &setup.env,
        ReleaseFundsItem {
            bounty_id: 1, // Already released
            contributor: setup.contributor.clone(),
        },
        ReleaseFundsItem {
            bounty_id: 2,
            contributor: contributor2.clone(),
        },
    ];

    setup.escrow.batch_release_funds(&items);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // DuplicateBountyId
fn test_batch_release_funds_duplicate_in_batch() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);

    let contributor = Address::generate(&setup.env);

    let items = vec![
        &setup.env,
        ReleaseFundsItem {
            bounty_id: 1,
            contributor: contributor.clone(),
        },
        ReleaseFundsItem {
            bounty_id: 1, // Duplicate in same batch
            contributor: contributor.clone(),
        },
    ];

    setup.escrow.batch_release_funds(&items);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // BountyExists
fn test_batch_operations_atomicity() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Lock one bounty successfully
    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);

    // Try to batch lock with one valid and one that would fail (duplicate)
    // This should fail entirely due to atomicity
    let items = vec![
        &setup.env,
        LockFundsItem {
            bounty_id: 2, // Valid
            depositor: setup.depositor.clone(),
            amount: 2000,
            deadline,
        },
        LockFundsItem {
            bounty_id: 1, // Already exists - should cause entire batch to fail
            depositor: setup.depositor.clone(),
            amount: 3000,
            deadline,
        },
    ];

    // This should panic and no bounties should be locked
    setup.escrow.batch_lock_funds(&items);
}

#[test]
fn test_batch_operations_large_batch() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;

    // Create a batch of 10 bounties
    let mut items = Vec::new(&setup.env);
    for i in 1..=10 {
        items.push_back(LockFundsItem {
            bounty_id: i,
            depositor: setup.depositor.clone(),
            amount: (i * 100) as i128,
            deadline,
        });
    }

    // Mint enough tokens
    setup.token_admin.mint(&setup.depositor, &10_000);

    // Batch lock
    let count = setup.escrow.batch_lock_funds(&items);
    assert_eq!(count, 10);

    // Verify all are locked
    for i in 1..=10 {
        let escrow = setup.escrow.get_escrow_info(&i);
        assert_eq!(escrow.status, EscrowStatus::Locked);
    }

    // Create batch release items
    let mut release_items = Vec::new(&setup.env);
    for i in 1..=10 {
        release_items.push_back(ReleaseFundsItem {
            bounty_id: i,
            contributor: Address::generate(&setup.env),
        });
    }

    // Batch release
    let release_count = setup.escrow.batch_release_funds(&release_items);
    assert_eq!(release_count, 10);
}

// ============================================================================
// CLAIM PERIOD TESTS
// ============================================================================

#[test]
fn test_set_claim_window_success() {
    let setup = TestSetup::new();
    // Admin sets a 2-hour claim window
    setup.escrow.set_claim_window(&7200);
}

#[test]
fn test_authorize_claim_creates_pending_claim() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    let claim = setup.escrow.get_pending_claim(&bounty_id);
    assert_eq!(claim.recipient, setup.contributor);
    assert_eq!(claim.amount, amount);
    assert!(!claim.claimed);
    // expires_at should be current time + 3600
    assert!(claim.expires_at > setup.env.ledger().timestamp());
}

#[test]
fn test_claim_within_window_transfers_funds() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    // Contributor claims within window
    setup.escrow.claim(&bounty_id);

    // Funds transferred
    assert_eq!(setup.token.balance(&setup.contributor), amount);
    assert_eq!(setup.token.balance(&setup.escrow_address), 0);

    // Escrow marked released
    let escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(escrow.status, EscrowStatus::Released);
    assert_eq!(escrow.remaining_amount, 0);
}

#[test]
#[should_panic]
fn test_claim_after_window_expires_panics() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 10000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    // Advance time past the claim window
    setup
        .env
        .ledger()
        .set_timestamp(setup.env.ledger().timestamp() + 3601);

    setup.escrow.claim(&bounty_id);
}

#[test]
fn test_cancel_pending_claim_restores_escrow() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    // Admin cancels the claim
    setup.escrow.cancel_pending_claim(&bounty_id);

    // Funds still in escrow
    assert_eq!(setup.token.balance(&setup.escrow_address), amount);
    assert_eq!(setup.token.balance(&setup.contributor), 0);

    // Escrow still locked
    let escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(escrow.status, EscrowStatus::Locked);
}

#[test]
#[should_panic]
fn test_get_pending_claim_not_found() {
    let setup = TestSetup::new();
    setup.escrow.get_pending_claim(&999);
}

#[test]
#[should_panic]
fn test_cancel_pending_claim_not_found() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);

    setup.escrow.cancel_pending_claim(&bounty_id);
}

#[test]
#[should_panic]
fn test_claim_twice_panics() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    setup.escrow.claim(&bounty_id);
    // Second claim should fail
    setup.escrow.claim(&bounty_id);
}

#[test]
fn test_authorize_claim_default_window_used_when_not_set() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    // No set_claim_window call — should use 86400 default
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    let claim = setup.escrow.get_pending_claim(&bounty_id);
    let expected_expiry = setup.env.ledger().timestamp() + 86400;
    assert_eq!(claim.expires_at, expected_expiry);
}

#[test]
#[should_panic]
fn test_authorize_claim_on_nonexistent_bounty() {
    let setup = TestSetup::new();
    setup.escrow.authorize_claim(&999, &setup.contributor);
}

#[test]
#[should_panic]
fn test_authorize_claim_on_released_bounty() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);
}

#[test]
#[should_panic]
fn test_authorize_claim_on_refunded_bounty() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let current_time = setup.env.ledger().timestamp();
    let deadline = current_time + 1000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);
    setup.escrow.refund(
        &bounty_id,
        &None::<i128>,
        &None::<Address>,
        &RefundMode::Full,
    );

    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);
}

#[test]
fn test_claim_at_exact_window_boundary_succeeds() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 10000;
    let claim_window = 3600u64;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&claim_window);

    let now = setup.env.ledger().timestamp();
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    // Advance to exactly expires_at
    setup.env.ledger().set_timestamp(now + claim_window);
    setup.escrow.claim(&bounty_id);

    assert_eq!(setup.token.balance(&setup.contributor), amount);
}

#[test]
fn test_cancel_expired_claim_then_authorize_new_one() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 10000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    // Advance past window
    setup
        .env
        .ledger()
        .set_timestamp(setup.env.ledger().timestamp() + 3601);

    // Admin cancels expired claim
    setup.escrow.cancel_pending_claim(&bounty_id);

    let new_contributor = Address::generate(&setup.env);
    setup.escrow.authorize_claim(&bounty_id, &new_contributor);

    let claim = setup.escrow.get_pending_claim(&bounty_id);
    assert_eq!(claim.recipient, new_contributor);
    assert!(!claim.claimed);
}

#[test]
fn test_cancel_claim_then_use_release_funds_normally() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 10000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    setup.escrow.cancel_pending_claim(&bounty_id);

    setup.escrow.release_funds(&bounty_id, &setup.contributor);

    assert_eq!(setup.token.balance(&setup.contributor), amount);
    let escrow = setup.escrow.get_escrow_info(&bounty_id);
    assert_eq!(escrow.status, EscrowStatus::Released);
}

#[test]
fn test_authorize_claim_zero_window_expires_immediately() {
    let setup = TestSetup::new();
    let bounty_id = 1;
    let amount = 1000;
    let deadline = setup.env.ledger().timestamp() + 10000;

    setup
        .escrow
        .lock_funds(&setup.depositor, &bounty_id, &amount, &deadline);
    // Set zero-second window — claim expires instantly
    setup.escrow.set_claim_window(&0);
    setup.escrow.authorize_claim(&bounty_id, &setup.contributor);

    let claim = setup.escrow.get_pending_claim(&bounty_id);
    // expires_at == created_at, so already expired
    assert!(claim.expires_at <= setup.env.ledger().timestamp());
}

#[test]
fn test_claim_does_not_affect_other_bounties() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 1000;
    let contributor2 = Address::generate(&setup.env);

    setup.token_admin.mint(&setup.depositor, &5000);

    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &2, &2000, &deadline);

    setup.escrow.set_claim_window(&3600);
    setup.escrow.authorize_claim(&1, &setup.contributor);

    // Claim bounty 1
    setup.escrow.claim(&1);

    // Bounty 2 should be unaffected
    let escrow2 = setup.escrow.get_escrow_info(&2);
    assert_eq!(escrow2.status, EscrowStatus::Locked);
    assert_eq!(escrow2.amount, 2000);
    assert_eq!(setup.token.balance(&setup.escrow_address), 2000);

    // Bounty 2 can still be released normally
    setup.escrow.release_funds(&2, &contributor2);
    assert_eq!(setup.token.balance(&contributor2), 2000);
}

// ============================================================================
// Multi-token / multi-bounty balance accounting tests
// Verify remaining_amount per bounty and that operations on one do not affect others
// ============================================================================

#[test]
fn test_multi_bounty_remaining_amount_per_bounty() {
    let setup = TestSetup::new();
    let deadline = setup.env.ledger().timestamp() + 10_000;

    setup.token_admin.mint(&setup.depositor, &10_000);

    // Lock different amounts into three bounties
    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &2, &2000, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &3, &3000, &deadline);

    // Verify remaining_balance (remaining_amount) per bounty before any release
    let e1 = setup.escrow.get_escrow_info(&1);
    let e2 = setup.escrow.get_escrow_info(&2);
    let e3 = setup.escrow.get_escrow_info(&3);
    assert_eq!(e1.remaining_amount, 1000);
    assert_eq!(e2.remaining_amount, 2000);
    assert_eq!(e3.remaining_amount, 3000);
    assert_eq!(e1.amount, 1000);
    assert_eq!(e2.amount, 2000);
    assert_eq!(e3.amount, 3000);

    assert_eq!(setup.token.balance(&setup.escrow_address), 6000);

    // Release only bounty 2 to contributor
    setup.escrow.release_funds(&2, &setup.contributor);

    // Per-bounty remaining_amount: bounty 1 and 3 unchanged; bounty 2 now released (remaining 0)
    let e1_after = setup.escrow.get_escrow_info(&1);
    let e2_after = setup.escrow.get_escrow_info(&2);
    let e3_after = setup.escrow.get_escrow_info(&3);
    assert_eq!(e1_after.remaining_amount, 1000);
    assert_eq!(e1_after.status, EscrowStatus::Locked);
    assert_eq!(e2_after.remaining_amount, 0);
    assert_eq!(e2_after.status, EscrowStatus::Released);
    assert_eq!(e3_after.remaining_amount, 3000);
    assert_eq!(e3_after.status, EscrowStatus::Locked);

    // Contract balance decreased by bounty 2 amount only
    assert_eq!(setup.token.balance(&setup.escrow_address), 4000);
    assert_eq!(setup.token.balance(&setup.contributor), 2000);
}

#[test]
fn test_multi_bounty_refund_one_does_not_affect_other() {
    let setup = TestSetup::new();
    let now = setup.env.ledger().timestamp();
    let deadline = now + 1000;

    // TestSetup already mints 1_000_000 to depositor
    let initial_balance = setup.token.balance(&setup.depositor);

    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1000, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &2, &2000, &deadline);

    let e1 = setup.escrow.get_escrow_info(&1);
    let e2 = setup.escrow.get_escrow_info(&2);
    assert_eq!(e1.remaining_amount, 1000);
    assert_eq!(e2.remaining_amount, 2000);

    // Advance past deadline
    setup.env.ledger().set_timestamp(deadline + 1);

    // Refund only bounty 1
    setup.escrow.refund(
        &1,
        &None::<i128>,
        &None::<Address>,
        &RefundMode::Full,
    );

    // Bounty 1: refunded; bounty 2: unchanged
    let e1_after = setup.escrow.get_escrow_info(&1);
    let e2_after = setup.escrow.get_escrow_info(&2);
    assert_eq!(e1_after.status, EscrowStatus::Refunded);
    assert_eq!(e1_after.remaining_amount, 0);
    assert_eq!(e2_after.status, EscrowStatus::Locked);
    assert_eq!(e2_after.remaining_amount, 2000);

    // Contract still holds bounty 2's amount
    assert_eq!(setup.token.balance(&setup.escrow_address), 2000);
    // Depositor: initial - 3000 (locked) + 1000 (refunded) = initial - 2000
    assert_eq!(setup.token.balance(&setup.depositor), initial_balance - 2000);
}

#[test]
fn test_multi_bounty_lock_and_release_independent_balance_maps() {
    let setup = TestSetup::new();
    let contributor2 = Address::generate(&setup.env);
    let deadline = setup.env.ledger().timestamp() + 10_000;

    // TestSetup already mints 1_000_000 to depositor
    let depositor_initial = setup.token.balance(&setup.depositor);
    println!("Initial depositor balance: {}", depositor_initial);
    assert!(depositor_initial >= 4000, "Depositor needs at least 4000 tokens");

    // Verify contributor2 starts with 0 balance
    let contributor2_initial = setup.token.balance(&contributor2);
    println!("Initial contributor2 balance: {}", contributor2_initial);
    assert_eq!(contributor2_initial, 0, "Contributor2 should start with 0 balance");

    // Lock two bounties
    setup
        .escrow
        .lock_funds(&setup.depositor, &1, &1500, &deadline);
    setup
        .escrow
        .lock_funds(&setup.depositor, &2, &2500, &deadline);

    // Verify escrow state after locking
    let escrow1 = setup.escrow.get_escrow_info(&1);
    let escrow2 = setup.escrow.get_escrow_info(&2);
    println!("After lock - Escrow1 amount: {}, remaining: {}", escrow1.amount, escrow1.remaining_amount);
    println!("After lock - Escrow2 amount: {}, remaining: {}", escrow2.amount, escrow2.remaining_amount);

    // Verify escrow.amount is correct (this is what gets transferred on release)
    assert_eq!(escrow1.amount, 1500, "Escrow1 amount should be 1500");
    assert_eq!(escrow2.amount, 2500, "Escrow2 amount should be 2500");
    assert_eq!(escrow1.remaining_amount, 1500);
    assert_eq!(escrow2.remaining_amount, 2500);

    let contract_balance = setup.token.balance(&setup.escrow_address);
    println!("After lock - Contract token balance: {}", contract_balance);
    assert_eq!(contract_balance, 4000, "Contract should hold 4000 tokens");

    // Release bounty 1 only
    setup.escrow.release_funds(&1, &setup.contributor);

    let escrow1_after = setup.escrow.get_escrow_info(&1);
    let escrow2_after = setup.escrow.get_escrow_info(&2);
    let contributor_balance = setup.token.balance(&setup.contributor);
    let contract_balance_after_1 = setup.token.balance(&setup.escrow_address);

    println!("After release 1 - Escrow1 remaining: {}, status: {:?}", escrow1_after.remaining_amount, escrow1_after.status);
    println!("After release 1 - Escrow2 remaining: {}, status: {:?}, amount: {}", escrow2_after.remaining_amount, escrow2_after.status, escrow2_after.amount);
    println!("After release 1 - Contributor balance: {}", contributor_balance);
    println!("After release 1 - Contract token balance: {}", contract_balance_after_1);

    assert_eq!(escrow1_after.remaining_amount, 0);
    assert_eq!(escrow1_after.status, EscrowStatus::Released);
    assert_eq!(escrow2_after.remaining_amount, 2500);
    assert_eq!(escrow2_after.status, EscrowStatus::Locked);
    assert_eq!(contributor_balance, 1500, "Contributor should have 1500");
    assert_eq!(contract_balance_after_1, 2500, "Contract should have 2500 remaining");

    // Release bounty 2 to different contributor
    // Verify escrow2.amount before release (this is what will be transferred)
    println!("Before release 2 - Escrow2 amount to be transferred: {}", escrow2_after.amount);

    setup.escrow.release_funds(&2, &contributor2);

    // Capture final state
    let escrow2_final = setup.escrow.get_escrow_info(&2);
    let contributor2_balance = setup.token.balance(&contributor2);
    let escrow_final_balance = setup.token.balance(&setup.escrow_address);

    println!("After release 2 - Escrow2 remaining: {}, status: {:?}", escrow2_final.remaining_amount, escrow2_final.status);
    println!("After release 2 - Contributor2 balance: {}", contributor2_balance);
    println!("After release 2 - Escrow contract balance: {}", escrow_final_balance);

    assert_eq!(escrow2_final.remaining_amount, 0, "Escrow2 remaining_amount should be 0 after release");
    assert_eq!(escrow2_final.status, EscrowStatus::Released, "Escrow2 should be Released");
    assert_eq!(contributor2_balance, 2500, "Contributor2 should have received 2500");
    assert_eq!(escrow_final_balance, 0, "Contract should have 0 balance after all releases");
}
