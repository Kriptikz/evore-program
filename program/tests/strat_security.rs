mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{
    create_strat_deployer, update_strat_deployer, mm_strat_autodeploy,
    mm_strat_full_autodeploy, mm_strat_autocheckpoint, recycle_strat_sol,
};
use evore::ore_api::miner_pda;
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

// ============================================================================
// Cross-instruction helper: set up a fully initialized StrategyDeployer
// ============================================================================

async fn setup_security_env() -> (
    solana_program_test::ProgramTestContext,
    Keypair,  // authority (manager authority)
    Keypair,  // deploy_authority
    Pubkey,   // manager pubkey
    Pubkey,   // strat_deployer_pda
    Pubkey,   // managed_miner_auth
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, _) = managed_miner_auth_pda(manager.pubkey(), auth_id);
    let (sd_pda, _) = strategy_deployer_pda(manager.pubkey());

    setup_strat_deploy_test_accounts(&mut program_test, 0, 1, 500);
    add_ore_miner_account(&mut program_test, mma_pda, [0u64; 25], 0, 0, 0, 0);
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
        0, 0, 0,
        2, // Manual
        manual_strategy_data(),
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, authority, deploy_authority, manager.pubkey(), sd_pda, mma_pda)
}

// ============================================================================
// Autodeploy: wrong deploy_authority rejected
// ============================================================================

#[tokio::test]
async fn test_autodeploy_rejects_non_deploy_authority() {
    let (mut context, _authority, _deploy_authority, manager, _sd_pda, _mma_pda) =
        setup_security_env().await;
    let payer = context.payer.insecure_clone();

    let attacker = Keypair::new();
    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000_000);
    send_transaction(&mut context, &[fund], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = mm_strat_autodeploy(attacker.pubkey(), manager, 0, 100_000_000, 1, 0);
    let result = send_transaction(&mut context, &[ix], &[&payer, &attacker]).await;
    assert!(result.is_err(), "Non-deploy_authority must be rejected for autodeploy");
}

// ============================================================================
// Full autodeploy: wrong deploy_authority rejected
// ============================================================================

#[tokio::test]
async fn test_full_autodeploy_rejects_non_deploy_authority() {
    let (mut context, _authority, _deploy_authority, manager, _sd_pda, _mma_pda) =
        setup_security_env().await;
    let payer = context.payer.insecure_clone();

    let attacker = Keypair::new();
    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000_000);
    send_transaction(&mut context, &[fund], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = mm_strat_full_autodeploy(attacker.pubkey(), manager, 0, 100_000_000, 1, 0);
    let result = send_transaction(&mut context, &[ix], &[&payer, &attacker]).await;
    assert!(result.is_err(), "Non-deploy_authority must be rejected for full autodeploy");
}

// ============================================================================
// Update: neither manager authority nor deploy_authority
// ============================================================================

#[tokio::test]
async fn test_update_rejects_random_signer() {
    let (mut context, _authority, deploy_authority, manager, _sd_pda, _mma_pda) =
        setup_security_env().await;
    let payer = context.payer.insecure_clone();

    let attacker = Keypair::new();
    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000_000);
    send_transaction(&mut context, &[fund], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = update_strat_deployer(
        attacker.pubkey(), manager, deploy_authority.pubkey(),
        0, 0, 0, 0, 0, 2, manual_strategy_data(),
    );
    let result = send_transaction(&mut context, &[ix], &[&payer, &attacker]).await;
    assert!(result.is_err(), "Random signer must be rejected for update");
}

// ============================================================================
// Checkpoint: wrong deploy_authority rejected
// ============================================================================

#[tokio::test]
async fn test_checkpoint_rejects_non_deploy_authority() {
    let (mut context, _authority, _deploy_authority, manager, _sd_pda, mma_pda) =
        setup_security_env().await;
    let payer = context.payer.insecure_clone();

    let attacker = Keypair::new();
    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000_000);
    send_transaction(&mut context, &[fund], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let (_, bump) = managed_miner_auth_pda(manager, 0);
    let ix = mm_strat_autocheckpoint(attacker.pubkey(), manager, 0, bump);
    let result = send_transaction(&mut context, &[ix], &[&payer, &attacker]).await;
    assert!(result.is_err(), "Non-deploy_authority must be rejected for checkpoint");
}

// ============================================================================
// RecycleStratSol: wrong deploy_authority rejected
// ============================================================================

#[tokio::test]
async fn test_recycle_rejects_non_deploy_authority() {
    let (mut context, _authority, _deploy_authority, manager, _sd_pda, _mma_pda) =
        setup_security_env().await;
    let payer = context.payer.insecure_clone();

    let attacker = Keypair::new();
    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &attacker.pubkey(), 1_000_000_000);
    send_transaction(&mut context, &[fund], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = recycle_strat_sol(attacker.pubkey(), manager, 0);
    let result = send_transaction(&mut context, &[ix], &[&payer, &attacker]).await;
    assert!(result.is_err(), "Non-deploy_authority must be rejected for recycle");
}

// ============================================================================
// Fee protection: bps_fee > expected_bps_fee rejected
// ============================================================================

#[tokio::test]
async fn test_autodeploy_rejects_fee_exceeding_expected() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, _) = managed_miner_auth_pda(manager.pubkey(), auth_id);
    setup_strat_deploy_test_accounts(&mut program_test, 0, 1, 500);
    add_autodeploy_balance(&mut program_test, mma_pda, 50_000_000_000);

    // Pre-build a strat deployer with bps_fee > expected_bps_fee
    let (sd_addr, _) = strategy_deployer_pda(manager.pubkey());
    add_strat_deployer_account(
        &mut program_test,
        sd_addr,
        manager.pubkey(),
        deploy_authority.pubkey(),
        200,    // bps_fee = 200 (actual fee)
        0,      // flat_fee
        100,    // expected_bps_fee = 100 (max the manager accepts)
        0,      // expected_flat_fee
        0,      // max_per_round
        2,      // Manual
        manual_strategy_data(),
    );

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000);
    let fund_fc = solana_sdk::system_instruction::transfer(&payer.pubkey(), &evore::consts::FEE_COLLECTOR, 1_000_000_000);
    send_transaction(&mut context, &[fund, fund_fc], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = mm_strat_autodeploy(deploy_authority.pubkey(), manager.pubkey(), auth_id, 100_000_000, 1, 0);
    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_err(), "Deploy with bps_fee > expected_bps_fee must be rejected");
}

// ============================================================================
// Existing Deployer and StrategyDeployer are independent (backward compat)
// ============================================================================

#[tokio::test]
async fn test_strat_deployer_does_not_affect_regular_deployer() {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund = solana_sdk::system_instruction::transfer(&payer.pubkey(), &authority.pubkey(), 2_000_000_000);
    send_transaction(&mut context, &[fund], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    // Create a strat deployer
    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, 0, 2, manual_strategy_data(),
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();

    // Verify the regular deployer PDA is NOT initialized
    let (deployer_pda, _) = evore::state::deployer_pda(manager.pubkey());
    let deployer_acct = context.banks_client.get_account(deployer_pda).await.unwrap();
    assert!(deployer_acct.is_none(), "StrategyDeployer creation must not create a regular Deployer");
}
