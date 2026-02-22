mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_full_autodeploy};
use evore::ore_api::{miner_pda, board_pda, round_pda};
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

/// Sets up a full autodeploy scenario: manager + strat deployer + ore miner that needs checkpoint
async fn setup_full_test(
    strategy_type: u8,
    strategy_data: [u8; 64],
    bps_fee: u64,
    needs_checkpoint: bool,
    rewards_sol: u64,
) -> (
    solana_program_test::ProgramTestContext,
    Keypair,  // deploy_authority
    Pubkey,   // manager pubkey
    u64,      // auth_id
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, _) = managed_miner_auth_pda(manager.pubkey(), auth_id);

    // Round 0 for deploy
    setup_strat_deploy_test_accounts(&mut program_test, 0, 1, 500);

    // Add ore miner - checkpoint_id behind if needs_checkpoint
    let checkpoint_id = if needs_checkpoint { 0 } else { 0 };
    add_ore_miner_account(
        &mut program_test,
        mma_pda,
        [0u64; 25],
        rewards_sol,
        0,
        checkpoint_id,
        0, // round_id = 0
    );

    add_autodeploy_balance(&mut program_test, mma_pda, 50_000_000_000);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund_ix = solana_sdk::system_instruction::transfer(&payer.pubkey(), &authority.pubkey(), 2_000_000_000);
    let fund_ix2 = solana_sdk::system_instruction::transfer(&payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000);
    let fund_fc = solana_sdk::system_instruction::transfer(&payer.pubkey(), &evore::consts::FEE_COLLECTOR, 1_000_000_000);
    send_transaction(&mut context, &[fund_ix, fund_ix2, fund_fc], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        bps_fee, 0, 0,
        strategy_type,
        strategy_data,
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), auth_id)
}

// ============================================================================
// Full autodeploy with Manual strategy (checkpoint + recycle + deploy)
// ============================================================================

#[tokio::test]
async fn test_full_manual_deploys() {
    let (mut context, deploy_authority, manager, auth_id) =
        setup_full_test(2, manual_strategy_data(), 0, false, 0).await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_full_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        100_000_000, // 0.1 SOL per square
        0b111,       // squares 0-2
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Full autodeploy Manual should succeed: {:?}", result.err());
}

// ============================================================================
// Full autodeploy with EV strategy
// ============================================================================

#[tokio::test]
async fn test_full_ev_deploys() {
    let strategy_data = ev_strategy_data(500_000_000, 1_000_000, 500, 2_000_000_000);
    let (mut context, deploy_authority, manager, auth_id) =
        setup_full_test(0, strategy_data, 0, false, 0).await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_full_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        5_000_000_000, // bankroll
        0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Full autodeploy EV should succeed: {:?}", result.err());
}

// ============================================================================
// Full autodeploy with Split strategy
// ============================================================================

#[tokio::test]
async fn test_full_split_deploys() {
    let strategy_data = split_strategy_data(0, 0);
    let (mut context, deploy_authority, manager, auth_id) =
        setup_full_test(3, strategy_data, 0, false, 0).await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_full_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        2_500_000_000, // bankroll
        0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Full autodeploy Split should succeed: {:?}", result.err());
}

// ============================================================================
// Full autodeploy with recycle (SOL rewards)
// ============================================================================

#[tokio::test]
async fn test_full_recycles_sol_before_deploy() {
    let (mut context, deploy_authority, manager, auth_id) =
        setup_full_test(2, manual_strategy_data(), 0, false, 500_000_000).await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_full_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        100_000_000,
        1, // square 0
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Full autodeploy with recycle should succeed: {:?}", result.err());
}
