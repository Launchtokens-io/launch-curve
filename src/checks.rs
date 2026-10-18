//! The account checks Anchor's `#[derive(Accounts)]` generated, written out. Anchor validates in two passes and this
//! program keeps that order (so a bad account fails with the same error number):
//!   1. load every account in declaration order (`Cursor`): missing → AccountNotEnoughKeys; `Signer` → is_signer;
//!      `Program<T>` → key, executable; `SystemAccount` → owner; `Account<T>` → initialized, owner, discriminator /
//!      SPL layout;
//!   2. `init` / `init_if_needed` accounts (create, then check), then every other account's constraints in
//!      declaration order, each in Anchor's order: seeds, associated_token, mut, signer, has_one, raw, owner,
//!      rent_exempt, executable, address, token::*.
//! See solana/LEAN.md for the per-instruction list.

use crate::consts::*;
use crate::err::*;
use crate::state::{Cfg, Crv, Tok, CONFIG_LEN, CURVE_LEN};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{create_program_address, find_program_address, Pubkey},
};

pub type R<T> = Result<T, ProgramError>;

pub struct Cursor<'a> {
    accs: &'a [AccountInfo],
    i: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(accs: &'a [AccountInfo]) -> Self {
        Self { accs, i: 0 }
    }

    /// `UncheckedAccount` / `AccountInfo`, and the raw slot of an `init` account.
    #[inline(never)]
    pub fn any(&mut self) -> R<&'a AccountInfo> {
        let a = self.accs.get(self.i).ok_or(e(ACCOUNT_NOT_ENOUGH_KEYS))?;
        self.i += 1;
        Ok(a)
    }

    /// `Signer<'info>`
    #[inline(never)]
    pub fn signer(&mut self) -> R<&'a AccountInfo> {
        let a = self.any()?;
        if !a.is_signer() {
            return Err(e(ACCOUNT_NOT_SIGNER));
        }
        Ok(a)
    }

    /// `Program<'info, T>`
    #[inline(never)]
    pub fn program(&mut self, id: &Pubkey) -> R<&'a AccountInfo> {
        let a = self.any()?;
        if a.key() != id {
            return Err(e(INVALID_PROGRAM_ID));
        }
        if !a.executable() {
            return Err(e(INVALID_PROGRAM_EXECUTABLE));
        }
        Ok(a)
    }

    /// `SystemAccount<'info>`
    #[inline(never)]
    pub fn system(&mut self) -> R<&'a AccountInfo> {
        let a = self.any()?;
        if a.owner() != &SYSTEM_PROGRAM {
            return Err(e(ACCOUNT_NOT_SYSTEM_OWNED));
        }
        Ok(a)
    }

    /// `Account<'info, Config>`
    pub fn config(&mut self) -> R<(&'a AccountInfo, Cfg)> {
        let a = self.any()?;
        let p = program_account(a, &CONFIG_DISC, CONFIG_LEN, &[107])?;
        Ok((a, Cfg(p)))
    }

    /// `Account<'info, Curve>`
    pub fn curve(&mut self) -> R<(&'a AccountInfo, Crv)> {
        let a = self.any()?;
        let p = program_account(a, &CURVE_DISC, CURVE_LEN, &[194, 195])?;
        Ok((a, Crv(p)))
    }

    /// `Account<'info, anchor_spl::token::Mint>`
    pub fn mint(&mut self) -> R<&'a AccountInfo> {
        let a = self.any()?;
        token_owned(a)?;
        mint_layout(a)?;
        Ok(a)
    }

    /// `Account<'info, anchor_spl::token::TokenAccount>`
    pub fn token(&mut self) -> R<(&'a AccountInfo, Tok)> {
        let a = self.any()?;
        token_owned(a)?;
        let t = token_layout(a)?;
        Ok((a, t))
    }
}

/// `Account::try_from` for this program's accounts: AccountNotInitialized, AccountOwnedByWrongProgram, then
/// `try_deserialize` (AccountDiscriminatorNotFound / Mismatch, AccountDidNotDeserialize when Borsh would fail: data
/// shorter than the struct or a bool byte other than 0/1).
#[inline(never)]
fn program_account(a: &AccountInfo, disc: &[u8; 8], len: usize, bools: &[usize]) -> R<*mut u8> {
    if a.owner() == &SYSTEM_PROGRAM && a.lamports() == 0 {
        return Err(e(ACCOUNT_NOT_INITIALIZED));
    }
    if a.owner() != &ID {
        return Err(e(ACCOUNT_OWNED_BY_WRONG_PROGRAM));
    }
    let n = a.data_len();
    let p = a.data_ptr();
    if n < 8 {
        return Err(e(ACCOUNT_DISCRIMINATOR_NOT_FOUND));
    }
    if unsafe { core::slice::from_raw_parts(p, 8) } != disc {
        return Err(e(ACCOUNT_DISCRIMINATOR_MISMATCH));
    }
    if n < len || bools.iter().any(|&o| unsafe { *p.add(o) } > 1) {
        return Err(e(ACCOUNT_DID_NOT_DESERIALIZE));
    }
    Ok(p)
}

/// The owner half of `Account::try_from` for SPL types (owner() = the classic Token program).
#[inline(never)]
fn token_owned(a: &AccountInfo) -> R<()> {
    if a.owner() == &SYSTEM_PROGRAM && a.lamports() == 0 {
        return Err(e(ACCOUNT_NOT_INITIALIZED));
    }
    if a.owner() != &TOKEN_PROGRAM {
        return Err(e(ACCOUNT_OWNED_BY_WRONG_PROGRAM));
    }
    Ok(())
}

#[inline(always)]
fn coption_ok(p: *const u8, o: usize) -> bool {
    let tag = unsafe { core::ptr::read_unaligned(p.add(o) as *const u32) };
    tag <= 1
}

/// `spl_token::state::Mint::unpack`: exactly 82 bytes, valid COption tags and bool, initialized.
#[inline(never)]
pub fn mint_layout(a: &AccountInfo) -> R<()> {
    let p = a.data_ptr();
    if a.data_len() != 82 || !coption_ok(p, 0) || !coption_ok(p, 46) || unsafe { *p.add(45) } > 1 {
        return Err(ProgramError::InvalidAccountData);
    }
    if unsafe { *p.add(45) } == 0 {
        return Err(ProgramError::UninitializedAccount);
    }
    Ok(())
}

/// `spl_token::state::Account::unpack` (what Anchor's `TokenAccount::try_deserialize` runs): exactly 165 bytes,
/// valid COption tags, state Initialized or Frozen.
#[inline(never)]
pub fn token_layout(a: &AccountInfo) -> R<Tok> {
    let p = a.data_ptr();
    if a.data_len() != 165 || !coption_ok(p, 72) || !coption_ok(p, 109) || !coption_ok(p, 129) || unsafe { *p.add(108) } > 2 {
        return Err(ProgramError::InvalidAccountData);
    }
    if unsafe { *p.add(108) } == 0 {
        return Err(ProgramError::UninitializedAccount);
    }
    Ok(Tok(p))
}

/// `#[account(mut)]`
#[inline(never)]
pub fn mutable(a: &AccountInfo) -> R<()> {
    if !a.is_writable() {
        return Err(e(CONSTRAINT_MUT));
    }
    Ok(())
}

/// `address = <expected> [@ code]`, `has_one = <field> [@ code]` and the raw `constraint = a == b @ code` checks.
#[inline(never)]
pub fn same(actual: &Pubkey, expected: &Pubkey, code: u32) -> R<()> {
    if actual != expected {
        return Err(e(code));
    }
    Ok(())
}

/// `seeds = [...], bump = <stored>`: `create_program_address` with the stored bump (ConstraintSeeds when it fails
/// or differs).
#[inline(never)]
pub fn seeds_with_bump(a: &AccountInfo, seed0: &[u8], seed1: &[u8], bump: u8) -> R<()> {
    let b = [bump];
    match create_program_address(&[seed0, seed1, &b], &ID) {
        Ok(k) if &k == a.key() => Ok(()),
        _ => Err(e(CONSTRAINT_SEEDS)),
    }
}

/// `seeds = [...], bump` (no stored bump): `find_program_address`; returns the bump.
#[inline(never)]
pub fn seeds_find(a: &AccountInfo, seed0: &[u8], seed1: &[u8]) -> R<u8> {
    let (k, b) = find_program_address(&[seed0, seed1], &ID);
    if &k != a.key() {
        return Err(e(CONSTRAINT_SEEDS));
    }
    Ok(b)
}

/// The config PDA check (`seeds = [CONFIG_SEED], bump = config.bump`): the stored bump is the canonical one, so the
/// derived address is the constant CONFIG_PDA.
#[inline(always)]
pub fn config_seeds(a: &AccountInfo) -> R<()> {
    same(a.key(), &CONFIG_PDA, CONSTRAINT_SEEDS)
}

/// The `#[event_cpi]` event authority (`seeds = [b"__event_authority"], bump`).
#[inline(always)]
pub fn event_authority(a: &AccountInfo) -> R<()> {
    same(a.key(), &EVENT_AUTHORITY, CONSTRAINT_SEEDS)
}

/// `get_associated_token_address(wallet, mint)` (classic Token program).
#[inline(never)]
pub fn ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    find_program_address(&[wallet, &TOKEN_PROGRAM, mint], &ATA_PROGRAM).0
}

/// `associated_token::mint = <mint>, associated_token::authority = <wallet>` on an existing token account:
/// ConstraintTokenOwner, then ConstraintAssociated.
#[inline(never)]
pub fn associated(a: &AccountInfo, t: &Tok, wallet: &Pubkey, mint: &Pubkey) -> R<()> {
    if t.owner() != wallet {
        return Err(e(CONSTRAINT_TOKEN_OWNER));
    }
    if a.key() != &ata(wallet, mint) {
        return Err(e(CONSTRAINT_ASSOCIATED));
    }
    Ok(())
}

/// `rent = <sysvar>` address check (`@ AccountSysvarMismatch`).
#[inline(always)]
pub fn rent_sysvar(a: &AccountInfo) -> R<()> {
    same(a.key(), &RENT_SYSVAR, ACCOUNT_SYSVAR_MISMATCH)
}

/// Borsh reader for instruction arguments (any short or malformed input: InstructionDidNotDeserialize). Like
/// Anchor's `deserialize`, trailing bytes are ignored.
pub struct Rd<'a> {
    d: &'a [u8],
}

impl<'a> Rd<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        Self { d }
    }
    #[inline(never)]
    pub fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if self.d.len() < n {
            return Err(e(INSTRUCTION_DID_NOT_DESERIALIZE));
        }
        let (h, t) = self.d.split_at(n);
        self.d = t;
        Ok(h)
    }
    pub fn u64(&mut self) -> R<u64> {
        let b = self.take(8)?;
        Ok(unsafe { core::ptr::read_unaligned(b.as_ptr() as *const u64) })
    }
    pub fn i64(&mut self) -> R<i64> {
        Ok(self.u64()? as i64)
    }
    /// Borsh `bool`: 0 or 1.
    pub fn bool(&mut self) -> R<bool> {
        match self.take(1)? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(e(INSTRUCTION_DID_NOT_DESERIALIZE)),
        }
    }
    pub fn key(&mut self) -> R<&'a Pubkey> {
        let b = self.take(32)?;
        Ok(unsafe { &*(b.as_ptr() as *const Pubkey) })
    }
    /// Borsh `String`: u32 length, then valid UTF-8.
    #[inline(never)]
    pub fn string(&mut self) -> R<&'a str> {
        let n = unsafe { core::ptr::read_unaligned(self.take(4)?.as_ptr() as *const u32) } as usize;
        let b = self.take(n)?;
        core::str::from_utf8(b).map_err(|_| e(INSTRUCTION_DID_NOT_DESERIALIZE))
    }
}
