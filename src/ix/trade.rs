//! `buy`, `sell`, `quote_buy`, `quote_sell` and the buy shared with `launch`
//! (programs/launch_curve/src/instructions/trade.rs).

use crate::checks::*;
use crate::consts::*;
use crate::cpi::{self, put, Ev, Pda};
use crate::err::*;
use crate::math::{self, BuyFill};
use crate::state::{Cfg, Crv};
use pinocchio::{
    account_info::AccountInfo,
    log::sol_log,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
};

/// The accounts one curve buy touches (shared by `buy` and the launch buy).
pub struct BuyCtx<'a> {
    pub buyer: &'a AccountInfo,
    pub config: &'a AccountInfo,
    pub curve: &'a AccountInfo,
    pub sol_vault: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub buyer_ata: &'a AccountInfo,
}

/// Runs one buy: charges the buyer `net + fee`, moves the tokens, credits the fees, and applies the holding cap
/// when `enforce_cap` (every buy except the launch buy). Same order of checks, transfers and writes as Anchor's.
#[allow(clippy::too_many_arguments)]
pub fn execute_buy(a: &BuyCtx, cfg: Cfg, crv: Crv, gross: u64, min_tokens_out: u64, enforce_cap: bool, now: i64) -> R<BuyFill> {
    if crv.graduated() {
        return Err(e(ALREADY_GRADUATED));
    }
    if crv.sold() >= CURVE_TOKENS {
        return Err(e(CURVE_COMPLETE));
    }
    if gross == 0 {
        return Err(e(ZERO_AMOUNT));
    }
    let f = math::buy_fill(crv.raised(), crv.sold(), cfg.target(), gross)?;
    if f.tokens_out == 0 {
        return Err(e(NOTHING_BOUGHT));
    }
    if f.tokens_out < min_tokens_out {
        return Err(e(SLIPPAGE_EXCEEDED));
    }
    let to_creator = math::creator_part(f.fee, cfg.creator_share_bps());
    let to_treasury = f.fee - to_creator;

    for (to, amount) in [(a.sol_vault, f.net), (a.curve, to_creator), (a.config, to_treasury)] {
        if amount > 0 {
            cpi::transfer(a.buyer, to, amount, &[])?;
        }
    }

    crv.set_raised(f.new_raised);
    crv.set_sold(f.new_sold);
    crv.set_creator_fees(crv.creator_fees() + to_creator);
    cfg.set_total_reserves(cfg.total_reserves() + f.net);
    cfg.set_total_creator_fees(cfg.total_creator_fees() + to_creator);
    cfg.set_treasury_fees(cfg.treasury_fees() + to_treasury);

    let bump = [crv.bump()];
    let seeds = Pda::new(CURVE_SEED, crv.mint(), &bump);
    cpi::token_transfer(a.vault, a.buyer_ata, a.curve, f.tokens_out, &[seeds.signer()])?;

    if enforce_cap && now < crv.protection_ends_at() {
        let held = token_layout(a.buyer_ata)?.amount();
        if held > crv.max_wallet() {
            return Err(e(HOLDING_CAP_EXCEEDED));
        }
    }
    Ok(f)
}

/// The `Trade` event.
#[allow(clippy::too_many_arguments)]
pub fn trade_event(ea: &AccountInfo, mint: &[u8; 32], trader: &[u8; 32], is_buy: bool, sol: u64, tokens: u64, fee: u64, raised: u64, sold: u64, now: i64) -> R<()> {
    Ev::new(&ev::TRADE).put(mint).put(trader).bool(is_buy).u64(sol).u64(tokens).u64(fee).u64(raised).u64(sold).i64(now).emit(ea)
}

pub fn buy(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: Buy");
    let mut rd = Rd::new(data);
    let lamports_in = rd.u64()?;
    let min_tokens_out = rd.u64()?;
    let deadline = rd.i64()?;

    let mut c = Cursor::new(accounts);
    let buyer = c.signer()?;
    let (config, cfg) = c.config()?;
    let (curve, crv) = c.curve()?;
    let sol_vault = c.system()?;
    let mint = c.mint()?;
    let (vault, vault_t) = c.token()?;
    let buyer_ata = c.any()?; // init_if_needed
    let token_program = c.program(&TOKEN_PROGRAM)?;
    let _ata_program = c.program(&ATA_PROGRAM)?;
    let system_program = c.program(&SYSTEM_PROGRAM)?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    // init_if_needed, associated_token::mint = mint, associated_token::authority = buyer
    let rent = Rent::get()?;
    let derived = ata(buyer.key(), mint.key());
    if buyer_ata.owner() == &SYSTEM_PROGRAM {
        cpi::create_ata_derived(&derived, buyer, buyer_ata, buyer, mint, system_program, token_program)?;
    }
    if buyer_ata.owner() == &SYSTEM_PROGRAM && buyer_ata.lamports() == 0 {
        return Err(e(ACCOUNT_NOT_INITIALIZED));
    }
    if buyer_ata.owner() != &TOKEN_PROGRAM {
        return Err(e(ACCOUNT_OWNED_BY_WRONG_PROGRAM));
    }
    let t = token_layout(buyer_ata)?;
    same(t.mint(), mint.key(), CONSTRAINT_TOKEN_MINT)?;
    same(t.owner(), buyer.key(), CONSTRAINT_TOKEN_OWNER)?;
    same(buyer_ata.owner(), token_program.key(), CONSTRAINT_ASSOCIATED_TOKEN_TOKEN_PROGRAM)?;
    same(buyer_ata.key(), &derived, ACCOUNT_NOT_ASSOCIATED_TOKEN_ACCOUNT)?;
    // (Anchor then re-runs the associated_token owner/address checks, which can't fail after the ones above)
    mutable(buyer_ata)?;
    if !rent.is_exempt(buyer_ata.lamports(), buyer_ata.data_len()) {
        return Err(e(CONSTRAINT_RENT_EXEMPT));
    }

    mutable(buyer)?;
    config_seeds(config)?;
    mutable(config)?;
    seeds_with_bump(curve, CURVE_SEED, mint.key(), crv.bump())?;
    mutable(curve)?;
    same(crv.mint(), mint.key(), CONSTRAINT_HAS_ONE)?;
    seeds_with_bump(sol_vault, SOL_VAULT_SEED, mint.key(), crv.vault_bump())?;
    mutable(sol_vault)?;
    associated(vault, &vault_t, curve.key(), mint.key())?;
    mutable(vault)?;
    event_authority(ea)?;

    let now = Clock::get()?.unix_timestamp;
    if cfg.paused() {
        return Err(e(LAUNCHES_PAUSED));
    }
    if now > deadline {
        return Err(e(EXPIRED));
    }
    let f = execute_buy(&BuyCtx { buyer, config, curve, sol_vault, vault, buyer_ata }, cfg, crv, lamports_in, min_tokens_out, true, now)?;
    trade_event(ea, mint.key(), buyer.key(), true, f.net + f.fee, f.tokens_out, f.fee, f.new_raised, f.new_sold, now)
}

/// Sells back to the curve. Never paused.
pub fn sell(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: Sell");
    let mut rd = Rd::new(data);
    let tokens_in = rd.u64()?;
    let min_lamports_out = rd.u64()?;
    let deadline = rd.i64()?;

    let mut c = Cursor::new(accounts);
    let seller = c.signer()?;
    let (config, cfg) = c.config()?;
    let (curve, crv) = c.curve()?;
    let sol_vault = c.system()?;
    let mint = c.mint()?;
    let (vault, vault_t) = c.token()?;
    let (seller_ata, seller_t) = c.token()?;
    let _token_program = c.program(&TOKEN_PROGRAM)?;
    let _system_program = c.program(&SYSTEM_PROGRAM)?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    mutable(seller)?;
    config_seeds(config)?;
    mutable(config)?;
    seeds_with_bump(curve, CURVE_SEED, mint.key(), crv.bump())?;
    mutable(curve)?;
    same(crv.mint(), mint.key(), CONSTRAINT_HAS_ONE)?;
    seeds_with_bump(sol_vault, SOL_VAULT_SEED, mint.key(), crv.vault_bump())?;
    mutable(sol_vault)?;
    associated(vault, &vault_t, curve.key(), mint.key())?;
    mutable(vault)?;
    mutable(seller_ata)?;
    same(seller_t.owner(), seller.key(), CONSTRAINT_TOKEN_OWNER)?;
    same(seller_t.mint(), mint.key(), CONSTRAINT_TOKEN_MINT)?;
    event_authority(ea)?;

    let now = Clock::get()?.unix_timestamp;
    if now > deadline {
        return Err(e(EXPIRED));
    }
    if crv.graduated() {
        return Err(e(ALREADY_GRADUATED));
    }
    if tokens_in == 0 {
        return Err(e(ZERO_AMOUNT));
    }
    if tokens_in > crv.sold() {
        return Err(e(EXCEEDS_SOLD));
    }
    let f = math::sell_fill(crv.raised(), crv.sold(), cfg.target(), tokens_in)?;
    if f.lamports_out == 0 {
        return Err(e(ZERO_AMOUNT));
    }
    if f.lamports_out < min_lamports_out {
        return Err(e(SLIPPAGE_EXCEEDED));
    }

    cpi::token_transfer(seller_ata, vault, seller, tokens_in, &[])?;

    let to_creator = math::creator_part(f.fee, cfg.creator_share_bps());
    let to_treasury = f.fee - to_creator;
    crv.set_raised(f.new_raised);
    crv.set_sold(f.new_sold);
    crv.set_creator_fees(crv.creator_fees() + to_creator);
    cfg.set_total_reserves(cfg.total_reserves() - f.gross);
    cfg.set_total_creator_fees(cfg.total_creator_fees() + to_creator);
    cfg.set_treasury_fees(cfg.treasury_fees() + to_treasury);

    let bump = [crv.vault_bump()];
    let seeds = Pda::new(SOL_VAULT_SEED, mint.key(), &bump);
    for (to, amount) in [(seller, f.lamports_out), (curve, to_creator), (config, to_treasury)] {
        if amount > 0 {
            cpi::transfer(sol_vault, to, amount, &[seeds.signer()])?;
        }
    }
    trade_event(ea, mint.key(), seller.key(), false, f.lamports_out, tokens_in, f.fee, f.new_raised, f.new_sold, now)
}

fn quote_accounts(accounts: &[AccountInfo]) -> R<(Cfg, Crv)> {
    let mut c = Cursor::new(accounts);
    let (config, cfg) = c.config()?;
    let (_curve, crv) = c.curve()?;
    config_seeds(config)?;
    Ok((cfg, crv))
}

/// What `buy` would do with `lamports_in` now (ignores the holding cap and the pause). Returns Borsh `BuyQuote`.
pub fn quote_buy(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: QuoteBuy");
    let lamports_in = Rd::new(data).u64()?;
    let (cfg, crv) = quote_accounts(accounts)?;
    if crv.graduated() {
        return Err(e(ALREADY_GRADUATED));
    }
    if crv.sold() >= CURVE_TOKENS {
        return Err(e(CURVE_COMPLETE));
    }
    let f = math::buy_fill(crv.raised(), crv.sold(), cfg.target(), lamports_in)?;
    let mut out = [0u8; 33];
    put(&mut out, 0, &f.tokens_out.to_le_bytes())?;
    put(&mut out, 8, &f.fee.to_le_bytes())?;
    put(&mut out, 16, &(f.net + f.fee).to_le_bytes())?;
    put(&mut out, 24, &f.refund.to_le_bytes())?;
    out[32] = f.completes as u8;
    pinocchio::cpi::set_return_data(&out);
    Ok(())
}

/// What `sell` would pay for `tokens_in` now. Returns Borsh `SellQuote`.
pub fn quote_sell(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: QuoteSell");
    let tokens_in = Rd::new(data).u64()?;
    let (cfg, crv) = quote_accounts(accounts)?;
    if crv.graduated() {
        return Err(e(ALREADY_GRADUATED));
    }
    if tokens_in > crv.sold() {
        return Err(e(EXCEEDS_SOLD));
    }
    let f = math::sell_fill(crv.raised(), crv.sold(), cfg.target(), tokens_in)?;
    let mut out = [0u8; 16];
    put(&mut out, 0, &f.lamports_out.to_le_bytes())?;
    put(&mut out, 8, &f.fee.to_le_bytes())?;
    pinocchio::cpi::set_return_data(&out);
    Ok(())
}
