//! Zero-copy views of the two program accounts, byte for byte the Borsh layout Anchor's `#[account]` gives
//! `Config` and `Curve` (8-byte discriminator, then the fields packed in declaration order, no padding), and of SPL
//! token accounts. A view is only made after the loader in checks.rs verified owner, discriminator and length.

use pinocchio::pubkey::Pubkey;

#[inline(always)]
fn rd64(p: *const u8, o: usize) -> u64 {
    unsafe { core::ptr::read_unaligned(p.add(o) as *const u64) }
}
#[inline(always)]
fn wr64(p: *mut u8, o: usize, v: u64) {
    unsafe { core::ptr::write_unaligned(p.add(o) as *mut u64, v) }
}
#[inline(always)]
fn rd16(p: *const u8, o: usize) -> u16 {
    unsafe { core::ptr::read_unaligned(p.add(o) as *const u16) }
}
#[inline(always)]
fn key<'a>(p: *const u8, o: usize) -> &'a Pubkey {
    unsafe { &*(p.add(o) as *const Pubkey) }
}
#[inline(always)]
fn wkey(p: *mut u8, o: usize, k: &Pubkey) {
    unsafe { core::ptr::copy_nonoverlapping(k.as_ptr(), p.add(o), 32) }
}

/// `Config`: 8 + 196 bytes.
pub const CONFIG_LEN: usize = 204;
/// `Curve`: 8 + 356 bytes.
pub const CURVE_LEN: usize = 364;

#[derive(Clone, Copy)]
pub struct Cfg(pub *mut u8);

#[allow(dead_code)]
impl Cfg {
    pub fn bump(&self) -> u8 {
        unsafe { *self.0.add(8) }
    }
    pub fn pauser(&self) -> &Pubkey {
        key(self.0, 9)
    }
    pub fn treasury(&self) -> &Pubkey {
        key(self.0, 41)
    }
    pub fn launch_fee(&self) -> u64 {
        rd64(self.0, 73)
    }
    pub fn target(&self) -> u64 {
        rd64(self.0, 81)
    }
    pub fn creator_share_bps(&self) -> u16 {
        rd16(self.0, 89)
    }
    pub fn pool_creator_share_bps(&self) -> u16 {
        rd16(self.0, 91)
    }
    pub fn protection_duration(&self) -> u32 {
        unsafe { core::ptr::read_unaligned(self.0.add(93) as *const u32) }
    }
    pub fn max_wallet_bps(&self) -> u16 {
        rd16(self.0, 97)
    }
    pub fn migration_reserve(&self) -> u64 {
        rd64(self.0, 99)
    }
    pub fn paused(&self) -> bool {
        unsafe { *self.0.add(107) != 0 }
    }
    pub fn launch_count(&self) -> u64 {
        rd64(self.0, 108)
    }
    pub fn set_launch_count(&self, v: u64) {
        wr64(self.0, 108, v)
    }
    pub fn total_reserves(&self) -> u64 {
        rd64(self.0, 116)
    }
    pub fn set_total_reserves(&self, v: u64) {
        wr64(self.0, 116, v)
    }
    pub fn total_creator_fees(&self) -> u64 {
        rd64(self.0, 124)
    }
    pub fn set_total_creator_fees(&self, v: u64) {
        wr64(self.0, 124, v)
    }
    pub fn treasury_fees(&self) -> u64 {
        rd64(self.0, 132)
    }
    pub fn set_treasury_fees(&self, v: u64) {
        wr64(self.0, 132, v)
    }
    pub fn pending_pauser(&self) -> &Pubkey {
        key(self.0, 140)
    }
    pub fn set_paused(&self, v: bool) {
        unsafe { *self.0.add(107) = v as u8 }
    }
    pub fn set_pauser(&self, k: &Pubkey) {
        wkey(self.0, 9, k)
    }
    pub fn set_pending_pauser(&self, k: &Pubkey) {
        wkey(self.0, 140, k)
    }

    /// Writes the discriminator and every parameter (initialize). The running totals stay zero.
    #[allow(clippy::too_many_arguments)]
    pub fn init(&self, bump: u8, p: &crate::ix::admin::InitParams) {
        let d = self.0;
        unsafe {
            core::ptr::copy_nonoverlapping(crate::consts::CONFIG_DISC.as_ptr(), d, 8);
            *d.add(8) = bump;
            core::ptr::copy_nonoverlapping(p.raw.as_ptr(), d.add(9), 64); // pauser, treasury
            core::ptr::copy_nonoverlapping(p.raw.as_ptr().add(64), d.add(73), 34); // launch_fee .. migration_reserve
            *d.add(107) = 0; // paused
        }
        wkey(d, 140, &[0u8; 32]); // pending_pauser
    }
}

#[derive(Clone, Copy)]
pub struct Crv(pub *mut u8);

#[allow(dead_code)]
impl Crv {
    pub fn bump(&self) -> u8 {
        unsafe { *self.0.add(8) }
    }
    pub fn vault_bump(&self) -> u8 {
        unsafe { *self.0.add(9) }
    }
    pub fn mint(&self) -> &Pubkey {
        key(self.0, 10)
    }
    pub fn creator(&self) -> &Pubkey {
        key(self.0, 42)
    }
    pub fn fee_recipient(&self) -> &Pubkey {
        key(self.0, 74)
    }
    pub fn pending_fee_recipient(&self) -> &Pubkey {
        key(self.0, 106)
    }
    pub fn set_fee_recipient(&self, k: &Pubkey) {
        wkey(self.0, 74, k)
    }
    pub fn set_pending_fee_recipient(&self, k: &Pubkey) {
        wkey(self.0, 106, k)
    }
    pub fn protection_ends_at(&self) -> i64 {
        rd64(self.0, 146) as i64
    }
    pub fn max_wallet(&self) -> u64 {
        rd64(self.0, 154)
    }
    pub fn launch_index(&self) -> u64 {
        rd64(self.0, 162)
    }
    pub fn raised(&self) -> u64 {
        rd64(self.0, 170)
    }
    pub fn set_raised(&self, v: u64) {
        wr64(self.0, 170, v)
    }
    pub fn sold(&self) -> u64 {
        rd64(self.0, 178)
    }
    pub fn set_sold(&self, v: u64) {
        wr64(self.0, 178, v)
    }
    pub fn creator_fees(&self) -> u64 {
        rd64(self.0, 186)
    }
    pub fn set_creator_fees(&self, v: u64) {
        wr64(self.0, 186, v)
    }
    pub fn graduated(&self) -> bool {
        unsafe { *self.0.add(194) != 0 }
    }
    pub fn set_graduated(&self) {
        unsafe { *self.0.add(194) = 1 }
    }
    pub fn liquidity_locked(&self) -> bool {
        unsafe { *self.0.add(195) != 0 }
    }
    pub fn set_liquidity_locked(&self) {
        unsafe { *self.0.add(195) = 1 }
    }
    pub fn pool(&self) -> &Pubkey {
        key(self.0, 196)
    }
    pub fn lp_mint(&self) -> &Pubkey {
        key(self.0, 228)
    }
    pub fn set_lp_mint(&self, k: &Pubkey) {
        wkey(self.0, 228, k)
    }
    pub fn fee_nft_mint(&self) -> &Pubkey {
        key(self.0, 260)
    }
    pub fn set_fee_nft_mint(&self, k: &Pubkey) {
        wkey(self.0, 260, k)
    }
    pub fn set_lp_amount(&self, v: u64) {
        wr64(self.0, 292, v)
    }

    /// Records a new launch (the fields `launch` sets; everything else stays zero).
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        &self,
        bump: u8,
        vault_bump: u8,
        mint: &Pubkey,
        creator: &Pubkey,
        now: i64,
        protection_ends_at: i64,
        max_wallet: u64,
        launch_index: u64,
        pool: &Pubkey,
    ) {
        let d = self.0;
        unsafe {
            core::ptr::copy_nonoverlapping(crate::consts::CURVE_DISC.as_ptr(), d, 8);
            *d.add(8) = bump;
            *d.add(9) = vault_bump;
        }
        wkey(d, 10, mint);
        wkey(d, 42, creator);
        wkey(d, 74, creator); // fee_recipient
        wkey(d, 106, &[0u8; 32]); // pending_fee_recipient
        wr64(d, 138, now as u64);
        wr64(d, 146, protection_ends_at as u64);
        wr64(d, 154, max_wallet);
        wr64(d, 162, launch_index);
        wkey(d, 196, pool);
    }
}

/// An SPL token account (165 bytes; checked by checks::token_layout).
#[derive(Clone, Copy)]
pub struct Tok(pub *const u8);

impl Tok {
    pub fn mint(&self) -> &Pubkey {
        key(self.0, 0)
    }
    pub fn owner(&self) -> &Pubkey {
        key(self.0, 32)
    }
    pub fn amount(&self) -> u64 {
        rd64(self.0, 64)
    }
}
