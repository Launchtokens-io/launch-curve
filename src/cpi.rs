//! Every cross-program call, with the exact instruction data and account metas the Anchor program's CPIs used
//! (anchor_lang::system_program, anchor_spl::token / associated_token, and the hand-written Metaplex / Raydium calls
//! of programs/launch_curve/src/dex.rs), so the callee sees the same instruction and the instruction trace has the
//! same length.

use crate::checks::R;
use crate::consts::*;
use pinocchio::{
    account_info::AccountInfo,
    cpi::{invoke_signed_unchecked, invoke_signed_with_bounds},
    instruction::{Account, AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Copies `s` into `d` at `o`. A write that would go past the end of `d` is refused (`BorshIoError`), never cut
/// short or written out of bounds. (Checked by hand rather than with slice indexing, whose panic path would pull
/// in core::fmt.) Every caller today writes a fixed-size piece at a fixed offset that fits.
#[inline(always)]
pub fn put(d: &mut [u8], o: usize, s: &[u8]) -> R<()> {
    if o > d.len() || s.len() > d.len() - o {
        return Err(ProgramError::BorshIoError);
    }
    // SAFETY: o + s.len() <= d.len(), checked above
    unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), d.as_mut_ptr().add(o), s.len()) };
    Ok(())
}

/// One copy of pinocchio's checked invoke for every call (max 20 accounts: Raydium's `initialize`).
#[inline(never)]
pub fn call(program: &Pubkey, data: &[u8], metas: &[AccountMeta], infos: &[&AccountInfo], signers: &[Signer]) -> R<()> {
    invoke_signed_with_bounds::<20>(&Instruction { program_id: program, data, accounts: metas }, infos, signers)
}

#[inline(always)]
pub fn w(a: &AccountInfo) -> AccountMeta {
    AccountMeta::writable(a.key())
}
#[inline(always)]
pub fn r(a: &AccountInfo) -> AccountMeta {
    AccountMeta::readonly(a.key())
}
#[inline(always)]
pub fn ws(a: &AccountInfo) -> AccountMeta {
    AccountMeta::writable_signer(a.key())
}
#[inline(always)]
pub fn rs(a: &AccountInfo) -> AccountMeta {
    AccountMeta::readonly_signer(a.key())
}

/// Seeds of a two-part PDA (`[prefix, key, [bump]]`) as a pinocchio signer.
pub struct Pda<'a> {
    seeds: [Seed<'a>; 3],
}

impl<'a> Pda<'a> {
    pub fn new(prefix: &'a [u8], key: &'a [u8], bump: &'a [u8; 1]) -> Self {
        Self { seeds: [Seed::from(prefix), Seed::from(key), Seed::from(&bump[..])] }
    }
    pub fn signer(&self) -> Signer<'_, '_> {
        Signer::from(&self.seeds[..])
    }
}

// ------------------------------------------------------------------------------------------------ system

/// `system_instruction::transfer` (signed when `from` is one of our PDAs).
#[inline(never)]
pub fn transfer(from: &AccountInfo, to: &AccountInfo, lamports: u64, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 12];
    d[0] = 2;
    put(&mut d, 4, &lamports.to_le_bytes())?;
    call(&SYSTEM_PROGRAM, &d, &[ws(from), w(to)], &[from, to], signers)
}

/// `system_instruction::create_account`
#[inline(never)]
pub fn create_account(payer: &AccountInfo, to: &AccountInfo, lamports: u64, space: u64, owner: &Pubkey, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 52];
    put(&mut d, 4, &lamports.to_le_bytes())?;
    put(&mut d, 12, &space.to_le_bytes())?;
    put(&mut d, 20, owner)?;
    call(&SYSTEM_PROGRAM, &d, &[ws(payer), ws(to)], &[payer, to], signers)
}

/// `system_instruction::allocate`
#[inline(never)]
pub fn allocate(a: &AccountInfo, space: u64, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 12];
    d[0] = 8;
    put(&mut d, 4, &space.to_le_bytes())?;
    call(&SYSTEM_PROGRAM, &d, &[ws(a)], &[a], signers)
}

/// `system_instruction::assign`
#[inline(never)]
pub fn assign(a: &AccountInfo, owner: &Pubkey, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 36];
    d[0] = 1;
    put(&mut d, 4, owner)?;
    call(&SYSTEM_PROGRAM, &d, &[ws(a)], &[a], signers)
}

// ------------------------------------------------------------------------------------------------ SPL Token

#[inline(never)]
fn token_amount_ix(tag: u8, a: &AccountInfo, b: &AccountInfo, auth: &AccountInfo, amount: u64, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 9];
    d[0] = tag;
    put(&mut d, 1, &amount.to_le_bytes())?;
    call(&TOKEN_PROGRAM, &d, &[w(a), w(b), rs(auth)], &[a, b, auth], signers)
}

/// `spl_token::instruction::transfer`
pub fn token_transfer(from: &AccountInfo, to: &AccountInfo, auth: &AccountInfo, amount: u64, signers: &[Signer]) -> R<()> {
    token_amount_ix(3, from, to, auth, amount, signers)
}

/// `spl_token::instruction::mint_to`
pub fn mint_to(mint: &AccountInfo, to: &AccountInfo, auth: &AccountInfo, amount: u64, signers: &[Signer]) -> R<()> {
    token_amount_ix(7, mint, to, auth, amount, signers)
}

/// `spl_token::instruction::burn`
pub fn burn(from: &AccountInfo, mint: &AccountInfo, auth: &AccountInfo, amount: u64, signers: &[Signer]) -> R<()> {
    token_amount_ix(8, from, mint, auth, amount, signers)
}

/// `spl_token::instruction::set_authority(mint, None, MintTokens, current)`
pub fn revoke_mint_authority(mint: &AccountInfo, auth: &AccountInfo, signers: &[Signer]) -> R<()> {
    call(&TOKEN_PROGRAM, &[6, 0, 0], &[w(mint), rs(auth)], &[mint, auth], signers)
}

/// `spl_token::instruction::sync_native`
pub fn sync_native(a: &AccountInfo) -> R<()> {
    call(&TOKEN_PROGRAM, &[17], &[w(a)], &[a], &[])
}

/// `spl_token::instruction::initialize_mint2(mint, authority, None, decimals)`
pub fn initialize_mint2(mint: &AccountInfo, authority: &Pubkey, decimals: u8) -> R<()> {
    let mut d = [0u8; 35];
    d[0] = 20;
    d[1] = decimals;
    put(&mut d, 2, authority)?;
    call(&TOKEN_PROGRAM, &d, &[w(mint)], &[mint], &[])
}

// ------------------------------------------------------------------------------------------------ ATA

/// Associated Token Account `Create` (`idempotent = false`, data [0]) or `CreateIdempotent` ([1]).
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn create_ata(
    idempotent: bool,
    payer: &AccountInfo,
    ata: &AccountInfo,
    wallet: &AccountInfo,
    mint: &AccountInfo,
    system_program: &AccountInfo,
    token_program: &AccountInfo,
    signers: &[Signer],
) -> R<()> {
    call(
        &ATA_PROGRAM,
        &[idempotent as u8],
        &[ws(payer), w(ata), r(wallet), r(mint), r(system_program), r(token_program)],
        &[payer, ata, wallet, mint, system_program, token_program],
        signers,
    )
}

/// `associated_token::create` exactly as Anchor's `init_if_needed` issues it: the ATA's account meta is the
/// DERIVED address (spl's `create_associated_token_account` computes it), the account info is the passed one. When
/// they differ the runtime rejects the call with `MissingAccount` (the derived key is not among the passed infos),
/// which is what the Anchor program returns for a wrong system-owned account in that slot.
pub fn create_ata_derived(
    derived: &Pubkey,
    payer: &AccountInfo,
    ata: &AccountInfo,
    wallet: &AccountInfo,
    mint: &AccountInfo,
    system_program: &AccountInfo,
    token_program: &AccountInfo,
) -> R<()> {
    if derived == ata.key() {
        return create_ata(false, payer, ata, wallet, mint, system_program, token_program, &[]);
    }
    let metas = [ws(payer), AccountMeta::writable(derived), r(wallet), r(mint), r(system_program), r(token_program)];
    let accs = [payer, ata, wallet, mint, system_program, token_program].map(Account::from);
    // SAFETY: no account data of these accounts is borrowed (the program never takes RefCell-style borrows); the
    // call cannot succeed (see above), the runtime fails the transaction.
    unsafe { invoke_signed_unchecked(&Instruction { program_id: &ATA_PROGRAM, data: &[0], accounts: &metas }, &accs, &[]) };
    Err(pinocchio::program_error::ProgramError::MissingRequiredSignature) // unreachable: the runtime aborted the call
}

// ------------------------------------------------------------------------------------------------ Metaplex

/// Metaplex `CreateMetadataAccountV3` (instruction 33): immutable metadata, no creators, no royalties, no collection.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn create_metadata_v3(
    metadata_program: &AccountInfo,
    metadata: &AccountInfo,
    mint: &AccountInfo,
    authority: &AccountInfo,
    payer: &AccountInfo,
    system_program: &AccountInfo,
    name: &str,
    symbol: &str,
    uri: &str,
    signers: &[Signer],
) -> R<()> {
    // 1 + (4 + 32) + (4 + 10) + (4 + 200) + 2 + 5
    // (launch checked the lengths: name <= 32, symbol <= 10, uri <= 200 bytes)
    let mut d = Ev::raw(33);
    d.str(name).str(symbol).str(uri);
    // seller_fee_basis_points 0, creators None, collection None, uses None, is_mutable false, collection_details None
    d.put(&[0; 7]);
    call(
        metadata_program.key(),
        d.bytes()?,
        &[w(metadata), r(mint), rs(authority), ws(payer), rs(authority), r(system_program)],
        &[metadata, mint, authority, payer, authority, system_program],
        signers,
    )
}

// ------------------------------------------------------------------------------------------------ Raydium

const IX_CPMM_INITIALIZE: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const IX_LOCK_CP_LIQUIDITY: [u8; 8] = [216, 157, 29, 78, 38, 51, 31, 26];

/// Raydium CPMM `initialize(init_amount_0, init_amount_1, open_time = 0)`.
/// Accounts: creator, amm_config, authority, pool_state, token_0_mint, token_1_mint, lp_mint, creator_token_0,
/// creator_token_1, creator_lp_token, token_0_vault, token_1_vault, create_pool_fee, observation_state,
/// token_program, associated_token_program, system_program, rent, (cpmm program).
#[inline(never)]
pub fn cpmm_initialize(cpmm: &AccountInfo, a: [&AccountInfo; 18], amount_0: u64, amount_1: u64, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 32];
    put(&mut d, 0, &IX_CPMM_INITIALIZE)?;
    put(&mut d, 8, &amount_0.to_le_bytes())?;
    put(&mut d, 16, &amount_1.to_le_bytes())?;
    let tok = a[14];
    call(
        cpmm.key(),
        &d,
        &[
            ws(a[0]),
            r(a[1]),
            r(a[2]),
            ws(a[3]),
            r(a[4]),
            r(a[5]),
            w(a[6]),
            w(a[7]),
            w(a[8]),
            w(a[9]),
            w(a[10]),
            w(a[11]),
            w(a[12]),
            w(a[13]),
            r(tok),
            r(tok), // token_0_program: classic SPL for both mints
            r(tok), // token_1_program
            r(a[15]),
            r(a[16]),
            r(a[17]),
        ],
        &[a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11], a[12], a[13], tok, tok, tok, a[15], a[16], a[17]],
        signers,
    )
}

/// Raydium LP lock `lock_cp_liquidity(lp_amount, with_metadata = false)`.
/// Accounts: authority, payer, liquidity_owner, fee_nft_owner, fee_nft_mint, fee_nft_account, pool_state,
/// locked_liquidity, lp_mint, liquidity_owner_lp, locked_lp_vault, token_0_vault, token_1_vault, metadata_account,
/// rent, system_program, token_program, associated_token_program, metadata_program.
#[inline(never)]
pub fn lock_cp_liquidity(lock: &AccountInfo, a: [&AccountInfo; 19], lp_amount: u64, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 17];
    put(&mut d, 0, &IX_LOCK_CP_LIQUIDITY)?;
    put(&mut d, 8, &lp_amount.to_le_bytes())?;
    call(
        lock.key(),
        &d,
        &[
            r(a[0]),
            ws(a[1]),
            rs(a[2]),
            r(a[3]),
            ws(a[4]),
            w(a[5]),
            r(a[6]),
            w(a[7]),
            r(a[8]),
            w(a[9]),
            w(a[10]),
            w(a[11]),
            w(a[12]),
            w(a[13]),
            r(a[14]),
            r(a[15]),
            r(a[16]),
            r(a[17]),
            r(a[18]),
        ],
        &a,
        signers,
    )
}

const IX_COLLECT_CP_FEES: [u8; 8] = [8, 30, 51, 199, 209, 184, 247, 133];

/// Raydium LP lock `collect_cp_fees(fee_lp_amount)`.
/// Accounts: authority, fee_nft_owner, fee_nft_account, locked_liquidity, cpmm_program, cpmm_authority, pool_state,
/// lp_mint, recipient_token_0, recipient_token_1, token_0_vault, token_1_vault, vault_0_mint, vault_1_mint,
/// locked_lp_vault, token_program, token_program_2022, memo_program.
#[inline(never)]
pub fn collect_cp_fees(lock: &AccountInfo, a: [&AccountInfo; 18], fee_lp_amount: u64, signers: &[Signer]) -> R<()> {
    let mut d = [0u8; 16];
    put(&mut d, 0, &IX_COLLECT_CP_FEES)?;
    put(&mut d, 8, &fee_lp_amount.to_le_bytes())?;
    call(
        lock.key(),
        &d,
        &[
            r(a[0]),
            rs(a[1]),
            w(a[2]),
            w(a[3]),
            r(a[4]),
            r(a[5]),
            w(a[6]),
            w(a[7]),
            w(a[8]),
            w(a[9]),
            w(a[10]),
            w(a[11]),
            r(a[12]),
            r(a[13]),
            w(a[14]),
            r(a[15]),
            r(a[16]),
            r(a[17]),
        ],
        &a,
        signers,
    )
}

// ------------------------------------------------------------------------------------------------ events

/// Anchor `emit_cpi!`: a self-CPI whose data is EVENT_IX_TAG, the event discriminator and the Borsh fields, signed
/// by the event authority PDA.
#[inline(never)]
pub fn emit(event_authority: &AccountInfo, data: &[u8]) -> R<()> {
    let bump = [EVENT_AUTHORITY_BUMP];
    let seeds = [Seed::from(EVENT_AUTHORITY_SEED), Seed::from(&bump[..])];
    call(&ID, data, &[rs(event_authority)], &[event_authority], &[Signer::from(&seeds[..])])
}

/// Event data builder (the largest event, CurveMetadata, is 16 + 32 + 36 + 14 + 204 + 32 = 334 bytes).
/// Appending past the end is never cut short: the builder marks itself overflowed and `bytes` / `emit` then fail
/// with `BorshIoError`, so an over-long event or instruction is refused instead of being sent truncated.
pub struct Ev {
    b: [u8; EV_CAP],
    n: usize,
    over: bool,
}

pub const EV_CAP: usize = 336;

impl Ev {
    #[inline(never)]
    pub fn new(disc: &[u8; 8]) -> Self {
        let mut ev = Self { b: [0u8; EV_CAP], n: 0, over: false };
        ev.put(&0x1d9acb512ea545e4u64.to_le_bytes()).put(disc);
        ev
    }
    /// A plain buffer starting with one byte (the Metaplex instruction data).
    pub fn raw(first: u8) -> Self {
        let mut ev = Self { b: [0u8; EV_CAP], n: 0, over: false };
        ev.put(&[first]);
        ev
    }
    /// The bytes written so far; `BorshIoError` if any `put` did not fit.
    pub fn bytes(&self) -> R<&[u8]> {
        if self.over {
            return Err(ProgramError::BorshIoError);
        }
        // SAFETY: n <= EV_CAP (put only advances n by what fits)
        Ok(unsafe { self.b.get_unchecked(..self.n) })
    }
    /// Appends `s`, or, if it does not fit in what is left, writes nothing and marks the builder overflowed.
    #[inline(never)]
    pub fn put(&mut self, s: &[u8]) -> &mut Self {
        if self.over || s.len() > EV_CAP - self.n {
            self.over = true;
            return self;
        }
        // SAFETY: n + s.len() <= EV_CAP, checked above
        unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), self.b.as_mut_ptr().add(self.n), s.len()) };
        self.n += s.len();
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.put(&v.to_le_bytes())
    }
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.put(&v.to_le_bytes())
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.put(&v.to_le_bytes())
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.put(&v.to_le_bytes())
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.put(&[v as u8])
    }
    pub fn str(&mut self, s: &str) -> &mut Self {
        self.u32(s.len() as u32).put(s.as_bytes())
    }
    pub fn emit(&self, event_authority: &AccountInfo) -> R<()> {
        emit(event_authority, self.bytes()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_refuses_writes_past_the_end() {
        let mut d = [0u8; 12];
        assert!(put(&mut d, 4, &[1u8; 8]).is_ok());
        assert_eq!(&d[4..], &[1u8; 8]);
        assert_eq!(put(&mut d, 5, &[2u8; 8]), Err(ProgramError::BorshIoError));
        assert_eq!(put(&mut d, 13, &[]), Err(ProgramError::BorshIoError));
        assert_eq!(put(&mut d, usize::MAX, &[2u8; 8]), Err(ProgramError::BorshIoError));
        assert_eq!(&d[4..], &[1u8; 8], "a refused write changes nothing");
    }

    #[test]
    fn ev_fills_exactly_and_refuses_overflow() {
        // the largest event: CurveMetadata with the longest name, symbol and URI launch accepts = 334 bytes
        let mut ev = Ev::new(&[9; 8]);
        ev.put(&[1; 32]).str(&"n".repeat(32)).str(&"s".repeat(10)).str(&"u".repeat(200)).put(&[2; 32]);
        assert_eq!(ev.bytes().unwrap().len(), 334);
        // filling to exactly EV_CAP is fine
        ev.put(&[3, 4]);
        assert_eq!(ev.bytes().unwrap().len(), EV_CAP);
        // one more byte overflows: nothing is written and the builder fails from then on
        ev.put(&[5]);
        assert_eq!(ev.bytes(), Err(ProgramError::BorshIoError));
        ev.put(&[]);
        assert_eq!(ev.bytes(), Err(ProgramError::BorshIoError));
        // a single piece larger than the space left is refused whole, not cut
        let mut big = Ev::raw(33);
        big.put(&[0; EV_CAP]);
        assert_eq!(big.bytes(), Err(ProgramError::BorshIoError));
        assert_eq!(big.n, 1);
    }
}
