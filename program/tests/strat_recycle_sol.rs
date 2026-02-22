mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, recycle_strat_sol};
use evore::ore_api::miner_pda;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_recycle_test(rewards_sol: u64) -> (
    solana_program_test::ProgramTestContext,
    Keypair,  // deploy_authority
    Pubkey,   // manager pubkey
    Pubkey,   // managed_miner_auth
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, _mma_bump) = managed_miner_auth_pda(manager.pubkey(), auth_id);

    // Add ore miner with claimable SOL
    add_ore_miner_account(
        &mut program_test,
        mma_pda,
        [0u64; 25],
        rewards_sol,
        0,
        TEST_ROUND_ID,
        TEST_ROUND_ID,
    );

    add_autodeploy_balance(&mut program_test, mma_pda, 5_000_000_000);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &authority.pubkey(), 1_000_000_000,
    );
    let fund_ix2 = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &deploy_authority.pubkey(), 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix, fund_ix2], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, 1_000_000_000, 2, manual_strategy_data(),
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda)
}

#[tokio::test]
async fn test_recycle_strat_sol_succeeds() {
    let (mut context, deploy_authority, manager, mma_pda) =
        setup_recycle_test(500_000_000).await; // 0.5 SOL claimable
    let payer = context.payer.insecure_clone();

    let ix = recycle_strat_sol(
        deploy_authority.pubkey(),
        manager,
        0, // auth_id
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "RecycleStratSol should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_recycle_strat_sol_nothing_to_recycle_ok() {
    let (mut context, deploy_authority, manager, _mma_pda) =
        setup_recycle_test(0).await; // 0 claimable
    let payer = context.payer.insecure_clone();

    let ix = recycle_strat_sol(
        deploy_authority.pubkey(),
        manager,
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "RecycleStratSol with 0 rewards should be a no-op success");
}

#[tokio::test]
async fn test_recycle_strat_sol_wrong_authority_fails() {
    let (mut context, _deploy_authority, manager, _mma_pda) =
        setup_recycle_test(500_000_000).await;
    let payer = context.payer.insecure_clone();

    let wrong_signer = Keypair::new();
    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &wrong_signer.pubkey(), 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = recycle_strat_sol(
        wrong_signer.pubkey(),
        manager,
        0,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &wrong_signer]).await;
    assert!(result.is_err(), "Wrong deploy_authority must be rejected");
}
