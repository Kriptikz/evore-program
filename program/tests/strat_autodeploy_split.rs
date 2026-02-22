mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autodeploy};
use evore::ore_api::miner_pda;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_split_test(
    max_per_round: u64,
    motherlode_min: u64,
    motherlode_max: u64,
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

    let strategy_data = split_strategy_data(motherlode_min, motherlode_max);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &authority.pubkey(), 2_000_000_000,
    );
    let fund_ix2 = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000,
    );
    let fund_fee_collector = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &evore::consts::FEE_COLLECTOR, 1_000_000,
    );
    send_transaction(&mut context, &[fund_ix, fund_ix2, fund_fee_collector], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, max_per_round,
        3, // Split
        strategy_data,
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, auth_id)
}

// ============================================================================
// Split strategy: deploy equal to all 25 squares
// ============================================================================

#[tokio::test]
async fn test_split_deploys_to_all_squares() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_split_test(0, 0, 0).await;
    let payer = context.payer.insecure_clone();

    // For split: `amount` is bankroll (total to spread), squares_mask is unused
    // The processor computes bankroll/25 per square and deploys to all 25
    let bankroll: u64 = 2_500_000_000; // 2.5 SOL -> 0.1 SOL per square

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        bankroll,
        0, // squares_mask ignored for split
        0, // extra unused
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Split autodeploy should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_split_small_bankroll_rounds_down() {
    let (mut context, deploy_authority, manager, mma_pda, auth_id) =
        setup_split_test(0, 0, 0).await;
    let payer = context.payer.insecure_clone();

    // 24 lamports / 25 = 0 per square -> should fail (NoDeployments)
    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(),
        manager,
        auth_id,
        24, // too small to divide by 25
        0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_err(), "Split with <25 lamports bankroll should fail");
}
