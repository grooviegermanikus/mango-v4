#![cfg(feature = "test-bpf")]

use fixed::types::I80F48;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transport::TransportError,
};

use mango_v4::state::*;
use program_test::*;

mod program_test;

#[tokio::test]
async fn test_bankrupt_tokens_socialize_loss() -> Result<(), TransportError> {
    let context = TestContext::new().await;
    let solana = &context.solana.clone();

    let admin = &Keypair::new();
    let owner = &context.users[0].key;
    let payer = &context.users[1].key;
    let mints = &context.mints[0..4];
    let payer_mint_accounts = &context.users[1].token_accounts[0..4];

    //
    // SETUP: Create a group and an account to fill the vaults
    //

    let mango_setup::GroupWithTokens { group, tokens, .. } = mango_setup::GroupWithTokensConfig {
        admin,
        payer,
        mints,
    }
    .create(solana)
    .await;
    let borrow_token1 = &tokens[0];
    let borrow_token2 = &tokens[1];
    let collateral_token1 = &tokens[2];
    let collateral_token2 = &tokens[3];

    // deposit some funds, to the vaults aren't empty
    let vault_account = send_tx(
        solana,
        AccountCreateInstruction {
            account_num: 2,
            token_count: 16,
            serum3_count: 8,
            perp_count: 8,
            perp_oo_count: 8,
            group,
            owner,
            payer,
        },
    )
    .await
    .unwrap()
    .account;
    let vault_amount = 100000;
    for &token_account in payer_mint_accounts {
        send_tx(
            solana,
            TokenDepositInstruction {
                amount: vault_amount,
                account: vault_account,
                token_account,
                token_authority: payer.clone(),
                bank_index: 1,
            },
        )
        .await
        .unwrap();
    }

    // also add a tiny amount to bank0 for borrow_token1, so we can test multi-bank socialized loss
    send_tx(
        solana,
        TokenDepositInstruction {
            amount: 10,
            account: vault_account,
            token_account: payer_mint_accounts[0],
            token_authority: payer.clone(),
            bank_index: 0,
        },
    )
    .await
    .unwrap();

    //
    // SETUP: Make an account with some collateral and some borrows
    //
    let account = send_tx(
        solana,
        AccountCreateInstruction {
            account_num: 0,
            token_count: 16,
            serum3_count: 8,
            perp_count: 8,
            perp_oo_count: 8,
            group,
            owner,
            payer,
        },
    )
    .await
    .unwrap()
    .account;

    let deposit1_amount = 1000;
    let deposit2_amount = 20;
    send_tx(
        solana,
        TokenDepositInstruction {
            amount: deposit1_amount,
            account,
            token_account: payer_mint_accounts[2],
            token_authority: payer.clone(),
            bank_index: 0,
        },
    )
    .await
    .unwrap();
    send_tx(
        solana,
        TokenDepositInstruction {
            amount: deposit2_amount,
            account,
            token_account: payer_mint_accounts[3],
            token_authority: payer.clone(),
            bank_index: 0,
        },
    )
    .await
    .unwrap();

    let borrow1_amount = 350;
    let borrow1_amount_bank0 = 10;
    let borrow1_amount_bank1 = borrow1_amount - borrow1_amount_bank0;
    let borrow2_amount = 50;
    send_tx(
        solana,
        TokenWithdrawInstruction {
            amount: borrow1_amount_bank1,
            allow_borrow: true,
            account,
            owner,
            token_account: payer_mint_accounts[0],
            bank_index: 1,
        },
    )
    .await
    .unwrap();
    send_tx(
        solana,
        TokenWithdrawInstruction {
            amount: borrow1_amount_bank0,
            allow_borrow: true,
            account,
            owner,
            token_account: payer_mint_accounts[0],
            bank_index: 0,
        },
    )
    .await
    .unwrap();
    send_tx(
        solana,
        TokenWithdrawInstruction {
            amount: borrow2_amount,
            allow_borrow: true,
            account,
            owner,
            token_account: payer_mint_accounts[1],
            bank_index: 1,
        },
    )
    .await
    .unwrap();

    //
    // SETUP: Change the oracle to make health go very negative
    //
    send_tx(
        solana,
        StubOracleSetInstruction {
            group,
            admin,
            mint: borrow_token1.mint.pubkey,
            payer,
            price: "20.0",
        },
    )
    .await
    .unwrap();

    //
    // SETUP: liquidate all the collateral against borrow1
    //

    // eat collateral1
    send_tx(
        solana,
        LiqTokenWithTokenInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            asset_token_index: collateral_token1.index,
            asset_bank_index: 1,
            liab_token_index: borrow_token1.index,
            liab_bank_index: 1,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    assert!(account_position_closed(solana, account, collateral_token1.bank).await);
    assert_eq!(
        account_position(solana, account, borrow_token1.bank).await,
        (-350.0f64 + (1000.0 / 20.0 / 1.04)).round() as i64
    );
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(!liqee.is_bankrupt());

    // eat collateral2, leaving the account bankrupt
    send_tx(
        solana,
        LiqTokenWithTokenInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            asset_token_index: collateral_token2.index,
            asset_bank_index: 1,
            liab_token_index: borrow_token1.index,
            liab_bank_index: 1,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    assert!(account_position_closed(solana, account, collateral_token2.bank).await);
    let borrow1_after_liq = -350.0f64 + (1000.0 / 20.0 / 1.04) + (20.0 / 20.0 / 1.04);
    assert_eq!(
        account_position(solana, account, borrow_token1.bank).await,
        borrow1_after_liq.round() as i64
    );
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(liqee.is_bankrupt());

    //
    // TEST: socialize loss on borrow1 and 2
    //

    let vault_before = account_position(solana, vault_account, borrow_token1.bank).await;
    send_tx(
        solana,
        LiqTokenBankruptcyInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            liab_token_index: borrow_token1.index,
            liab_mint_info: borrow_token1.mint_info,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        account_position(solana, vault_account, borrow_token1.bank).await,
        vault_before + (borrow1_after_liq.round() as i64)
    );
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(liqee.is_bankrupt());
    assert!(account_position_closed(solana, account, borrow_token1.bank).await);
    // both bank's borrows were completely wiped: no one else borrowed
    let borrow1_bank0: Bank = solana.get_account(borrow_token1.bank).await;
    let borrow1_bank1: Bank = solana.get_account(borrow_token1.bank).await;
    assert_eq!(borrow1_bank0.native_borrows(), 0);
    assert_eq!(borrow1_bank1.native_borrows(), 0);

    send_tx(
        solana,
        LiqTokenBankruptcyInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            liab_token_index: borrow_token2.index,
            liab_mint_info: borrow_token2.mint_info,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        account_position(solana, vault_account, borrow_token2.bank).await,
        (vault_amount - borrow2_amount) as i64
    );
    let liqee = get_mango_account(solana, account).await;
    assert!(!liqee.being_liquidated());
    assert!(!liqee.is_bankrupt());
    assert!(account_position_closed(solana, account, borrow_token2.bank).await);

    Ok(())
}

#[tokio::test]
async fn test_bankrupt_tokens_insurance_fund() -> Result<(), TransportError> {
    let context = TestContext::new().await;
    let solana = &context.solana.clone();

    let admin = &Keypair::new();
    let owner = &context.users[0].key;
    let payer = &context.users[1].key;
    let mints = &context.mints[0..4];
    let payer_mint_accounts = &context.users[1].token_accounts[0..4];

    //
    // SETUP: Create a group and an account to fill the vaults
    //

    let mango_setup::GroupWithTokens {
        group,
        tokens,
        insurance_vault,
    } = mango_setup::GroupWithTokensConfig {
        admin,
        payer,
        mints,
    }
    .create(solana)
    .await;
    let borrow_token1 = &tokens[0]; // USDC
    let borrow_token2 = &tokens[1];
    let collateral_token1 = &tokens[2];
    let collateral_token2 = &tokens[3];

    // fund the insurance vault
    {
        let mut tx = ClientTransaction::new(solana);
        tx.add_instruction_direct(
            spl_token::instruction::transfer(
                &spl_token::ID,
                &payer_mint_accounts[0],
                &insurance_vault,
                &payer.pubkey(),
                &[&payer.pubkey()],
                1051,
            )
            .unwrap(),
        );
        tx.add_signer(payer);
        tx.send().await.unwrap();
    }

    // deposit some funds, to the vaults aren't empty
    let vault_account = send_tx(
        solana,
        AccountCreateInstruction {
            account_num: 2,
            token_count: 16,
            serum3_count: 8,
            perp_count: 8,
            perp_oo_count: 8,
            group,
            owner,
            payer,
        },
    )
    .await
    .unwrap()
    .account;
    let vault_amount = 100000;
    for &token_account in payer_mint_accounts {
        send_tx(
            solana,
            TokenDepositInstruction {
                amount: vault_amount,
                account: vault_account,
                token_account,
                token_authority: payer.clone(),
                bank_index: 1,
            },
        )
        .await
        .unwrap();
    }

    // also add a tiny amount to bank0 for borrow_token1, so we can test multi-bank socialized loss
    send_tx(
        solana,
        TokenDepositInstruction {
            amount: 10,
            account: vault_account,
            token_account: payer_mint_accounts[0],
            token_authority: payer.clone(),
            bank_index: 0,
        },
    )
    .await
    .unwrap();

    //
    // SETUP: Make an account with some collateral and some borrows
    //
    let account = send_tx(
        solana,
        AccountCreateInstruction {
            account_num: 0,
            token_count: 16,
            serum3_count: 8,
            perp_count: 8,
            perp_oo_count: 8,
            group,
            owner,
            payer,
        },
    )
    .await
    .unwrap()
    .account;

    let deposit1_amount = 20;
    let deposit2_amount = 1000;
    send_tx(
        solana,
        TokenDepositInstruction {
            amount: deposit1_amount,
            account,
            token_account: payer_mint_accounts[2],
            token_authority: payer.clone(),
            bank_index: 0,
        },
    )
    .await
    .unwrap();
    send_tx(
        solana,
        TokenDepositInstruction {
            amount: deposit2_amount,
            account,
            token_account: payer_mint_accounts[3],
            token_authority: payer.clone(),
            bank_index: 0,
        },
    )
    .await
    .unwrap();

    let borrow1_amount = 50;
    let borrow1_amount_bank0 = 10;
    let borrow1_amount_bank1 = borrow1_amount - borrow1_amount_bank0;
    let borrow2_amount = 350;
    send_tx(
        solana,
        TokenWithdrawInstruction {
            amount: borrow1_amount_bank1,
            allow_borrow: true,
            account,
            owner,
            token_account: payer_mint_accounts[0],
            bank_index: 1,
        },
    )
    .await
    .unwrap();
    send_tx(
        solana,
        TokenWithdrawInstruction {
            amount: borrow1_amount_bank0,
            allow_borrow: true,
            account,
            owner,
            token_account: payer_mint_accounts[0],
            bank_index: 0,
        },
    )
    .await
    .unwrap();
    send_tx(
        solana,
        TokenWithdrawInstruction {
            amount: borrow2_amount,
            allow_borrow: true,
            account,
            owner,
            token_account: payer_mint_accounts[1],
            bank_index: 1,
        },
    )
    .await
    .unwrap();

    //
    // SETUP: Change the oracle to make health go very negative
    //
    send_tx(
        solana,
        StubOracleSetInstruction {
            group,
            admin,
            mint: borrow_token2.mint.pubkey,
            payer,
            price: "20.0",
        },
    )
    .await
    .unwrap();

    //
    // SETUP: liquidate all the collateral against borrow2
    //

    // eat collateral1
    send_tx(
        solana,
        LiqTokenWithTokenInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            asset_token_index: collateral_token1.index,
            asset_bank_index: 1,
            liab_token_index: borrow_token2.index,
            liab_bank_index: 1,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    assert!(account_position_closed(solana, account, collateral_token1.bank).await);
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(!liqee.is_bankrupt());

    // eat collateral2, leaving the account bankrupt
    send_tx(
        solana,
        LiqTokenWithTokenInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            asset_token_index: collateral_token2.index,
            asset_bank_index: 1,
            liab_token_index: borrow_token2.index,
            liab_bank_index: 1,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    assert!(account_position_closed(solana, account, collateral_token2.bank).await,);
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(liqee.is_bankrupt());

    //
    // TEST: use the insurance fund to liquidate borrow1 and borrow2
    //

    // bankruptcy of an USDC liability: just transfers funds from insurance vault to liqee,
    // the liqor is uninvolved
    let insurance_vault_before = solana.token_account_balance(insurance_vault).await;
    let liqor_before = account_position(solana, vault_account, borrow_token1.bank).await;
    send_tx(
        solana,
        LiqTokenBankruptcyInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            liab_token_index: borrow_token1.index,
            liab_mint_info: borrow_token1.mint_info,
            max_liab_transfer: I80F48::from_num(100000.0),
        },
    )
    .await
    .unwrap();
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(liqee.is_bankrupt());
    assert!(account_position_closed(solana, account, borrow_token1.bank).await);
    assert_eq!(
        solana.token_account_balance(insurance_vault).await,
        // the loan origination fees push the borrow above 50.0 and cause this rounding
        insurance_vault_before - borrow1_amount - 1
    );
    assert_eq!(
        account_position(solana, vault_account, borrow_token1.bank).await,
        liqor_before
    );

    // bankruptcy of a non-USDC liability: USDC to liqor, liability to liqee
    // liquidating only a partial amount
    let liab_before = account_position_f64(solana, account, borrow_token2.bank).await;
    let insurance_vault_before = solana.token_account_balance(insurance_vault).await;
    let liqor_before = account_position(solana, vault_account, borrow_token1.bank).await;
    let liab_transfer: f64 = 500.0 / 20.0;
    send_tx(
        solana,
        LiqTokenBankruptcyInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            liab_token_index: borrow_token2.index,
            liab_mint_info: borrow_token2.mint_info,
            max_liab_transfer: I80F48::from_num(liab_transfer),
        },
    )
    .await
    .unwrap();
    let liqee = get_mango_account(solana, account).await;
    assert!(liqee.being_liquidated());
    assert!(liqee.is_bankrupt());
    assert!(account_position_closed(solana, account, borrow_token1.bank).await);
    assert_eq!(
        account_position(solana, account, borrow_token2.bank).await,
        (liab_before + liab_transfer) as i64
    );
    let usdc_amount = (liab_transfer * 20.0 * 1.02).ceil() as u64;
    assert_eq!(
        solana.token_account_balance(insurance_vault).await,
        insurance_vault_before - usdc_amount
    );
    assert_eq!(
        account_position(solana, vault_account, borrow_token1.bank).await,
        liqor_before + usdc_amount as i64
    );

    // bankruptcy of a non-USDC liability: USDC to liqor, liability to liqee
    // liquidating fully and then doing socialized loss because the insurance fund is exhausted
    let insurance_vault_before = solana.token_account_balance(insurance_vault).await;
    let liqor_before = account_position(solana, vault_account, borrow_token1.bank).await;
    send_tx(
        solana,
        LiqTokenBankruptcyInstruction {
            liqee: account,
            liqor: vault_account,
            liqor_owner: owner,
            liab_token_index: borrow_token2.index,
            liab_mint_info: borrow_token2.mint_info,
            max_liab_transfer: I80F48::from_num(1000000.0),
        },
    )
    .await
    .unwrap();
    let liqee = get_mango_account(solana, account).await;
    assert!(!liqee.being_liquidated());
    assert!(!liqee.is_bankrupt());
    assert!(account_position_closed(solana, account, borrow_token1.bank).await);
    assert!(account_position_closed(solana, account, borrow_token2.bank).await);
    assert_eq!(solana.token_account_balance(insurance_vault).await, 0);
    assert_eq!(
        account_position(solana, vault_account, borrow_token1.bank).await,
        liqor_before + insurance_vault_before as i64
    );

    Ok(())
}