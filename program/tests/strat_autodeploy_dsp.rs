mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autodeploy};
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_dsp_test(
    percentage: u64,
    squares_mask: u64,
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

    let strategy_data = dsp_strategy_data(percentage, squares_mask, motherlode_min, motherlode_max);

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
        4, // DynamicSplitPercentage
        strategy_data,
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, auth_id)
}

#[tokio::test]
async fn test_dsp_deploys_to_masked_squares() {
    // DSP with 20% on squares 0-4 (mask = 0b11111 = 31)
    let (mut context, deploy_authority, manager, _, auth_id) =
        setup_dsp_test(2000, 0b11111, 0, 0).await;
    let payer = context.payer.insecure_clone();

    let bankroll: u64 = 3_000_000_000;

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(), manager, auth_id,
        bankroll, 0, 0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "DSP autodeploy should succeed: {:?}", result.err());
}
