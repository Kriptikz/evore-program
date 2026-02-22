mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autocheckpoint};
use evore::ore_api::miner_pda;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_checkpoint_test() -> (
    solana_program_test::ProgramTestContext,
    Keypair,  // deploy_authority
    Pubkey,   // manager pubkey
    Pubkey,   // managed_miner_auth
    Pubkey,   // ore_miner
    u8,       // managed_miner_auth_bump
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, mma_bump) = managed_miner_auth_pda(manager.pubkey(), auth_id);
    let ore_miner = miner_pda(mma_pda).0;

    // Add ORE accounts for checkpoint
    let board = setup_strat_deploy_test_accounts(&mut program_test, TEST_ROUND_ID, 1, 500);

    // Add ore miner that needs checkpointing (checkpoint_id < board.round_id)
    add_ore_miner_account(
        &mut program_test,
        mma_pda,
        [0u64; 25],
        0,     // rewards_sol
        0,     // rewards_ore
        TEST_ROUND_ID - 1, // checkpoint_id (behind current round)
        TEST_ROUND_ID,
    );

    // Fund the managed_miner_auth
    add_autodeploy_balance(&mut program_test, mma_pda, 10_000_000_000);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    // Fund deploy_authority and authority
    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &authority.pubkey(), 1_000_000_000,
    );
    let fund_ix2 = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &deploy_authority.pubkey(), 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix, fund_ix2], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    // Create strat deployer
    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, 1_000_000_000, 2, manual_strategy_data(),
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, ore_miner, mma_bump)
}

#[tokio::test]
async fn test_strat_checkpoint_succeeds() {
    let (mut context, deploy_authority, manager, mma_pda, ore_miner, mma_bump) =
        setup_checkpoint_test().await;
    let payer = context.payer.insecure_clone();

    let ix = mm_strat_autocheckpoint(
        deploy_authority.pubkey(),
        manager,
        0, // auth_id
        mma_bump,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "Strat autocheckpoint should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_strat_checkpoint_wrong_authority_fails() {
    let (mut context, _deploy_authority, manager, _mma_pda, _ore_miner, mma_bump) =
        setup_checkpoint_test().await;
    let payer = context.payer.insecure_clone();

    let wrong_signer = Keypair::new();
    let fund_ix = solana_sdk::system_instruction::transfer(
        &payer.pubkey(), &wrong_signer.pubkey(), 1_000_000_000,
    );
    send_transaction(&mut context, &[fund_ix], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = mm_strat_autocheckpoint(
        wrong_signer.pubkey(),
        manager,
        0,
        mma_bump,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &wrong_signer]).await;
    assert!(result.is_err(), "Wrong deploy_authority must be rejected");
}
