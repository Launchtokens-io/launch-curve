//! `launch` (programs/launch_curve/src/instructions/launch.rs).

use crate::checks::*;
use crate::consts::*;
use crate::cpi::{self, Ev, Pda};
use crate::err::*;
use crate::ix::admin::init_pda;
use crate::ix::trade::{execute_buy, trade_event, BuyCtx};
use crate::math;
use crate::state::{Crv, CURVE_LEN};
use pinocchio::{
    account_info::AccountInfo,
    log::sol_log,
    pubkey::find_program_address,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
};

pub fn launch(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: Launch");
    // LaunchArgs { name, symbol, uri, metadata_hash, launch_buy_lamports, min_tokens_out }
    let mut rd = Rd::new(data);
    let name = rd.string()?;
    let symbol = rd.string()?;
    let uri = rd.string()?;
    let metadata_hash = rd.take(32)?;
    let launch_buy = rd.u64()?;
    let min_tokens_out = rd.u64()?;

    let mut c = Cursor::new(accounts);
    let creator = c.signer()?;
    let (config, cfg) = c.config()?;
    let treasury = c.any()?;
    let mint = c.signer()?;
    let curve = c.any()?; // init
    let sol_vault = c.system()?;
    let curve_vault = c.any()?;
    let creator_ata = c.any()?;
    let metadata = c.any()?;
    let token_program = c.program(&TOKEN_PROGRAM)?;
    let _ata_program = c.program(&ATA_PROGRAM)?;
    let metadata_program = c.any()?;
    let system_program = c.program(&SYSTEM_PROGRAM)?;
    let rent_acc = c.any()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    // init curve: seeds = [CURVE_SEED, mint], bump; payer = creator; space = 8 + Curve::INIT_SPACE
    let rent = Rent::get()?;
    let (curve_key, curve_bump) = find_program_address(&[CURVE_SEED, mint.key()], &ID);
    same(curve.key(), &curve_key, CONSTRAINT_SEEDS)?;
    let cb = [curve_bump];
    let curve_seeds = Pda::new(CURVE_SEED, mint.key(), &cb);
    init_pda(creator, curve, CURVE_LEN, &rent, curve_seeds.signer())?;

    mutable(creator)?;
    config_seeds(config)?;
    mutable(config)?;
    same(cfg.treasury(), treasury.key(), CONSTRAINT_HAS_ONE)?;
    mutable(treasury)?;
    mutable(mint)?;
    let vault_bump = seeds_find(sol_vault, SOL_VAULT_SEED, mint.key())?;
    mutable(sol_vault)?;
    mutable(curve_vault)?;
    same(curve_vault.key(), &ata(curve.key(), mint.key()), ACCOUNT_MISMATCH)?;
    mutable(creator_ata)?;
    same(creator_ata.key(), &ata(creator.key(), mint.key()), ACCOUNT_MISMATCH)?;
    // seeds = [b"metadata", token_metadata_program, mint], seeds::program = token_metadata_program
    let (md, _) = find_program_address(&[b"metadata", metadata_program.key(), mint.key()], metadata_program.key());
    same(metadata.key(), &md, CONSTRAINT_SEEDS)?;
    mutable(metadata)?;
    same(metadata_program.key(), &METADATA_PROGRAM, CONSTRAINT_ADDRESS)?;
    rent_sysvar(rent_acc)?;
    event_authority(ea)?;

    // ---- handler
    let now = Clock::get()?.unix_timestamp;
    if cfg.paused() {
        return Err(e(LAUNCHES_PAUSED));
    }
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(e(INVALID_NAME));
    }
    if symbol.is_empty() || symbol.len() > MAX_SYMBOL_BYTES {
        return Err(e(INVALID_SYMBOL));
    }
    if uri.len() > MAX_URI_BYTES {
        return Err(e(METADATA_TOO_LONG));
    }
    let target = cfg.target();
    if launch_buy > 0 && math::buy_fill(0, 0, target, launch_buy)?.completes {
        return Err(e(LAUNCH_BUY_WOULD_COMPLETE));
    }

    if cfg.launch_fee() > 0 {
        cpi::transfer(creator, treasury, cfg.launch_fee(), &[])?;
    }
    // the SOL vault starts at its rent-exempt minimum, so any buy can land in it
    let vault_rent = rent.minimum_balance(0).saturating_sub(sol_vault.lamports());
    if vault_rent > 0 {
        cpi::transfer(creator, sol_vault, vault_rent, &[])?;
    }

    // the mint: classic SPL, 6 decimals, mint authority the curve PDA, no freeze authority
    cpi::create_account(creator, mint, rent.minimum_balance(82), 82, &TOKEN_PROGRAM, &[])?;
    cpi::initialize_mint2(mint, curve.key(), DECIMALS)?;
    for (ata, owner) in [(curve_vault, curve), (creator_ata, creator)] {
        cpi::create_ata(true, creator, ata, owner, mint, system_program, token_program, &[])?;
    }
    // immutable metadata: no creators, no royalties, update authority the curve PDA but is_mutable = false
    cpi::create_metadata_v3(metadata_program, metadata, mint, curve, creator, system_program, name, symbol, uri, &[curve_seeds.signer()])?;
    cpi::mint_to(mint, curve_vault, curve, SUPPLY, &[curve_seeds.signer()])?;
    cpi::revoke_mint_authority(mint, curve, &[curve_seeds.signer()])?;

    let launch_index = cfg.launch_count() + 1;
    cfg.set_launch_count(launch_index);
    let (pool, _) = find_program_address(&[POOL_SEED, mint.key()], &ID);
    let crv = Crv(curve.data_ptr());
    let protection_ends_at = now + cfg.protection_duration() as i64;
    let max_wallet = (SUPPLY as u128 * cfg.max_wallet_bps() as u128 / BPS as u128) as u64;
    crv.init(curve_bump, vault_bump, mint.key(), creator.key(), now, protection_ends_at, max_wallet, launch_index, &pool);

    let mut trade = None;
    if launch_buy > 0 {
        let ctx = BuyCtx { buyer: creator, config, curve, sol_vault, vault: curve_vault, buyer_ata: creator_ata };
        trade = Some(execute_buy(&ctx, cfg, crv, launch_buy, min_tokens_out, false, now)?);
    }

    Ev::new(&ev::CURVE_LAUNCHED)
        .put(mint.key())
        .put(creator.key())
        .put(curve.key())
        .put(&pool)
        .u64(SUPPLY)
        .u64(CURVE_TOKENS)
        .u64(POOL_TOKENS)
        .u64(target)
        .i64(protection_ends_at)
        .u64(max_wallet)
        .u64(trade.map_or(0, |f| f.net + f.fee))
        .u64(trade.map_or(0, |f| f.tokens_out))
        .u64(launch_index)
        .i64(now)
        .emit(ea)?;
    Ev::new(&ev::CURVE_METADATA).put(mint.key()).str(name).str(symbol).str(uri).put(metadata_hash).emit(ea)?;
    if let Some(f) = trade {
        trade_event(ea, mint.key(), creator.key(), true, f.net + f.fee, f.tokens_out, f.fee, f.new_raised, f.new_sold, now)?;
    }
    Ok(())
}
