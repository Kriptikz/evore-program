mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autodeploy};
use evore::ore_api::miner_pda;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_ev_test(
    max_per_square: u64,
    min_bet: u64,
    slots_left: u64,
    ore_value: u64,
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

    let (mma_pda, _) = managed_miner_auth_pda(manager.pubkey(), auth_id);

    setup_strat_deploy_test_accounts(&mut program_test, 0, 1, 500);
    add_autodeploy_balance(&mut program_test, mma_pda, 50_000_000_000);

    let strategy_data = ev_strategy_data(max_per_square, min_bet, slots_left, ore_value);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &authority.pubkey(), 2_000_000_000,
    );
    let fund_ix2 = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000,
    );
    // Fund FEE_COLLECTOR to keep it rent-exempt after protocol fee transfers
    let fund_fc = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &evore::consts::FEE_COLLECTOR, 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix, fund_ix2, fund_fc], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, max_per_round,
        0, // EV
        strategy_data,
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, auth_id)
}

// ============================================================================
// EV strategy: waterfill deploy
// ============================================================================

#[tokio::test]
async fn test_ev_deploys_to_positive_ev_squares() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_ev_test(
            500_000_000,  // max 0.5 SOL per square
            1_000_000,    // min bet 0.001 SOL
            500,          // slots_left threshold
            2_000_000_000, // 2 SOL ore value
            0,            // no max per round
        ).await;
    let payer = context.payer.insecure_clone();

    // For EV: `amount` is bankroll
    let bankroll: u64 = 5_000_000_000; // 5 SOL bankroll

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        bankroll,
        0, // unused for EV
        0, // unused for EV
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "EV autodeploy should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_ev_small_bankroll_still_deploys() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_ev_test(
            100_000_000,
            1_000_000,   // 0.001 SOL min bet
            500,
            0,           // no ore value
            0,
        ).await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        50_000_000, // 0.05 SOL bankroll
        0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "EV with small bankroll should deploy to best squares: {:?}", result.err());
}
