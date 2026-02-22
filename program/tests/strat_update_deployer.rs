mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, StrategyDeployer};
use evore::instruction::{create_strat_deployer, update_strat_deployer};
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

// ============================================================================
// Helper: create a strat deployer then return the context for update tests
// ============================================================================

async fn setup_with_strat_deployer(
    bps_fee: u64,
    flat_fee: u64,
    max_per_round: u64,
    strategy_type: u8,
    strategy_data: [u8; 64],
) -> (
    solana_program_test::ProgramTestContext,
    Keypair,  // authority (manager authority)
    Pubkey,   // manager pubkey
    Keypair,  // deploy_authority
    Pubkey,   // strat_deployer PDA
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();
    let (strat_deployer_pda_addr, _) = strategy_deployer_pda(manager.pubkey());

    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &authority.pubkey(), 1_000_000_000,
    );
    let fund_ix2 = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &deploy_authority.pubkey(), 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix, fund_ix2], &[&payer])
        .await.unwrap();

    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(),
        manager.pubkey(),
        deploy_authority.pubkey(),
        bps_fee,
        flat_fee,
        max_per_round,
        strategy_type,
        strategy_data,
    );

    send_transaction(&mut context, &[ix], &[&payer, &authority])
        .await.unwrap();

    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, authority, manager.pubkey(), deploy_authority, strat_deployer_pda_addr)
}

// ============================================================================
// Manager updates expected fees
// ============================================================================

#[tokio::test]
async fn test_manager_updates_expected_bps_fee() {
    let (mut context, authority, manager, deploy_authority, strat_pda) =
        setup_with_strat_deployer(100, 50, 1_000_000_000, 2, manual_strategy_data()).await;
    let payer = context.payer.insecure_clone();

    let ix = update_strat_deployer(
        authority.pubkey(),
        manager,
        deploy_authority.pubkey(),
        100,   // bps_fee (unchanged, manager can't set this)
        50,    // flat_fee (unchanged)
        200,   // new expected_bps_fee
        100,   // new expected_flat_fee
        2_000_000_000,  // new max_per_round
        2,     // strategy_type unchanged
        manual_strategy_data(),
    );

    send_transaction(&mut context, &[ix], &[&payer, &authority])
        .await.unwrap();

    let state = get_strat_deployer_state(&mut context.banks_client, strat_pda).await;
    assert_eq!(state.expected_bps_fee, 200);
    assert_eq!(state.expected_flat_fee, 100);
    assert_eq!(state.max_per_round, 2_000_000_000);
}

// ============================================================================
// Deploy authority updates actual fees
// ============================================================================

#[tokio::test]
async fn test_deploy_authority_updates_bps_fee() {
    let (mut context, _authority, manager, deploy_authority, strat_pda) =
        setup_with_strat_deployer(100, 50, 1_000_000_000, 2, manual_strategy_data()).await;
    let payer = context.payer.insecure_clone();

    let ix = update_strat_deployer(
        deploy_authority.pubkey(),
        manager,
        deploy_authority.pubkey(),
        50,    // new bps_fee (lowered from 100)
        25,    // new flat_fee (lowered from 50)
        100,   // expected_bps_fee (deploy_authority can't change)
        50,    // expected_flat_fee (deploy_authority can't change)
        1_000_000_000,
        2,
        manual_strategy_data(),
    );

    send_transaction(&mut context, &[ix], &[&payer, &deploy_authority])
        .await.unwrap();

    let state = get_strat_deployer_state(&mut context.banks_client, strat_pda).await;
    assert_eq!(state.bps_fee, 50);
    assert_eq!(state.flat_fee, 25);
    // Expected fees should NOT have changed
    assert_eq!(state.expected_bps_fee, 100);
    assert_eq!(state.expected_flat_fee, 50);
}

// ============================================================================
// Manager updates strategy
// ============================================================================

#[tokio::test]
async fn test_manager_updates_strategy_to_ev() {
    let (mut context, authority, manager, deploy_authority, strat_pda) =
        setup_with_strat_deployer(0, 0, 1_000_000_000, 2, manual_strategy_data()).await;
    let payer = context.payer.insecure_clone();

    let new_strategy_data = ev_strategy_data(500_000, 10_000, 100, 2_000_000_000);

    let ix = update_strat_deployer(
        authority.pubkey(),
        manager,
        deploy_authority.pubkey(),
        0, 0, 0, 0,
        1_000_000_000,
        0,  // EV strategy
        new_strategy_data,
    );

    send_transaction(&mut context, &[ix], &[&payer, &authority])
        .await.unwrap();

    let state = get_strat_deployer_state(&mut context.banks_client, strat_pda).await;
    assert_eq!(state.strategy_type, 0);
    assert_eq!(state.strategy_data, new_strategy_data);
}

// ============================================================================
// Invalid strategy data rejected
// ============================================================================

#[tokio::test]
async fn test_update_invalid_strategy_data_fails() {
    let (mut context, authority, manager, deploy_authority, _strat_pda) =
        setup_with_strat_deployer(0, 0, 1_000_000_000, 2, manual_strategy_data()).await;
    let payer = context.payer.insecure_clone();

    let bad_data = ev_strategy_data(100_000, 0, 50, 1_000_000_000);

    let ix = update_strat_deployer(
        authority.pubkey(),
        manager,
        deploy_authority.pubkey(),
        0, 0, 0, 0,
        1_000_000_000,
        0,  // EV
        bad_data,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &authority]).await;
    assert!(result.is_err(), "Invalid strategy data must be rejected on update");
}

// ============================================================================
// Wrong authority rejected
// ============================================================================

#[tokio::test]
async fn test_update_wrong_authority_fails() {
    let (mut context, _authority, manager, _deploy_authority, _strat_pda) =
        setup_with_strat_deployer(0, 0, 1_000_000_000, 2, manual_strategy_data()).await;
    let payer = context.payer.insecure_clone();

    let wrong_signer = Keypair::new();
    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &wrong_signer.pubkey(), 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix], &[&payer])
        .await.unwrap();

    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = update_strat_deployer(
        wrong_signer.pubkey(),
        manager,
        wrong_signer.pubkey(),
        0, 0, 0, 0,
        1_000_000_000,
        2,
        manual_strategy_data(),
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &wrong_signer]).await;
    assert!(result.is_err(), "Wrong authority must be rejected");
}
