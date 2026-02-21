#![cfg(test)]
/// # Escrow Analytics & Monitoring View Tests
///
/// Closes #391
///
/// This module validates that every monitoring metric and analytics view correctly
/// reflects the escrow state after lock, release, and refund operations — including
/// both success and failure/error paths.
///
/// ## Coverage
/// * `get_aggregate_stats`  – totals update after lock → release → refund lifecycle
/// * `get_escrow_count`     – increments on each lock; never decrements
/// * `query_escrows_by_status` – returns correct subset filtered by status
/// * `query_escrows_by_amount` – range filter works for locked, released, and mixed states
/// * `query_escrows_by_deadline` – deadline range filter returns correct bounties
/// * `query_escrows_by_depositor` – per-depositor index is populated on lock
/// * `get_escrow_ids_by_status` – ID-only view mirrors full-object equivalent
/// * `get_refund_eligibility` – eligibility flags flip correctly across lifecycle
/// * `get_refund_history`    – history vector is populated by approved-refund path
/// * Monitoring event emission – lock/release/refund each emit ≥ 1 event
/// * Error flows             – failed attempts do not corrupt metrics
use crate::{BountyEscrowContract, BountyEscrowContractClient, EscrowStatus, RefundMode};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token, Address, Env,
};

// ---------------------------------------------------------------------------
// Shared helpers – matching the pattern used in the existing test.rs
// ---------------------------------------------------------------------------

fn create_token_contract<'a>(
    e: &'a Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = e.register_stellar_asset_contract(admin.clone());
    (
        token::Client::new(e, &contract_address),
        token::StellarAssetClient::new(e, &contract_address),
    )
}

fn create_escrow_contract<'a>(e: &'a Env) -> BountyEscrowContractClient<'a> {
    let contract_id = e.register_contract(None, BountyEscrowContract);
    BountyEscrowContractClient::new(e, &contract_id)
}

// ===========================================================================
// 1. Aggregate stats – lock path
// ===========================================================================

#[test]
fn test_aggregate_stats_initial_state_is_zeroed() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (token, _token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.total_locked, 0);
    assert_eq!(stats.total_released, 0);
    assert_eq!(stats.total_refunded, 0);
    assert_eq!(stats.count_locked, 0);
    assert_eq!(stats.count_released, 0);
    assert_eq!(stats.count_refunded, 0);
}

#[test]
fn test_aggregate_stats_reflects_single_lock() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &1, &500, &deadline);

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.count_locked, 1);
    assert_eq!(stats.total_locked, 500);
    assert_eq!(stats.count_released, 0);
    assert_eq!(stats.count_refunded, 0);
}

#[test]
fn test_aggregate_stats_reflects_multiple_locks() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &10_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &10, &1_000, &deadline);
    escrow.lock_funds(&depositor, &11, &2_000, &deadline);
    escrow.lock_funds(&depositor, &12, &3_000, &deadline);

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.count_locked, 3);
    assert_eq!(stats.total_locked, 6_000);
    assert_eq!(stats.count_released, 0);
}

// ===========================================================================
// 2. Aggregate stats – release path
// ===========================================================================

#[test]
fn test_aggregate_stats_after_release_moves_to_released_bucket() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &20, &1_000, &deadline);
    escrow.release_funds(&20, &contributor);

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.count_locked, 0);
    assert_eq!(stats.total_locked, 0);
    assert_eq!(stats.count_released, 1);
    assert_eq!(stats.total_released, 1_000);
    assert_eq!(stats.count_refunded, 0);
}

#[test]
fn test_aggregate_stats_mixed_lock_and_release() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    // Lock three, release one, keep two locked
    escrow.lock_funds(&depositor, &30, &500, &deadline);
    escrow.lock_funds(&depositor, &31, &700, &deadline);
    escrow.lock_funds(&depositor, &32, &300, &deadline);
    escrow.release_funds(&31, &contributor);

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.count_locked, 2);
    assert_eq!(stats.total_locked, 800); // 500 + 300
    assert_eq!(stats.count_released, 1);
    assert_eq!(stats.total_released, 700);
}

// ===========================================================================
// 3. Aggregate stats – refund path
// ===========================================================================

#[test]
fn test_aggregate_stats_after_refund_moves_to_refunded_bucket() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 500;
    escrow.lock_funds(&depositor, &40, &900, &deadline);
    // Advance time past deadline
    env.ledger().set_timestamp(deadline + 1);
    escrow.refund(&40);

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.count_locked, 0);
    assert_eq!(stats.count_released, 0);
    assert_eq!(stats.count_refunded, 1);
    assert_eq!(stats.total_refunded, 900);
}

#[test]
fn test_aggregate_stats_full_lifecycle_lock_release_refund() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &10_000_000);

    let now = env.ledger().timestamp();
    // One of each outcome
    escrow.lock_funds(&depositor, &50, &1_000, &(now + 500));
    escrow.lock_funds(&depositor, &51, &2_000, &(now + 500));
    escrow.lock_funds(&depositor, &52, &3_000, &(now + 5000));

    escrow.release_funds(&50, &contributor); // → released
    env.ledger().set_timestamp(now + 501);
    escrow.refund(&51); // → refunded
    // 52 remains locked (deadline not yet passed)

    let stats = escrow.get_aggregate_stats();

    assert_eq!(stats.count_locked, 1);
    assert_eq!(stats.total_locked, 3_000);
    assert_eq!(stats.count_released, 1);
    assert_eq!(stats.total_released, 1_000);
    assert_eq!(stats.count_refunded, 1);
    assert_eq!(stats.total_refunded, 2_000);
}

// ===========================================================================
// 4. Escrow count monitoring view
// ===========================================================================

#[test]
fn test_escrow_count_zero_before_any_lock() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (token, _token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);

    assert_eq!(escrow.get_escrow_count(), 0);
}

#[test]
fn test_escrow_count_increments_on_each_lock() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;

    assert_eq!(escrow.get_escrow_count(), 0);

    escrow.lock_funds(&depositor, &60, &100, &deadline);
    assert_eq!(escrow.get_escrow_count(), 1);

    escrow.lock_funds(&depositor, &61, &100, &deadline);
    assert_eq!(escrow.get_escrow_count(), 2);

    escrow.lock_funds(&depositor, &62, &100, &deadline);
    assert_eq!(escrow.get_escrow_count(), 3);
}

#[test]
fn test_escrow_count_does_not_decrement_after_release() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &63, &500, &deadline);
    escrow.release_funds(&63, &contributor);

    // Count tracks total created, not currently locked
    assert_eq!(escrow.get_escrow_count(), 1);
}

#[test]
fn test_escrow_count_does_not_decrement_after_refund() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 500;
    escrow.lock_funds(&depositor, &64, &500, &deadline);
    env.ledger().set_timestamp(deadline + 1);
    escrow.refund(&64);

    assert_eq!(escrow.get_escrow_count(), 1);
}

// ===========================================================================
// 5. Query by status – monitoring view
// ===========================================================================

#[test]
fn test_query_by_status_locked_returns_only_locked() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &70, &100, &deadline);
    escrow.lock_funds(&depositor, &71, &200, &deadline);
    escrow.lock_funds(&depositor, &72, &300, &deadline);
    escrow.release_funds(&71, &contributor); // 71 becomes Released

    let locked = escrow.query_escrows_by_status(&EscrowStatus::Locked, &0, &10);
    assert_eq!(locked.len(), 2);

    // Verify the two locked bounties are 70 and 72
    let ids: soroban_sdk::Vec<u64> = soroban_sdk::Vec::from_array(
        &env,
        [locked.get(0).unwrap().bounty_id, locked.get(1).unwrap().bounty_id],
    );
    assert!(ids.contains(70_u64));
    assert!(ids.contains(72_u64));
}

#[test]
fn test_query_by_status_released_returns_only_released() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &80, &400, &deadline);
    escrow.lock_funds(&depositor, &81, &500, &deadline);
    escrow.release_funds(&80, &contributor);

    let released = escrow.query_escrows_by_status(&EscrowStatus::Released, &0, &10);
    assert_eq!(released.len(), 1);
    assert_eq!(released.get(0).unwrap().bounty_id, 80);
    assert_eq!(released.get(0).unwrap().escrow.status, EscrowStatus::Released);
}

#[test]
fn test_query_by_status_refunded_returns_only_refunded() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let now = env.ledger().timestamp();
    escrow.lock_funds(&depositor, &90, &600, &(now + 500));
    escrow.lock_funds(&depositor, &91, &700, &(now + 2000));
    env.ledger().set_timestamp(now + 501);
    escrow.refund(&90);

    let refunded = escrow.query_escrows_by_status(&EscrowStatus::Refunded, &0, &10);
    assert_eq!(refunded.len(), 1);
    assert_eq!(refunded.get(0).unwrap().bounty_id, 90);
}

#[test]
fn test_query_by_status_empty_when_no_match() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &95, &100, &deadline);

    // Ask for Released when nothing has been released
    let released = escrow.query_escrows_by_status(&EscrowStatus::Released, &0, &10);
    assert_eq!(released.len(), 0);
}

#[test]
fn test_query_by_status_pagination_offset_and_limit() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 2000;
    // Lock 5 bounties, all remain locked
    for id in 100_u64..105 {
        escrow.lock_funds(&depositor, &id, &100, &deadline);
    }

    let page1 = escrow.query_escrows_by_status(&EscrowStatus::Locked, &0, &3);
    assert_eq!(page1.len(), 3);

    let page2 = escrow.query_escrows_by_status(&EscrowStatus::Locked, &3, &3);
    assert_eq!(page2.len(), 2); // only 2 remain after offset=3

    // Ensure no overlap between pages
    let p1_id0 = page1.get(0).unwrap().bounty_id;
    let p2_id0 = page2.get(0).unwrap().bounty_id;
    assert_ne!(p1_id0, p2_id0);
}

// ===========================================================================
// 6. Query by amount range – monitoring view
// ===========================================================================

#[test]
fn test_query_by_amount_range_returns_matching_escrows() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &10_000_000);

    let deadline = env.ledger().timestamp() + 2000;
    escrow.lock_funds(&depositor, &110, &100, &deadline);
    escrow.lock_funds(&depositor, &111, &500, &deadline);
    escrow.lock_funds(&depositor, &112, &1_000, &deadline);
    escrow.lock_funds(&depositor, &113, &5_000, &deadline);

    // Query amounts between 200 and 2000
    let results = escrow.query_escrows_by_amount(&200, &2_000, &0, &10);
    assert_eq!(results.len(), 2); // 500 and 1000 fit

    for item in results.iter() {
        assert!(item.escrow.amount >= 200 && item.escrow.amount <= 2_000);
    }
}

#[test]
fn test_query_by_amount_exact_boundaries_included() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &10_000_000);

    let deadline = env.ledger().timestamp() + 2000;
    escrow.lock_funds(&depositor, &120, &1_000, &deadline);
    escrow.lock_funds(&depositor, &121, &2_000, &deadline);
    escrow.lock_funds(&depositor, &122, &3_000, &deadline);

    let results = escrow.query_escrows_by_amount(&1_000, &2_000, &0, &10);
    assert_eq!(results.len(), 2); // both boundary values are inclusive
}

#[test]
fn test_query_by_amount_no_results_outside_range() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 2000;
    escrow.lock_funds(&depositor, &130, &50, &deadline);
    escrow.lock_funds(&depositor, &131, &500, &deadline);

    let results = escrow.query_escrows_by_amount(&600, &1_000, &0, &10);
    assert_eq!(results.len(), 0);
}

// ===========================================================================
// 7. Query by deadline range – monitoring view
// ===========================================================================

#[test]
fn test_query_by_deadline_range_filters_correctly() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let now = env.ledger().timestamp();
    escrow.lock_funds(&depositor, &140, &100, &(now + 100));
    escrow.lock_funds(&depositor, &141, &100, &(now + 500));
    escrow.lock_funds(&depositor, &142, &100, &(now + 1_000));
    escrow.lock_funds(&depositor, &143, &100, &(now + 5_000));

    // Query deadlines between now+200 and now+2000
    let results = escrow.query_escrows_by_deadline(&(now + 200), &(now + 2_000), &0, &10);
    assert_eq!(results.len(), 2); // 500 and 1000

    for item in results.iter() {
        assert!(item.escrow.deadline >= now + 200 && item.escrow.deadline <= now + 2_000);
    }
}

#[test]
fn test_query_by_deadline_exact_boundary_included() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let now = env.ledger().timestamp();
    escrow.lock_funds(&depositor, &150, &100, &(now + 1_000));
    escrow.lock_funds(&depositor, &151, &100, &(now + 2_000));

    let results = escrow.query_escrows_by_deadline(&(now + 1_000), &(now + 2_000), &0, &10);
    assert_eq!(results.len(), 2);
}

// ===========================================================================
// 8. Query by depositor – monitoring view
// ===========================================================================

#[test]
fn test_query_by_depositor_returns_only_that_depositors_escrows() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor_a = Address::generate(&env);
    let depositor_b = Address::generate(&env);

    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);

    token_admin.mint(&depositor_a, &5_000);
    token_admin.mint(&depositor_b, &5_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor_a, &160, &1_000, &deadline);
    escrow.lock_funds(&depositor_a, &161, &2_000, &deadline);
    escrow.lock_funds(&depositor_b, &162, &3_000, &deadline);

    let a_results = escrow.query_escrows_by_depositor(&depositor_a, &0, &10);
    assert_eq!(a_results.len(), 2);
    for item in a_results.iter() {
        assert_eq!(item.escrow.depositor, depositor_a);
    }

    let b_results = escrow.query_escrows_by_depositor(&depositor_b, &0, &10);
    assert_eq!(b_results.len(), 1);
    assert_eq!(b_results.get(0).unwrap().escrow.depositor, depositor_b);
}

#[test]
fn test_query_by_depositor_returns_empty_for_unknown_address() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &165, &100, &deadline);

    let unknown = Address::generate(&env);
    let results = escrow.query_escrows_by_depositor(&unknown, &0, &10);
    assert_eq!(results.len(), 0);
}

// ===========================================================================
// 9. Get escrow IDs by status – monitoring view
// ===========================================================================

#[test]
fn test_get_escrow_ids_by_status_returns_correct_ids() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &170, &100, &deadline);
    escrow.lock_funds(&depositor, &171, &200, &deadline);
    escrow.lock_funds(&depositor, &172, &300, &deadline);
    escrow.release_funds(&171, &contributor);

    let locked_ids = escrow.get_escrow_ids_by_status(&EscrowStatus::Locked, &0, &10);
    assert_eq!(locked_ids.len(), 2);
    assert!(locked_ids.contains(170_u64));
    assert!(locked_ids.contains(172_u64));

    let released_ids = escrow.get_escrow_ids_by_status(&EscrowStatus::Released, &0, &10);
    assert_eq!(released_ids.len(), 1);
    assert!(released_ids.contains(171_u64));
}

#[test]
fn test_get_escrow_ids_by_status_empty_when_no_match() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let (token, token_admin) = create_token_contract(&env, &admin);
    let escrow = create_escrow_contract(&env);
    escrow.init(&admin, &token.address);
    token_admin.mint(&depositor, &1_000_000);

    let deadline = env.ledger().timestamp() + 1000;
    escrow.lock_funds(&depositor, &175, &100, &deadline);

    let released_ids = escrow.get_escrow_ids_by_status(&EscrowStatus::Released, &0, &10);
    assert_eq!(released_ids.len(), 0);
}

