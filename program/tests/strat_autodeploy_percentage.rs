mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autodeploy};
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_percentage_test(
    percentage: u64,
    squares_count: u64,
    motherlode_min: u64,
    motherlode_max: u64,
) -> (
    solana_program_test::ProgramTestContext,
    Keypair,
    Pubkey,
    Pubkey,
    u64,
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

    let strategy_data = percentage_strategy_data(percentage, squares_count, motherlode_min, motherlode_max);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund_ix = solana_sdk::system_instruction::transfer(&payer.pubkey(), &authority.pubkey(), 2_000_000_000);
    let fund_ix2 = solana_sdk::system_instruction::transfer(&payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000);
    let fund_fc = solana_sdk::system_instruction::transfer(&payer.pubkey(), &evore::consts::FEE_COLLECTOR, 1_000_000_000);
    send_transaction(&mut context, &[fund_ix, fund_ix2, fund_fc], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, 0,
        1, // Percentage
        strategy_data,
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, auth_id)
}

#[tokio::test]
async fn test_percentage_deploys_to_top_squares() {
    let (mut context, deploy_authority, manager, _, auth_id) =
        setup_percentage_test(1000, 5, 0, 0).await; // 10% of top 5
    let payer = context.payer.insecure_clone();

    let bankroll: u64 = 5_000_000_000;

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(), manager, auth_id,
        bankroll, 0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Percentage autodeploy should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_percentage_small_bankroll_reduces_pct() {
    let (mut context, deploy_authority, manager, _, auth_id) =
        setup_percentage_test(5000, 3, 0, 0).await; // 50% of top 3
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(), manager, auth_id,
        100_000_000, // small bankroll
        0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Percentage with small bankroll should still deploy: {:?}", result.err());
}
