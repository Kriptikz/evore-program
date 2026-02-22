mod strat_common;

use strat_common::*;

use evore::consts::FEE_COLLECTOR;
use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autodeploy};
use evore::ore_api::miner_pda;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_manual_autodeploy_test(
    bps_fee: u64,
    flat_fee: u64,
    max_per_round: u64,
) -> (
    solana_program_test::ProgramTestContext,
    Keypair,  // deploy_authority
    Pubkey,   // manager pubkey
    Pubkey,   // managed_miner_auth
    u64,      // auth_id
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, _mma_bump) = managed_miner_auth_pda(manager.pubkey(), auth_id);

    // Setup ORE accounts
    setup_strat_deploy_test_accounts(&mut program_test, 0, 1, 500);

    // Fund the managed_miner_auth generously
    add_autodeploy_balance(&mut program_test, mma_pda, 50_000_000_000);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    // Fund signers and fee collector
    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &authority.pubkey(), 2_000_000_000,
    );
    let fund_ix2 = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000,
    );
    let fund_fee_collector = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &FEE_COLLECTOR, 1_000_000,
    );
    send_transaction(&mut context, &[fund_ix, fund_ix2, fund_fee_collector], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    // Create strat deployer with Manual strategy
    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        bps_fee, flat_fee, max_per_round,
        2, // Manual
        manual_strategy_data(),
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, auth_id)
}

// ============================================================================
// Manual strategy: basic deploy
// ============================================================================

#[tokio::test]
async fn test_manual_single_square() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_manual_autodeploy_test(0, 0, 0).await;
    let payer = context.payer.insecure_clone();

    // Deploy 0.1 SOL to square 0 only
    let squares_mask: u32 = 1; // bit 0
    let amount: u64 = 100_000_000; // 0.1 SOL

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        amount,
        squares_mask,
        0, // extra (unused for manual)
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Manual autodeploy single square should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_manual_multiple_squares() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_manual_autodeploy_test(0, 0, 0).await;
    let payer = context.payer.insecure_clone();

    // Deploy 0.05 SOL to squares 0-4
    let squares_mask: u32 = 0b11111; // bits 0-4
    let amount: u64 = 50_000_000; // 0.05 SOL per square

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        amount,
        squares_mask,
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Manual autodeploy multiple squares should succeed: {:?}", result.err());
}

// ============================================================================
// Fee calculation
// ============================================================================

#[tokio::test]
async fn test_manual_fee_calculation_bps_and_flat() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_manual_autodeploy_test(1000, 10_000, 0).await; // 10% bps + 10k lamport flat
    let payer = context.payer.insecure_clone();

    let balance_before = context.banks_client
        .get_balance(deploy_authority.pubkey()).await.unwrap();

    let squares_mask: u32 = 1;
    let amount: u64 = 100_000_000; // 0.1 SOL

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        amount,
        squares_mask,
        0,
    );

    send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await.unwrap();

    let balance_after = context.banks_client
        .get_balance(deploy_authority.pubkey()).await.unwrap();

    // deployer_fee = (100_000_000 * 1000 / 10000) + 10_000 = 10_000_000 + 10_000 = 10_010_000
    // Balance should increase by deployer_fee (minus tx fee for the deploy_authority signer)
    assert!(
        balance_after > balance_before,
        "Deploy authority should receive fee. Before: {}, After: {}",
        balance_before, balance_after,
    );
}

// ============================================================================
// Zero amount fails
// ============================================================================

#[tokio::test]
async fn test_manual_zero_amount_fails() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_manual_autodeploy_test(0, 0, 0).await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        0, // zero amount
        1, // square 0
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_err(), "Zero amount deploy must fail");
}

// ============================================================================
// Max per round enforced
// ============================================================================

#[tokio::test]
async fn test_manual_max_per_round_enforced() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_manual_autodeploy_test(0, 0, 50_000_000).await; // 0.05 SOL max per round
    let payer = context.payer.insecure_clone();

    // Try to deploy 0.1 SOL (exceeds 0.05 max)
    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        100_000_000, // 0.1 SOL
        1,
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_err(), "Deploy exceeding max_per_round must fail");
}
