//! `initialize` (programs/launch_curve/src/instructions/admin.rs).

use crate::checks::*;
use crate::consts::{dex, *};
use crate::cpi::{self, Ev};
use crate::err::*;
use crate::state::{Cfg, CONFIG_LEN};
use pinocchio::{account_info::AccountInfo, instruction::Signer, log::sol_log, pubkey::Pubkey, sysvars::{rent::Rent, Sysvar}};

/// `InitializeParams`, kept as its 98 Borsh bytes (pauser, treasury, launch_fee, target, creator_share_bps,
/// pool_creator_share_bps, protection_duration, max_wallet_bps, migration_reserve).
pub struct InitParams<'a> {
    pub raw: &'a [u8],
}

// SAFETY (all getters): `raw` is exactly 98 bytes (Rd::take(98)).
impl InitParams<'_> {
    fn u64(&self, o: usize) -> u64 {
        unsafe { core::ptr::read_unaligned(self.raw.as_ptr().add(o) as *const u64) }
    }
    fn u16(&self, o: usize) -> u16 {
        unsafe { core::ptr::read_unaligned(self.raw.as_ptr().add(o) as *const u16) }
    }
    pub fn pauser(&self) -> &Pubkey {
        unsafe { &*(self.raw.as_ptr() as *const Pubkey) }
    }
    pub fn treasury(&self) -> &Pubkey {
        unsafe { &*(self.raw.as_ptr().add(32) as *const Pubkey) }
    }
    pub fn launch_fee(&self) -> u64 {
        self.u64(64)
    }
    pub fn target(&self) -> u64 {
        self.u64(72)
    }
    pub fn creator_share_bps(&self) -> u16 {
        self.u16(80)
    }
    pub fn pool_creator_share_bps(&self) -> u16 {
        self.u16(82)
    }
    pub fn protection_duration(&self) -> u32 {
        unsafe { core::ptr::read_unaligned(self.raw.as_ptr().add(84) as *const u32) }
    }
    pub fn max_wallet_bps(&self) -> u16 {
        self.u16(88)
    }
    pub fn migration_reserve(&self) -> u64 {
        self.u64(90)
    }
}

/// Anchor `init` of a PDA (`generate_create_account`): create_account when the address holds no lamports;
/// otherwise top up to rent (TryingToInitPayerAsProgramAccount if the payer is the account), allocate and assign.
/// Then the implied `mut` (ConstraintMut) and `rent_exempt = enforce` (ConstraintRentExempt) checks.
pub fn init_pda(payer: &AccountInfo, acc: &AccountInfo, space: usize, rent: &Rent, signer: Signer) -> R<()> {
    let current = acc.lamports();
    if current == 0 {
        cpi::create_account(payer, acc, rent.minimum_balance(space), space as u64, &ID, &[signer])?;
    } else {
        if payer.key() == acc.key() {
            return Err(e(TRYING_TO_INIT_PAYER_AS_PROGRAM_ACCOUNT));
        }
        let required = rent.minimum_balance(space).max(1).saturating_sub(current);
        if required > 0 {
            cpi::transfer(payer, acc, required, &[])?;
        }
        cpi::allocate(acc, space as u64, &[signer.clone()])?;
        cpi::assign(acc, &ID, &[signer])?;
    }
    // Account::try_from_unchecked after the creation: owned by this program now (the System program just assigned it)
    mutable(acc)?;
    if !rent.is_exempt(acc.lamports(), acc.data_len()) {
        return Err(e(CONSTRAINT_RENT_EXEMPT));
    }
    Ok(())
}

/// `UpgradeableLoaderState::Program { programdata_address }`: u32 tag 2, then the address.
fn programdata_address(program: &AccountInfo) -> Option<&Pubkey> {
    let p = program.data_ptr();
    // SAFETY: reads stay within the first 36 bytes, checked first
    if program.owner() != &BPF_LOADER_UPGRADEABLE || program.data_len() < 36 || unsafe { core::ptr::read_unaligned(p as *const u32) } != 2 {
        return None;
    }
    Some(unsafe { &*(p.add(4) as *const Pubkey) })
}

/// `UpgradeableLoaderState::ProgramData { slot, upgrade_authority_address: Some(..) }`.
fn upgrade_authority(program_data: &AccountInfo) -> Option<&Pubkey> {
    let p = program_data.data_ptr();
    // SAFETY: reads stay within the first 45 bytes, checked first
    if program_data.data_len() < 45 || unsafe { core::ptr::read_unaligned(p as *const u32) } != 3 || unsafe { *p.add(12) } != 1 {
        return None;
    }
    Some(unsafe { &*(p.add(13) as *const Pubkey) })
}

pub fn initialize(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: Initialize");
    let p = InitParams { raw: Rd::new(data).take(98)? };

    let mut c = Cursor::new(accounts);
    let authority = c.signer()?;
    let config = c.any()?; // init
    let launch_program = c.program(&ID)?;
    let program_data = c.any()?;
    let amm_config = c.any()?;
    let cpmm_program = c.any()?;
    let lock_program = c.any()?;
    let _system_program = c.program(&SYSTEM_PROGRAM)?;
    let event_auth = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)

    // init config: seeds = [CONFIG_SEED], bump; payer = authority
    let rent = Rent::get()?;
    let bump = seeds_find_1(config, CONFIG_SEED)?;
    let b = [bump];
    let seeds = [pinocchio::instruction::Seed::from(CONFIG_SEED), pinocchio::instruction::Seed::from(&b[..])];
    init_pda(authority, config, CONFIG_LEN, &rent, Signer::from(&seeds[..]))?;

    mutable(authority)?;
    if programdata_address(launch_program) != Some(program_data.key()) {
        return Err(e(ACCOUNT_MISMATCH));
    }
    if upgrade_authority(program_data) != Some(authority.key()) {
        return Err(e(NOT_UPGRADE_AUTHORITY));
    }
    same(program_data.owner(), &BPF_LOADER_UPGRADEABLE, ACCOUNT_MISMATCH)?;
    same(amm_config.owner(), &dex::CPMM_PROGRAM, WRONG_CLUSTER)?;
    same(amm_config.key(), &dex::CPMM_AMM_CONFIG, WRONG_CLUSTER)?;
    for (prog, id) in [(cpmm_program, &dex::CPMM_PROGRAM), (lock_program, &dex::LOCK_PROGRAM)] {
        if !prog.executable() {
            return Err(e(WRONG_CLUSTER));
        }
        same(prog.key(), id, WRONG_CLUSTER)?;
    }
    event_authority(event_auth)?;

    let ok = p.target() >= MIN_TARGET
        && p.target() <= MAX_TARGET
        && p.migration_reserve() >= MIN_MIGRATION_RESERVE
        && p.migration_reserve() <= MAX_MIGRATION_RESERVE
        && p.migration_reserve().checked_mul(2).map_or(false, |r| r < p.target())
        && p.creator_share_bps() as u64 <= BPS
        && p.pool_creator_share_bps() as u64 <= BPS
        && p.protection_duration() <= MAX_PROTECTION_DURATION
        && p.max_wallet_bps() >= 1
        && p.max_wallet_bps() as u64 <= BPS
        && p.treasury() != &[0u8; 32];
    if !ok {
        return Err(e(INVALID_PARAMETER));
    }
    Cfg(config.data_ptr()).init(bump, &p);

    Ev::new(&ev::CONFIG_INITIALIZED)
        .put(p.treasury())
        .put(p.pauser())
        .u64(p.launch_fee())
        .u64(p.target())
        .u16(p.creator_share_bps())
        .u16(p.pool_creator_share_bps())
        .u32(p.protection_duration())
        .u16(p.max_wallet_bps())
        .u64(p.migration_reserve())
        .emit(event_auth)
}

/// `seeds = [seed], bump` (one seed) for an init account.
fn seeds_find_1(a: &AccountInfo, seed: &[u8]) -> R<u8> {
    let (k, b) = pinocchio::pubkey::find_program_address(&[seed], &ID);
    same(&k, a.key(), CONSTRAINT_SEEDS)?;
    Ok(b)
}


/// The accounts of `PauserOnly` (pauser signer, config with `has_one = pauser @ NotPauser`).
fn pauser_only(accounts: &[AccountInfo]) -> R<(&AccountInfo, Cfg, &AccountInfo)> {
    let mut c = Cursor::new(accounts);
    let pauser = c.signer()?;
    let (config, cfg) = c.config()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)
    config_seeds(config)?;
    mutable(config)?;
    same(cfg.pauser(), pauser.key(), NOT_PAUSER)?;
    event_authority(ea)?;
    Ok((pauser, cfg, ea))
}

fn pauser_transferred(ea: &AccountInfo, previous: &Pubkey, pauser: &Pubkey) -> R<()> {
    Ev::new(&ev::PAUSER_TRANSFERRED).put(previous).put(pauser).emit(ea)
}

/// Stops (or restarts) new launches and buys. Never sells, graduations, fee collection or claims.
pub fn set_paused(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: SetPaused");
    let paused = Rd::new(data).bool()?;
    let (pauser, cfg, ea) = pauser_only(accounts)?;
    cfg.set_paused(paused);
    Ev::new(&ev::PAUSED).bool(paused).put(pauser.key()).emit(ea)
}

/// Step 1 of handing the pause switch on: the pauser names the next pauser (the default key cancels a proposal).
pub fn propose_pauser(accounts: &[AccountInfo], data: &[u8]) -> R<()> {
    sol_log("Instruction: ProposePauser");
    let new_pauser = *Rd::new(data).key()?;
    let (_, cfg, ea) = pauser_only(accounts)?;
    cfg.set_pending_pauser(&new_pauser);
    Ev::new(&ev::PAUSER_PROPOSED).put(cfg.pauser()).put(&new_pauser).emit(ea)
}

/// Removes the pause switch for good (no pauser can ever be set again).
pub fn renounce_pauser(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: RenouncePauser");
    let (_, cfg, ea) = pauser_only(accounts)?;
    let previous = *cfg.pauser();
    cfg.set_pauser(&[0u8; 32]);
    cfg.set_pending_pauser(&[0u8; 32]);
    pauser_transferred(ea, &previous, &[0u8; 32])
}

/// Step 2: the proposed pauser accepts.
pub fn accept_pauser(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: AcceptPauser");
    let mut c = Cursor::new(accounts);
    let new_pauser = c.signer()?;
    let (config, cfg) = c.config()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)
    config_seeds(config)?;
    mutable(config)?;
    event_authority(ea)?;

    let pending = *cfg.pending_pauser();
    if pending == [0u8; 32] {
        return Err(e(NO_PROPOSAL));
    }
    if &pending != new_pauser.key() {
        return Err(e(NOT_PROPOSED_PAUSER));
    }
    let previous = *cfg.pauser();
    cfg.set_pauser(&pending);
    cfg.set_pending_pauser(&[0u8; 32]);
    pauser_transferred(ea, &previous, &pending)
}

/// Pays the accrued treasury curve fees to the treasury. Anyone may call it.
pub fn claim_treasury_fees(accounts: &[AccountInfo], _data: &[u8]) -> R<()> {
    sol_log("Instruction: ClaimTreasuryFees");
    let mut c = Cursor::new(accounts);
    let caller = c.signer()?;
    let (config, cfg) = c.config()?;
    let treasury = c.any()?;
    let ea = c.any()?;
    let _program = c.any()?; // #[event_cpi] `program`: unchecked (a wrong one fails at the event self-CPI)
    config_seeds(config)?;
    mutable(config)?;
    same(cfg.treasury(), treasury.key(), CONSTRAINT_HAS_ONE)?;
    mutable(treasury)?;
    event_authority(ea)?;

    let amount = cfg.treasury_fees();
    if amount == 0 {
        return Err(e(NOTHING_TO_CLAIM));
    }
    cfg.set_treasury_fees(0);
    crate::ix::fees::move_lamports(config, treasury, amount)?;
    Ev::new(&ev::TREASURY_FEES_CLAIMED).put(caller.key()).u64(amount).emit(ea)
}
