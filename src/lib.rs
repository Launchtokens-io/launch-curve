//! launch_curve without Anchor: the same program (solana/SPEC.md) with the same interface (instruction
//! discriminators and data, account lists, account layouts, PDAs, events, error numbers, maths), built on pinocchio
//! for a smaller binary. Every check Anchor generated is written out in the order Anchor ran it; solana/LEAN.md lists
//! them per instruction. Not audited.
#![no_std]

mod checks;
mod consts;
mod cpi;
mod err;
mod ix;
mod math;
mod state;

use consts::ix as d;
use err::{e, DECLARED_PROGRAM_ID_MISMATCH, IDL_INSTRUCTION_STUB, INSTRUCTION_FALLBACK_NOT_FOUND};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult};

pub use consts::ID;

#[cfg(not(feature = "no-entrypoint"))]
mod entry {
    pinocchio::program_entrypoint!(crate::process_instruction, 32);
    pinocchio::no_allocator!();
    pinocchio::nostd_panic_handler!();
}

/// Anchor's `entry`: program id check, dispatch, and the error log on failure.
pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let r = dispatch(program_id, accounts, data);
    if let Err(err) = &r {
        err::log(err);
    }
    r
}

fn dispatch(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if program_id != &ID {
        return Err(e(DECLARED_PROGRAM_ID_MISMATCH));
    }
    if data.len() < 8 {
        return Err(e(INSTRUCTION_FALLBACK_NOT_FOUND));
    }
    // SAFETY: data.len() >= 8
    let tag = unsafe { core::ptr::read_unaligned(data.as_ptr() as *const u64) };
    let rest = unsafe { data.get_unchecked(8..) };
    match tag {
        d::INITIALIZE => ix::admin::initialize(accounts, rest),
        d::LAUNCH => ix::launch::launch(accounts, rest),
        d::BUY => ix::trade::buy(accounts, rest),
        d::SELL => ix::trade::sell(accounts, rest),
        d::GRADUATE => ix::graduate::graduate(accounts, rest),
        d::LOCK_LIQUIDITY => ix::graduate::lock_liquidity(accounts, rest),
        d::QUOTE_BUY => ix::trade::quote_buy(accounts, rest),
        d::QUOTE_SELL => ix::trade::quote_sell(accounts, rest),
        d::COLLECT_POOL_FEES => ix::fees::collect_pool_fees(accounts, rest),
        d::CLAIM_CREATOR_FEES => ix::fees::claim_creator_fees(accounts, rest),
        d::CLAIM_TREASURY_FEES => ix::admin::claim_treasury_fees(accounts, rest),
        d::PROPOSE_FEE_RECIPIENT => ix::fees::propose_fee_recipient(accounts, rest),
        d::CANCEL_FEE_RECIPIENT_PROPOSAL => ix::fees::cancel_fee_recipient_proposal(accounts, rest),
        d::ACCEPT_FEE_RECIPIENT => ix::fees::accept_fee_recipient(accounts, rest),
        d::SET_PAUSED => ix::admin::set_paused(accounts, rest),
        d::PROPOSE_PAUSER => ix::admin::propose_pauser(accounts, rest),
        d::ACCEPT_PAUSER => ix::admin::accept_pauser(accounts, rest),
        d::RENOUNCE_PAUSER => ix::admin::renounce_pauser(accounts, rest),
        d::IDL_TAG => Err(e(IDL_INSTRUCTION_STUB)),
        d::EVENT_TAG => event_dispatch(accounts),
        _ => Err(e(INSTRUCTION_FALLBACK_NOT_FOUND)),
    }
}

/// Anchor's `__event_dispatch`: the event self-CPI is accepted only when signed by the event authority PDA.
fn event_dispatch(accounts: &[AccountInfo]) -> ProgramResult {
    let a = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    if !a.is_signer() {
        return Err(e(err::CONSTRAINT_SIGNER));
    }
    checks::event_authority(a)
}
