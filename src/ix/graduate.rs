//! `graduate` and `lock_liquidity` (programs/launch_curve/src/instructions/graduate.rs).

use crate::checks::*;
use crate::consts::{dex, *};
use crate::cpi::{self, Ev, Pda};
use crate::err::*;
use pinocchio::{
    account_info::AccountInfo,
    log::sol_log,
    sysvars::{clock::Clock, Sysvar},
};

/// Moves a complete curve into its Raydium CPMM pool at exactly the curve's last price. Permissionless.
pub fn graduate(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: Graduate");
    let mut c = Cursor::new(accounts);
    let payer = c.signer()?;
    let (config, cfg) = c.config()?;
    let (curve, crv) = c.curve()?;
    let mint = c.mint()?;
    let (vault, vault_t) = c.token()?;
    let sol_vault = c.any()?;
    let vault_wsol = c.any()?;
    let vault_token = c.any()?;
    let wsol_mint = c.mint()?;
    let cpmm_program = c.any()?;
    let amm_config = c.any()?;
    let cpmm_authority = c.any()?;
    let pool_state = c.any()?;
    let lp_mint = c.any()?;
    let vault_lp = c.any()?;
    let pool_vault_0 = c.any()?;
    let pool_vault_1 = c.any()?;
    let create_pool_fee = c.any()?;
    let observation = c.any()?;
    let token_program = c.program(&TOKEN_PROGRAM)?;
    let ata_program = c.program(&ATA_PROGRAM)?;
    let system_program = c.program(&SYSTEM_PROGRAM)?;
    let rent_acc = c.any()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    mutable(payer)?;
    config_seeds(config)?;
    mutable(config)?;
    seeds_with_bump(curve, CURVE_SEED, mint.key(), crv.bump())?;
    mutable(curve)?;
    same(crv.mint(), mint.key(), CONSTRAINT_HAS_ONE)?;
    mutable(mint)?;
    associated(vault, &vault_t, curve.key(), mint.key())?;
    mutable(vault)?;
    seeds_with_bump(sol_vault, SOL_VAULT_SEED, mint.key(), crv.vault_bump())?;
    mutable(sol_vault)?;
    mutable(vault_wsol)?;
    same(vault_wsol.key(), &ata(sol_vault.key(), &WSOL_MINT), ACCOUNT_MISMATCH)?;
    mutable(vault_token)?;
    same(vault_token.key(), &ata(sol_vault.key(), mint.key()), ACCOUNT_MISMATCH)?;
    same(wsol_mint.key(), &WSOL_MINT, CONSTRAINT_ADDRESS)?;
    same(cpmm_program.key(), &dex::CPMM_PROGRAM, CONSTRAINT_ADDRESS)?;
    same(amm_config.key(), &dex::CPMM_AMM_CONFIG, CONSTRAINT_ADDRESS)?;
    same(cpmm_authority.key(), &dex::CPMM_AUTHORITY, CONSTRAINT_ADDRESS)?;
    let pool_bump = seeds_find(pool_state, POOL_SEED, mint.key())?;
    mutable(pool_state)?;
    same(pool_state.key(), crv.pool(), ACCOUNT_MISMATCH)?;
    mutable(lp_mint)?;
    mutable(vault_lp)?;
    same(vault_lp.key(), &ata(sol_vault.key(), lp_mint.key()), ACCOUNT_MISMATCH)?;
    mutable(pool_vault_0)?;
    mutable(pool_vault_1)?;
    mutable(create_pool_fee)?;
    same(create_pool_fee.key(), &dex::CPMM_CREATE_POOL_FEE, CONSTRAINT_ADDRESS)?;
    mutable(observation)?;
    rent_sysvar(rent_acc)?;
    event_authority(ea)?;

    // ---- handler
    let now = Clock::get()?.unix_timestamp;
    if crv.graduated() {
        return Err(e(ALREADY_GRADUATED));
    }
    if crv.sold() != CURVE_TOKENS {
        return Err(e(CURVE_NOT_COMPLETE));
    }
    let target = cfg.target();
    let reserve = cfg.migration_reserve();
    let raised = crv.raised(); // == target
    let sol_to_pool = raised.checked_sub(reserve).ok_or(e(MATH_OVERFLOW))?;
    // pool price = sol_to_pool / tokens_to_pool = target / POOL_TOKENS: the curve's last price, no jump
    let tokens_to_pool = (POOL_TOKENS as u128 * sol_to_pool as u128 / target as u128) as u64;
    let vault_tokens = vault_t.amount(); // 200M plus anything sent to the vault
    let tokens_burned = vault_tokens - tokens_to_pool;

    // state first
    crv.set_graduated();
    crv.set_raised(0);
    cfg.set_total_reserves(cfg.total_reserves() - raised);

    let mint_key = mint.key();
    let cb = [crv.bump()];
    let curve_seeds = Pda::new(CURVE_SEED, mint_key, &cb);
    let vb = [crv.vault_bump()];
    let vault_seeds = Pda::new(SOL_VAULT_SEED, mint_key, &vb);
    let pb = [pool_bump];
    let pool_seeds = Pda::new(POOL_SEED, mint_key, &pb);

    // the SOL vault holds rent + raised; it becomes the pool creator
    if sol_vault.lamports() < raised {
        return Err(e(MATH_OVERFLOW));
    }
    cpi::create_ata(true, sol_vault, vault_wsol, sol_vault, wsol_mint, system_program, token_program, &[vault_seeds.signer()])?;
    cpi::create_ata(true, sol_vault, vault_token, sol_vault, mint, system_program, token_program, &[vault_seeds.signer()])?;

    // wrap the pool's SOL
    cpi::transfer(sol_vault, vault_wsol, sol_to_pool, &[vault_seeds.signer()])?;
    cpi::sync_native(vault_wsol)?;

    // the pool's tokens, and burn the rest of the vault
    cpi::token_transfer(vault, vault_token, curve, tokens_to_pool, &[curve_seeds.signer()])?;
    if tokens_burned > 0 {
        cpi::burn(vault, mint, curve, tokens_burned, &[curve_seeds.signer()])?;
    }

    // Raydium orders the pool's mints by byte value
    let wsol_first = WSOL_MINT < *mint_key;
    let (m0, m1, c0, c1, amt0, amt1) = if wsol_first {
        (wsol_mint, mint, vault_wsol, vault_token, sol_to_pool, tokens_to_pool)
    } else {
        (mint, wsol_mint, vault_token, vault_wsol, tokens_to_pool, sol_to_pool)
    };
    cpi::cpmm_initialize(
        cpmm_program,
        [
            sol_vault,
            amm_config,
            cpmm_authority,
            pool_state,
            m0,
            m1,
            lp_mint,
            c0,
            c1,
            vault_lp,
            pool_vault_0,
            pool_vault_1,
            create_pool_fee,
            observation,
            token_program,
            ata_program,
            system_program,
            rent_acc,
        ],
        amt0,
        amt1,
        &[vault_seeds.signer(), pool_seeds.signer()],
    )?;

    let lp_amount = token_layout(vault_lp)?.amount();
    crv.set_lp_mint(lp_mint.key());
    crv.set_lp_amount(lp_amount);

    Ev::new(&ev::GRADUATED)
        .put(mint_key)
        .put(pool_state.key())
        .put(lp_mint.key())
        .u64(sol_to_pool)
        .u64(tokens_to_pool)
        .u64(tokens_burned)
        .u64(lp_amount)
        .u64(reserve)
        .i64(now)
        .emit(ea)
}

/// Locks all the curve's LP in Raydium's LP lock for good and sweeps the unused migration reserve to the
/// treasury. Permissionless.
pub fn lock_liquidity(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: LockLiquidity");
    let mut c = Cursor::new(accounts);
    let payer = c.signer()?;
    let (config, cfg) = c.config()?;
    let treasury = c.any()?;
    let (curve, crv) = c.curve()?;
    let sol_vault = c.any()?;
    let lock_program = c.any()?;
    let lock_authority = c.any()?;
    let fee_nft_mint = c.any()?;
    let fee_nft_account = c.any()?;
    let pool_state = c.any()?;
    let locked_liquidity = c.any()?;
    let lp_mint = c.any()?;
    let vault_lp = c.any()?;
    let locked_lp_vault = c.any()?;
    let pool_vault_0 = c.any()?;
    let pool_vault_1 = c.any()?;
    let fee_nft_metadata = c.any()?;
    let rent_acc = c.any()?;
    let system_program = c.program(&SYSTEM_PROGRAM)?;
    let token_program = c.program(&TOKEN_PROGRAM)?;
    let ata_program = c.program(&ATA_PROGRAM)?;
    let metadata_program = c.any()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    let mint_key = crv.mint();
    mutable(payer)?;
    config_seeds(config)?;
    same(cfg.treasury(), treasury.key(), CONSTRAINT_HAS_ONE)?;
    mutable(treasury)?;
    seeds_with_bump(curve, CURVE_SEED, mint_key, crv.bump())?;
    mutable(curve)?;
    seeds_with_bump(sol_vault, SOL_VAULT_SEED, mint_key, crv.vault_bump())?;
    mutable(sol_vault)?;
    same(lock_program.key(), &dex::LOCK_PROGRAM, CONSTRAINT_ADDRESS)?;
    same(lock_authority.key(), &dex::LOCK_AUTHORITY, CONSTRAINT_ADDRESS)?;
    let nft_bump = seeds_find(fee_nft_mint, FEE_NFT_SEED, mint_key)?;
    mutable(fee_nft_mint)?;
    mutable(fee_nft_account)?;
    same(fee_nft_account.key(), &ata(sol_vault.key(), fee_nft_mint.key()), ACCOUNT_MISMATCH)?;
    same(pool_state.key(), crv.pool(), ACCOUNT_MISMATCH)?;
    mutable(locked_liquidity)?;
    same(lp_mint.key(), crv.lp_mint(), ACCOUNT_MISMATCH)?;
    mutable(vault_lp)?;
    same(vault_lp.key(), &ata(sol_vault.key(), lp_mint.key()), ACCOUNT_MISMATCH)?;
    mutable(locked_lp_vault)?;
    mutable(pool_vault_0)?;
    mutable(pool_vault_1)?;
    mutable(fee_nft_metadata)?;
    rent_sysvar(rent_acc)?;
    same(metadata_program.key(), &METADATA_PROGRAM, CONSTRAINT_ADDRESS)?;
    event_authority(ea)?;

    // ---- handler
    if !crv.graduated() {
        return Err(e(NOT_GRADUATED));
    }
    if crv.liquidity_locked() {
        return Err(e(ALREADY_LOCKED));
    }
    let vb = [crv.vault_bump()];
    let vault_seeds = Pda::new(SOL_VAULT_SEED, mint_key, &vb);
    let nb = [nft_bump];
    let nft_seeds = Pda::new(FEE_NFT_SEED, mint_key, &nb);

    let lp_amount = token_layout(vault_lp)?.amount();
    if lp_amount == 0 {
        return Err(e(ZERO_AMOUNT));
    }
    cpi::lock_cp_liquidity(
        lock_program,
        [
            lock_authority,
            sol_vault, // payer
            sol_vault, // liquidity_owner
            sol_vault, // fee_nft_owner
            fee_nft_mint,
            fee_nft_account,
            pool_state,
            locked_liquidity,
            lp_mint,
            vault_lp,
            locked_lp_vault,
            pool_vault_0,
            pool_vault_1,
            fee_nft_metadata,
            rent_acc,
            system_program,
            token_program,
            ata_program,
            metadata_program,
        ],
        lp_amount,
        &[vault_seeds.signer(), nft_seeds.signer()],
    )?;
    if token_layout(vault_lp)?.amount() != 0 {
        return Err(e(ACCOUNT_MISMATCH));
    }

    // the unused migration reserve goes to the treasury; the SOL vault keeps nothing
    let leftover = sol_vault.lamports();
    if leftover > 0 {
        cpi::transfer(sol_vault, treasury, leftover, &[vault_seeds.signer()])?;
    }

    crv.set_liquidity_locked();
    crv.set_fee_nft_mint(fee_nft_mint.key());
    crv.set_lp_amount(lp_amount);
    Ev::new(&ev::LIQUIDITY_LOCKED)
        .put(mint_key)
        .put(crv.pool())
        .put(fee_nft_mint.key())
        .u64(lp_amount)
        .u64(leftover)
        .emit(ea)
}
