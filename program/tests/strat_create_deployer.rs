mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, StrategyDeployer};
use evore::instruction::create_strat_deployer;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

// ============================================================================
// Happy path: Create with Manual strategy
// ============================================================================

#[tokio::test]
async fn test_create_manual_strategy_succeeds() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());
    fund_account(&mut program_test, authority.pubkey(), 10_000_000_000);

    let mut context = program_test.start_with_context().await;

    let (strat_deployer_pda, _) = strategy_deployer_pda(manager.pubkey());
    let strategy_data = manual_strategy_data();

    let ix = create_strat_deployer(
        authority.pubkey(),
        manager.pubkey(),
        deploy_authority.pubkey(),
        100,  // bps_fee
        50,   // flat_fee
        1_000_000_000, // max_per_round
        2,    // Manual strategy
        strategy_data,
    );

    let payer = context.payer.insecure_clone();
    let result = send_transaction(
        &mut context,
        &[ix],
        &[&payer, &authority],
    ).await;

    assert!(result.is_ok(), "CreateStratDeployer should succeed: {:?}", result.err());

    let state = get_strat_deployer_state(&mut context.banks_client, strat_deployer_pda).await;
    assert_eq!(state.manager_key, manager.pubkey());
    assert_eq!(state.deploy_authority, deploy_authority.pubkey());
    assert_eq!(state.bps_fee, 100);
    assert_eq!(state.flat_fee, 50);
    assert_eq!(state.expected_bps_fee, 100);
    assert_eq!(state.expected_flat_fee, 50);
    assert_eq!(state.max_per_round, 1_000_000_000);
    assert_eq!(state.strategy_type, 2);
    assert_eq!(state.strategy_data, strategy_data);
}

// ============================================================================
// Happy path: Create with EV strategy
// ============================================================================

#[tokio::test]
async fn test_create_ev_strategy_succeeds() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());
    fund_account(&mut program_test, authority.pubkey(), 10_000_000_000);

    let mut context = program_test.start_with_context().await;

    let strategy_data = ev_strategy_data(500_000, 10_000, 100, 2_000_000_000);

    let ix = create_strat_deployer(
        authority.pubkey(),
        manager.pubkey(),
        deploy_authority.pubkey(),
        0,
        0,
        2_000_000_000,
        0, // EV strategy
        strategy_data,
    );

    let payer = context.payer.insecure_clone();
    let result = send_transaction(
        &mut context,
        &[ix],
        &[&payer, &authority],
    ).await;

    assert!(result.is_ok(), "CreateStratDeployer with EV should succeed: {:?}", result.err());

    let (strat_deployer_pda, _) = strategy_deployer_pda(manager.pubkey());
    let state = get_strat_deployer_state(&mut context.banks_client, strat_deployer_pda).await;
    assert_eq!(state.strategy_type, 0);
    assert_eq!(state.strategy_data, strategy_data);
}

// ============================================================================
// Duplicate creation fails
// ============================================================================

#[tokio::test]
async fn test_create_duplicate_fails() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());
    fund_account(&mut program_test, authority.pubkey(), 10_000_000_000);

    let mut context = program_test.start_with_context().await;

    let ix = create_strat_deployer(
        authority.pubkey(),
        manager.pubkey(),
        deploy_authority.pubkey(),
        0, 0, 1_000_000_000,
        2, manual_strategy_data(),
    );

    // First create succeeds
    let payer = context.payer.insecure_clone();
    send_transaction(&mut context, &[ix.clone()], &[&payer, &authority])
        .await
        .unwrap();

    // Second create should fail
    context.warp_to_slot(2).unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let payer = context.payer.insecure_clone();
    let result = send_transaction(&mut context, &[ix], &[&payer, &authority]).await;
    assert!(result.is_err(), "Duplicate CreateStratDeployer must fail");
}

// ============================================================================
// Wrong authority rejected
// ============================================================================

#[tokio::test]
async fn test_create_wrong_authority_fails() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let real_authority = Keypair::new();
    let wrong_authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), real_authority.pubkey());

    let mut context = program_test.start_with_context().await;

    let ix = create_strat_deployer(
        wrong_authority.pubkey(),
        manager.pubkey(),
        deploy_authority.pubkey(),
        0, 0, 1_000_000_000,
        2, manual_strategy_data(),
    );

    let payer = context.payer.insecure_clone();
    let result = send_transaction(&mut context, &[ix], &[&payer, &wrong_authority]).await;
    assert!(result.is_err(), "Wrong authority must be rejected");
}

// ============================================================================
// Invalid strategy data rejected
// ============================================================================

#[tokio::test]
async fn test_create_invalid_strategy_data_fails() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let mut context = program_test.start_with_context().await;

    // EV strategy with min_bet=0 should fail validation
    let bad_ev_data = ev_strategy_data(100_000, 0, 50, 1_000_000_000);

    let ix = create_strat_deployer(
        authority.pubkey(),
        manager.pubkey(),
        deploy_authority.pubkey(),
        0, 0, 1_000_000_000,
        0, // EV strategy
        bad_ev_data,
    );

    let payer = context.payer.insecure_clone();
    let result = send_transaction(&mut context, &[ix], &[&payer, &authority]).await;
    assert!(result.is_err(), "Invalid strategy data must be rejected");
}
