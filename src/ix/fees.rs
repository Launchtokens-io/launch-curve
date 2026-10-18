//! `claim_creator_fees`, the fee-recipient move (programs/launch_curve/src/instructions/fees.rs) and
//! `collect_pool_fees` (programs/launch_curve/src/instructions/graduate.rs).

use crate::checks::*;
use crate::consts::{dex, *};
use crate::cpi::{self, Ev, Pda};
use crate::err::*;
use crate::state::Crv;
use pinocchio::{account_info::AccountInfo, log::sol_log, program_error::ProgramError};

/// Anchor's `sub_lamports` / `add_lamports` on two accounts this program may debit (checked, ArithmeticOverflow).
pub fn move_lamports(from: &AccountInfo, to: &AccountInfo, amount: u64) -> R<()> {
    let f = from.lamports().checked_sub(amount).ok_or(ProgramError::ArithmeticOverflow)?;
    *from.try_borrow_mut_lamports()? = f;
    let t = to.lamports().checked_add(amount).ok_or(ProgramError::ArithmeticOverflow)?;
    *to.try_borrow_mut_lamports()? = t;
    Ok(())
}

/// `seeds = [CURVE_SEED, curve.mint.as_ref()], bump = curve.bump` (the curve's own mint).
fn own_curve_seeds(curve: &AccountInfo, crv: Crv) -> R<()> {
    seeds_with_bump(curve, CURVE_SEED, crv.mint(), crv.bump())
}

/// Pays the curve's accrued creator fees to its fee recipient. Works paused or not, before or after graduation.
pub fn claim_creator_fees(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: ClaimCreatorFees");
    let mut c = Cursor::new(accounts);
    let recipient = c.signer()?;
    let (config, cfg) = c.config()?;
    let (curve, crv) = c.curve()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    mutable(recipient)?;
    config_seeds(config)?;
    mutable(config)?;
    own_curve_seeds(curve, crv)?;
    mutable(curve)?;
    same(crv.fee_recipient(), recipient.key(), NOT_FEE_RECIPIENT)?;
    event_authority(ea)?;

    let amount = crv.creator_fees();
    if amount == 0 {
        return Err(e(NOTHING_TO_CLAIM));
    }
    crv.set_creator_fees(0);
    cfg.set_total_creator_fees(cfg.total_creator_fees() - amount);
    move_lamports(curve, recipient, amount)?;
    Ev::new(&ev::CREATOR_FEES_CLAIMED).put(crv.mint()).put(recipient.key()).u64(amount).emit(ea)
}

/// The accounts of `FeeRecipientOnly` (fee_recipient signer, curve with `has_one = fee_recipient`).
fn fee_recipient_only(accounts: &[AccountInfo]) -> R<(&AccountInfo, Crv, &AccountInfo)> {
    let mut c = Cursor::new(accounts);
    let recipient = c.signer()?;
    let (curve, crv) = c.curve()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)
    own_curve_seeds(curve, crv)?;
    mutable(curve)?;
    same(crv.fee_recipient(), recipient.key(), NOT_FEE_RECIPIENT)?;
    event_authority(ea)?;
    Ok((recipient, crv, ea))
}

fn proposed_event(ea: &AccountInfo, crv: Crv, proposed: &[u8; 32]) -> R<()> {
    Ev::new(&ev::FEE_RECIPIENT_PROPOSED).put(crv.mint()).put(crv.fee_recipient()).put(proposed).emit(ea)
}

/// Step 1 of moving the creator earnings: the current recipient names a new wallet.
pub fn propose_fee_recipient(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: ProposeFeeRecipient");
    let new_recipient = *Rd::new(data).key()?;
    let (_, crv, ea) = fee_recipient_only(accounts)?;
    if new_recipient == [0u8; 32] {
        return Err(e(INVALID_PARAMETER));
    }
    crv.set_pending_fee_recipient(&new_recipient);
    proposed_event(ea, crv, &new_recipient)
}

/// The current recipient withdraws a pending proposal.
pub fn cancel_fee_recipient_proposal(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: CancelFeeRecipientProposal");
    let (_, crv, ea) = fee_recipient_only(accounts)?;
    if crv.pending_fee_recipient() == &[0u8; 32] {
        return Err(e(NO_PROPOSAL));
    }
    crv.set_pending_fee_recipient(&[0u8; 32]);
    proposed_event(ea, crv, &[0u8; 32])
}

/// Step 2: the proposed wallet accepts. Unclaimed creator fees move with it.
pub fn accept_fee_recipient(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: AcceptFeeRecipient");
    let mut c = Cursor::new(accounts);
    let new_recipient = c.signer()?;
    let (curve, crv) = c.curve()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)
    own_curve_seeds(curve, crv)?;
    mutable(curve)?;
    event_authority(ea)?;

    let pending = *crv.pending_fee_recipient();
    if pending == [0u8; 32] {
        return Err(e(NO_PROPOSAL));
    }
    if &pending != new_recipient.key() {
        return Err(e(NOT_PROPOSED_RECIPIENT));
    }
    let previous = *crv.fee_recipient();
    crv.set_fee_recipient(&pending);
    crv.set_pending_fee_recipient(&[0u8; 32]);
    Ev::new(&ev::FEE_RECIPIENT_CHANGED).put(crv.mint()).put(&previous).put(&pending).emit(ea)
}

/// Harvests the locked position's fees and splits them between the fee recipient and the treasury. Permissionless.
pub fn collect_pool_fees(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: CollectPoolFees");
    let mut c = Cursor::new(accounts);
    let payer = c.signer()?;
    let (config, cfg) = c.config()?;
    let (curve, crv) = c.curve()?;
    let mint = c.mint()?;
    let wsol_mint = c.mint()?;
    let sol_vault = c.any()?;
    let lock_program = c.any()?;
    let lock_authority = c.any()?;
    let fee_nft_account = c.any()?;
    let locked_liquidity = c.any()?;
    let cpmm_program = c.any()?;
    let cpmm_authority = c.any()?;
    let pool_state = c.any()?;
    let lp_mint = c.any()?;
    let (vault_wsol, vault_wsol_t) = c.token()?;
    let (vault_token, vault_token_t) = c.token()?;
    let pool_vault_0 = c.any()?;
    let pool_vault_1 = c.any()?;
    let locked_lp_vault = c.any()?;
    let (recipient_wsol, _) = c.token()?;
    let (recipient_token, _) = c.token()?;
    let (treasury_wsol, _) = c.token()?;
    let (treasury_token, _) = c.token()?;
    let token_program = c.program(&TOKEN_PROGRAM)?;
    let token_program_2022 = c.any()?;
    let memo_program = c.any()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    mutable(payer)?;
    config_seeds(config)?;
    seeds_with_bump(curve, CURVE_SEED, mint.key(), crv.bump())?;
    same(crv.mint(), mint.key(), CONSTRAINT_HAS_ONE)?;
    same(wsol_mint.key(), &WSOL_MINT, CONSTRAINT_ADDRESS)?;
    seeds_with_bump(sol_vault, SOL_VAULT_SEED, mint.key(), crv.vault_bump())?;
    same(lock_program.key(), &dex::LOCK_PROGRAM, CONSTRAINT_ADDRESS)?;
    same(lock_authority.key(), &dex::LOCK_AUTHORITY, CONSTRAINT_ADDRESS)?;
    mutable(fee_nft_account)?;
    same(fee_nft_account.key(), &ata(sol_vault.key(), crv.fee_nft_mint()), ACCOUNT_MISMATCH)?;
    mutable(locked_liquidity)?;
    same(cpmm_program.key(), &dex::CPMM_PROGRAM, CONSTRAINT_ADDRESS)?;
    same(cpmm_authority.key(), &dex::CPMM_AUTHORITY, CONSTRAINT_ADDRESS)?;
    mutable(pool_state)?;
    same(pool_state.key(), crv.pool(), ACCOUNT_MISMATCH)?;
    mutable(lp_mint)?;
    same(lp_mint.key(), crv.lp_mint(), ACCOUNT_MISMATCH)?;
    associated(vault_wsol, &vault_wsol_t, sol_vault.key(), wsol_mint.key())?;
    mutable(vault_wsol)?;
    associated(vault_token, &vault_token_t, sol_vault.key(), mint.key())?;
    mutable(vault_token)?;
    mutable(pool_vault_0)?;
    mutable(pool_vault_1)?;
    mutable(locked_lp_vault)?;
    mutable(recipient_wsol)?;
    same(recipient_wsol.key(), &ata(crv.fee_recipient(), &WSOL_MINT), ACCOUNT_MISMATCH)?;
    mutable(recipient_token)?;
    same(recipient_token.key(), &ata(crv.fee_recipient(), mint.key()), ACCOUNT_MISMATCH)?;
    mutable(treasury_wsol)?;
    same(treasury_wsol.key(), &ata(cfg.treasury(), &WSOL_MINT), ACCOUNT_MISMATCH)?;
    mutable(treasury_token)?;
    same(treasury_token.key(), &ata(cfg.treasury(), mint.key()), ACCOUNT_MISMATCH)?;
    same(token_program_2022.key(), &TOKEN_2022_PROGRAM, CONSTRAINT_ADDRESS)?;
    same(memo_program.key(), &MEMO_PROGRAM, CONSTRAINT_ADDRESS)?;
    event_authority(ea)?;

    // ---- handler
    if !crv.liquidity_locked() {
        return Err(e(NOT_LOCKED));
    }
    let vb = [crv.vault_bump()];
    let vault_seeds = Pda::new(SOL_VAULT_SEED, mint.key(), &vb);
    let wsol_first = WSOL_MINT < *mint.key();
    let (m0, m1, r0, r1) = if wsol_first { (wsol_mint, mint, vault_wsol, vault_token) } else { (mint, wsol_mint, vault_token, vault_wsol) };
    cpi::collect_cp_fees(
        lock_program,
        [
            lock_authority,
            sol_vault, // fee_nft_owner
            fee_nft_account,
            locked_liquidity,
            cpmm_program,
            cpmm_authority,
            pool_state,
            lp_mint,
            r0,
            r1,
            pool_vault_0,
            pool_vault_1,
            m0,
            m1,
            locked_lp_vault,
            token_program,
            token_program_2022,
            memo_program,
        ],
        u64::MAX,
        &[vault_seeds.signer()],
    )?;

    // Account::reload: the SPL layout again (no owner check, as in Anchor; the Token program still owns both)
    let sol = token_layout(vault_wsol)?.amount();
    let tokens = token_layout(vault_token)?.amount();
    let share = cfg.pool_creator_share_bps() as u128;
    let sol_to_recipient = (sol as u128 * share / BPS as u128) as u64;
    let tokens_to_recipient = (tokens as u128 * share / BPS as u128) as u64;
    for (from, to, amount) in [
        (vault_wsol, recipient_wsol, sol_to_recipient),
        (vault_wsol, treasury_wsol, sol - sol_to_recipient),
        (vault_token, recipient_token, tokens_to_recipient),
        (vault_token, treasury_token, tokens - tokens_to_recipient),
    ] {
        if amount > 0 {
            cpi::token_transfer(from, to, sol_vault, amount, &[vault_seeds.signer()])?;
        }
    }

    Ev::new(&ev::POOL_FEES_COLLECTED)
        .put(mint.key())
        .put(crv.pool())
        .put(crv.fee_recipient())
        .u64(sol_to_recipient)
        .u64(sol - sol_to_recipient)
        .u64(tokens_to_recipient)
        .u64(tokens - tokens_to_recipient)
        .emit(ea)
}
