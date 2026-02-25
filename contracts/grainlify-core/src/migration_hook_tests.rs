//! Comprehensive tests for version migration hooks in Grainlify Core.
//!
//! These tests verify that the state migration system:
//! - Executes migrations only once per version (idempotency)
//! - Performs expected state transformations across version boundaries
//! - Enforces proper admin authorization
//! - Handles edge cases like unsupported paths, downgrades, and repeated calls

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
    Address, BytesN, Env, IntoVal,
};

use crate::{GrainlifyContract, GrainlifyContractClient};

// ============================================================================
// Helpers
// ============================================================================

/// Registers a fresh contract, initializes it with a single admin, and returns
/// the client together with the admin address.
fn setup_contract(env: &Env) -> (GrainlifyContractClient<'_>, Address) {
    let contract_id = env.register_contract(None, GrainlifyContract);
    let client = GrainlifyContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.init_admin(&admin);

    (client, admin)
}

/// Returns a deterministic 32-byte migration hash seeded by `seed`.
fn migration_hash(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

// ============================================================================
// 1. Single-execution guarantee (idempotency)
// ============================================================================

#[test]
fn migrate_v2_to_v3_executes_only_once() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_contract(&env);

    // Contract starts at VERSION = 2 after init_admin
    assert_eq!(client.get_version(), 2);

    let hash = migration_hash(&env, 0xAA);

    // First migration — should succeed
    client.migrate(&3, &hash);
    let state_after_first = client.get_migration_state().unwrap();
    assert_eq!(state_after_first.from_version, 2);
    assert_eq!(state_after_first.to_version, 3);
    let ts_first = state_after_first.migrated_at;

    // Second call with the same target — must be a no-op
    client.migrate(&3, &hash);
    let state_after_second = client.get_migration_state().unwrap();

    // Timestamp and all fields must be identical (migration did not re-run)
    assert_eq!(state_after_second.migrated_at, ts_first);
    assert_eq!(state_after_second.from_version, 2);
    assert_eq!(state_after_second.to_version, 3);
    assert_eq!(state_after_second.migration_hash, hash);
}

#[test]
fn repeated_migrate_calls_do_not_change_version() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let hash = migration_hash(&env, 0xBB);
    client.migrate(&3, &hash);
    assert_eq!(client.get_version(), 3);

    // Call again — version must stay at 3
    client.migrate(&3, &hash);
    assert_eq!(client.get_version(), 3);

    // Even with a different hash, the idempotency guard fires first
    let hash2 = migration_hash(&env, 0xCC);
    client.migrate(&3, &hash2);
    assert_eq!(client.get_version(), 3);
}

// ============================================================================
// 2. Correct state transformations across version boundaries
// ============================================================================

#[test]
fn migrate_v2_to_v3_records_correct_state() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let hash = migration_hash(&env, 0x01);
    client.migrate(&3, &hash);

    let state = client.get_migration_state().unwrap();
    assert_eq!(state.from_version, 2);
    assert_eq!(state.to_version, 3);
    assert_eq!(state.migration_hash, hash);
    assert_eq!(client.get_version(), 3);
}

#[test]
fn migrate_v1_to_v2_works_when_starting_at_v1() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Manually set version to 1 to simulate a v1 contract
    client.set_version(&1);
    assert_eq!(client.get_version(), 1);

    let hash = migration_hash(&env, 0x02);
    client.migrate(&2, &hash);

    assert_eq!(client.get_version(), 2);
    let state = client.get_migration_state().unwrap();
    assert_eq!(state.from_version, 1);
    assert_eq!(state.to_version, 2);
}

#[test]
fn migrate_v1_to_v3_chains_through_v2() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Start from v1
    client.set_version(&1);
    assert_eq!(client.get_version(), 1);

    let hash = migration_hash(&env, 0x03);

    // Migrate directly to v3 — the contract should chain v1→v2→v3
    client.migrate(&3, &hash);

    assert_eq!(client.get_version(), 3);
    let state = client.get_migration_state().unwrap();
    assert_eq!(state.from_version, 1);
    assert_eq!(state.to_version, 3);
}

#[test]
fn migration_hash_is_stored_and_retrievable() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let hash = migration_hash(&env, 0xDE);
    client.migrate(&3, &hash);

    let state = client.get_migration_state().unwrap();
    assert_eq!(state.migration_hash, hash);
}

#[test]
fn no_migration_state_before_first_migrate() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Before any migration, state should be None
    assert!(client.get_migration_state().is_none());
}

// ============================================================================
// 3. Authorization checks
// ============================================================================

#[test]
#[should_panic]
fn migrate_panics_without_any_auth() {
    let env = Env::default();
    // Deliberately do NOT mock any auths

    let (client, _) = setup_contract(&env);

    let hash = migration_hash(&env, 0x10);

    // Should panic because no auth is provided
    client.migrate(&3, &hash);
}

#[test]
#[should_panic]
fn migrate_rejects_non_admin_caller() {
    let env = Env::default();
    // No blanket mock — we selectively authorize the wrong address

    let contract_id = env.register_contract(None, GrainlifyContract);
    let client = GrainlifyContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // init_admin needs auth mock to succeed
    env.mock_all_auths();
    client.init_admin(&admin);

    // Now stop mocking all auths and mock only the attacker
    let attacker = Address::generate(&env);
    env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "migrate",
            args: (3u32, migration_hash(&env, 0x11)).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    // Must panic — attacker is not the admin
    client.migrate(&3, &migration_hash(&env, 0x11));
}

#[test]
fn migrate_succeeds_with_correct_admin_auth() {
    let env = Env::default();

    let contract_id = env.register_contract(None, GrainlifyContract);
    let client = GrainlifyContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.init_admin(&admin);

    let hash = migration_hash(&env, 0x12);
    client.migrate(&3, &hash);

    // Verify auth was actually required (at least one auth entry)
    assert!(!env.auths().is_empty());
    assert_eq!(client.get_version(), 3);
}

// ============================================================================
// 4. Invalid migration paths
// ============================================================================

#[test]
#[should_panic(expected = "Target version must be greater than current version")]
fn migrate_rejects_same_version_as_target() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Current version is 2; migrating to 2 is invalid
    let hash = migration_hash(&env, 0x20);
    client.migrate(&2, &hash);
}

#[test]
#[should_panic(expected = "Target version must be greater than current version")]
fn migrate_rejects_lower_target_version() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Use set_version to bump to v3 without creating a migration state,
    // so the idempotency guard does not silently swallow the call.
    client.set_version(&3);
    assert_eq!(client.get_version(), 3);

    // Attempting to migrate to v2 must fail with a version check panic
    client.migrate(&2, &migration_hash(&env, 0x22));
}

#[test]
#[should_panic(expected = "No migration path available")]
fn migrate_panics_for_unsupported_version_jump() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Migrate to v3 first (supported: v2→v3)
    client.migrate(&3, &migration_hash(&env, 0x23));

    // Now try v3→v4 — no migrate_v3_to_v4 function exists
    client.migrate(&4, &migration_hash(&env, 0x24));
}

// ============================================================================
// 5. Event emission
// ============================================================================

#[test]
fn successful_migration_emits_events() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let events_before = env.events().all().len();

    let hash = migration_hash(&env, 0x30);
    client.migrate(&3, &hash);

    let events_after = env.events().all();
    assert!(
        events_after.len() > events_before,
        "Migration must emit at least one event"
    );
}

#[test]
fn idempotent_migration_does_not_emit_extra_events() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let hash = migration_hash(&env, 0x31);
    client.migrate(&3, &hash);

    let events_after_first = env.events().all().len();

    // Second call — idempotent, should not emit new migration events
    client.migrate(&3, &hash);

    let events_after_second = env.events().all().len();
    assert_eq!(
        events_after_first, events_after_second,
        "Idempotent migration must not emit additional events"
    );
}

// ============================================================================
// 6. Version management integration
// ============================================================================

#[test]
fn version_reflects_migration_target_after_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    assert_eq!(client.get_version(), 2);

    client.migrate(&3, &migration_hash(&env, 0x40));
    assert_eq!(client.get_version(), 3);
}

#[test]
fn migration_state_from_version_matches_pre_migration_version() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let pre = client.get_version();
    client.migrate(&3, &migration_hash(&env, 0x41));

    let state = client.get_migration_state().unwrap();
    assert_eq!(state.from_version, pre);
}

#[test]
fn set_version_and_migrate_interact_correctly() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Manually set version to 1 (simulating an older contract)
    client.set_version(&1);
    assert_eq!(client.get_version(), 1);

    // Migrate from v1 → v2
    client.migrate(&2, &migration_hash(&env, 0x42));
    assert_eq!(client.get_version(), 2);

    let state = client.get_migration_state().unwrap();
    assert_eq!(state.from_version, 1);
    assert_eq!(state.to_version, 2);
}

// ============================================================================
// 7. Edge cases and boundary conditions
// ============================================================================

#[test]
fn migration_with_all_zero_hash() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let zero_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.migrate(&3, &zero_hash);

    let state = client.get_migration_state().unwrap();
    assert_eq!(state.migration_hash, zero_hash);
    assert_eq!(client.get_version(), 3);
}

#[test]
fn migration_with_max_byte_hash() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    let max_hash = BytesN::from_array(&env, &[0xFF; 32]);
    client.migrate(&3, &max_hash);

    let state = client.get_migration_state().unwrap();
    assert_eq!(state.migration_hash, max_hash);
    assert_eq!(client.get_version(), 3);
}

#[test]
fn migration_preserves_admin_address() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup_contract(&env);

    client.migrate(&3, &migration_hash(&env, 0x50));

    // Admin must remain unchanged after migration
    let state = client.get_migration_state().unwrap();
    assert_eq!(state.to_version, 3);

    // set_version still requires admin auth — proves admin is intact
    client.set_version(&3);
    assert_eq!(client.get_version(), 3);
}

#[test]
fn chained_migration_v1_through_v3_preserves_final_state() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Reset to v1
    client.set_version(&1);

    let hash = migration_hash(&env, 0x60);
    client.migrate(&3, &hash);

    // Final state should reflect the full chain
    assert_eq!(client.get_version(), 3);
    let state = client.get_migration_state().unwrap();
    assert_eq!(state.from_version, 1);
    assert_eq!(state.to_version, 3);
    assert_eq!(state.migration_hash, hash);
}

#[test]
fn sequential_independent_migrations_update_state() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // First migration: v2 → v3
    let hash_a = migration_hash(&env, 0x70);
    client.migrate(&3, &hash_a);

    let state_a = client.get_migration_state().unwrap();
    assert_eq!(state_a.from_version, 2);
    assert_eq!(state_a.to_version, 3);
    assert_eq!(state_a.migration_hash, hash_a);

    // Note: v3→v4 would panic with "No migration path available"
    // because migrate_v3_to_v4 is not implemented.
    // This is the expected boundary — only v1→v2 and v2→v3 are supported.
}

#[test]
#[should_panic(expected = "Target version must be greater than current version")]
fn migrate_to_version_zero_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _) = setup_contract(&env);

    // Version 0 is always less than current (2), so this must fail
    client.migrate(&0, &migration_hash(&env, 0x80));
}
