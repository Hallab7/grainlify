use super::*;
use soroban_sdk::{
    testutils::Address as _, token, vec, Address, Env, String,
};

fn setup_program(env: &Env) -> (ProgramEscrowContractClient<'_>, Address, Address, Address) {
    env.mock_all_auths();

    let contract_id = env.register(ProgramEscrowContract, ());
    let contract = ProgramEscrowContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin);
    let token_address = token_contract.address();

    let token_admin_client = token::StellarAssetClient::new(env, &token_address);
    token_admin_client.mint(&contract_id, &1_000_000_000_000i128);

    let program_id = String::from_str(env, "hackathon-2024-q1");
    contract.init_program(&program_id, &admin, &token_address);

    (contract, contract_id, admin, token_address)
}

#[test]
fn test_nonce_starts_at_zero() {
    let env = Env::default();
    let (contract, _contract_id, admin, _token_address) = setup_program(&env);

    assert_eq!(contract.get_nonce(&admin), 0);
}

#[test]
fn test_nonce_increments_on_single_payout() {
    let env = Env::default();
    let (contract, _contract_id, admin, _token_address) = setup_program(&env);
    let recipient = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);
    assert_eq!(contract.get_nonce(&admin), 0);

    contract.single_payout(&recipient, &10_000_000_000, &0);
    assert_eq!(contract.get_nonce(&admin), 1);

    contract.single_payout(&recipient, &10_000_000_000, &1);
    assert_eq!(contract.get_nonce(&admin), 2);
}

#[test]
fn test_nonce_increments_on_batch_payout() {
    let env = Env::default();
    let (contract, _contract_id, admin, _token_address) = setup_program(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);

    let recipients = vec![&env, recipient1, recipient2];
    let amounts = vec![&env, 10_000_000_000, 20_000_000_000];

    contract.batch_payout(&recipients, &amounts, &0);
    assert_eq!(contract.get_nonce(&admin), 1);
}

#[test]
#[should_panic(expected = "Invalid nonce")]
fn test_replay_attack_single_payout() {
    let env = Env::default();
    let (contract, _contract_id, _admin, _token_address) = setup_program(&env);
    let recipient = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);

    contract.single_payout(&recipient, &10_000_000_000, &0);
    contract.single_payout(&recipient, &10_000_000_000, &0);
}

#[test]
#[should_panic(expected = "Invalid nonce")]
fn test_replay_attack_batch_payout() {
    let env = Env::default();
    let (contract, _contract_id, _admin, _token_address) = setup_program(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);

    let recipients = vec![&env, recipient1, recipient2];
    let amounts = vec![&env, 10_000_000_000, 20_000_000_000];

    contract.batch_payout(&recipients, &amounts, &0);
    contract.batch_payout(&recipients, &amounts, &0);
}

#[test]
#[should_panic(expected = "Invalid nonce")]
fn test_cross_entrypoint_replay_single_then_batch() {
    let env = Env::default();
    let (contract, _contract_id, admin, _token_address) = setup_program(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);

    contract.single_payout(&recipient1, &10_000_000_000, &0);
    assert_eq!(contract.get_nonce(&admin), 1);

    let recipients = vec![&env, recipient1, recipient2];
    let amounts = vec![&env, 10_000_000_000, 20_000_000_000];

    contract.batch_payout(&recipients, &amounts, &0);
}

#[test]
#[should_panic(expected = "Invalid nonce")]
fn test_cross_entrypoint_replay_batch_then_single() {
    let env = Env::default();
    let (contract, _contract_id, admin, _token_address) = setup_program(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);

    let recipients = vec![&env, recipient1.clone(), recipient2];
    let amounts = vec![&env, 10_000_000_000, 20_000_000_000];
    contract.batch_payout(&recipients, &amounts, &0);
    assert_eq!(contract.get_nonce(&admin), 1);

    contract.single_payout(&recipient1, &10_000_000_000, &0);
}

#[test]
#[should_panic(expected = "Invalid nonce")]
fn test_out_of_order_nonce() {
    let env = Env::default();
    let (contract, _contract_id, _admin, _token_address) = setup_program(&env);
    let recipient = Address::generate(&env);

    contract.lock_program_funds(&100_000_000_000);
    contract.single_payout(&recipient, &10_000_000_000, &5);
}

#[test]
fn test_nonces_are_per_signer() {
    let env = Env::default();
    let (contract, _contract_id, _admin, _token_address) = setup_program(&env);
    let signer_a = Address::generate(&env);
    let signer_b = Address::generate(&env);

    assert_eq!(contract.get_nonce(&signer_a), 0);
    assert_eq!(contract.get_nonce(&signer_b), 0);
}

#[test]
#[should_panic(expected = "Invalid asset identifier")]
fn test_init_rejects_non_contract_asset_identifier() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ProgramEscrowContract, ());
    let contract = ProgramEscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let issuer_admin = Address::generate(&env);
    let stellar_asset = env.register_stellar_asset_contract_v2(issuer_admin);
    let invalid_asset_id = stellar_asset.issuer().address();
    let program_id = String::from_str(&env, "invalid-asset-id");

    contract.init_program(&program_id, &admin, &invalid_asset_id);
}
