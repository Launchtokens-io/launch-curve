//! Addresses, seeds, discriminators and the fixed numbers of the curve. Everything here is the same value the Anchor
//! program (programs/launch_curve) uses; the PDAs and bumps that Anchor derived at run time are precomputed for this
//! program id (sdk/test/lean.test.ts checks them against the SDK's derivations).

use pinocchio::pubkey::Pubkey;
use pinocchio_pubkey::pubkey;

/// The lean program's own id, the same in the devnet and the mainnet build (devnet since 2026-10-15; the Anchor
/// program stays at `E7GsgE4M6Kq6Zppk3P126YBvBc7E3FrBG3HpefgSLL38` on devnet). A different mainnet id means changing
/// these three constants and rebuilding (sdk/test/lean.test.ts checks them against the SDK).
pub const ID: Pubkey = pubkey!("7AxEAL5aCUMiPN12Q7qx2ZLS7QWuVG8pqWNyKXj7qJAv");
/// PDA ["config"] of ID, bump 255 (Anchor: `seeds = [CONFIG_SEED], bump = config.bump`)
pub const CONFIG_PDA: Pubkey = pubkey!("7QvoBjcSsSdEUd2siGWxZtAHyB34q7e1znS9fxkw64D1");
/// PDA ["__event_authority"] of ID (Anchor `#[event_cpi]`)
pub const EVENT_AUTHORITY: Pubkey = pubkey!("CsK8EdxrnKmthzrXb8QV8ZKdFg41NxhXA3YrNg1GsXDe");
pub const EVENT_AUTHORITY_BUMP: u8 = 248;

pub const SYSTEM_PROGRAM: Pubkey = [0u8; 32];
pub const TOKEN_PROGRAM: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ATA_PROGRAM: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const METADATA_PROGRAM: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
pub const TOKEN_2022_PROGRAM: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const MEMO_PROGRAM: Pubkey = pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
pub const WSOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
pub const RENT_SYSVAR: Pubkey = pubkey!("SysvarRent111111111111111111111111111111111");
pub const BPF_LOADER_UPGRADEABLE: Pubkey = pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");

#[cfg(feature = "devnet")]
pub mod dex {
    use super::*;
    pub const CPMM_PROGRAM: Pubkey = pubkey!("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb");
    pub const CPMM_AUTHORITY: Pubkey = pubkey!("CXniRufdq5xL8t8jZAPxsPZDpuudwuJSPWnbcD5Y5Nxq");
    pub const CPMM_AMM_CONFIG: Pubkey = pubkey!("EsTevfacYXpuho5VBuzBjDZi8dtWidGnXoSYAr8krTvz");
    pub const CPMM_CREATE_POOL_FEE: Pubkey = pubkey!("3oE58BKVt8KuYkGxx8zBojugnymWmBiyafWgMrnb6eYy");
    pub const LOCK_PROGRAM: Pubkey = pubkey!("DRay25Usp3YJAi7beckgpGUC7mGJ2cR1AVPxhYfwVCUX");
    pub const LOCK_AUTHORITY: Pubkey = pubkey!("7qWVV8UY2bRJfDLP4s37YzBPKUkVB46DStYJBpYbQzu3");
}

#[cfg(not(feature = "devnet"))]
pub mod dex {
    use super::*;
    pub const CPMM_PROGRAM: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
    pub const CPMM_AUTHORITY: Pubkey = pubkey!("GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL");
    pub const CPMM_AMM_CONFIG: Pubkey = pubkey!("G95xxie3XbkCqtE39GgQ9Ggc7xBC8Uceve7HFDEFApkc");
    pub const CPMM_CREATE_POOL_FEE: Pubkey = pubkey!("DNXgeM9EiiaAbaWvwjHj9fQQLAX5ZsfHyvmYUNRAdNC8");
    pub const LOCK_PROGRAM: Pubkey = pubkey!("LockrWmn6K5twhz3y9w1dQERbmgSaRkfnTeTKbpofwE");
    pub const LOCK_AUTHORITY: Pubkey = pubkey!("3f7GcQFG397GAaEnv51zR6tsTVihYRydnydDD1cXekxH");
}

pub const CONFIG_SEED: &[u8] = b"config";
pub const CURVE_SEED: &[u8] = b"curve";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const POOL_SEED: &[u8] = b"pool";
pub const FEE_NFT_SEED: &[u8] = b"fee_nft";
pub const EVENT_AUTHORITY_SEED: &[u8] = b"__event_authority";

pub const DECIMALS: u8 = 6;
pub const SUPPLY: u64 = 1_000_000_000_000_000;
pub const CURVE_TOKENS: u64 = 800_000_000_000_000;
pub const POOL_TOKENS: u64 = 200_000_000_000_000;
pub const BPS: u64 = 10_000;
pub const FEE_BPS: u64 = 100;
pub const MAX_NAME_BYTES: usize = 32;
pub const MAX_SYMBOL_BYTES: usize = 10;
pub const MAX_URI_BYTES: usize = 200;
pub const MAX_PROTECTION_DURATION: u32 = 3_600;
pub const MIN_TARGET: u64 = 10_000_000;
pub const MAX_TARGET: u64 = 100_000_000_000_000;
pub const MIN_MIGRATION_RESERVE: u64 = 200_000_000;
pub const MAX_MIGRATION_RESERVE: u64 = 1_000_000_000;

const fn d(b: [u8; 8]) -> u64 {
    u64::from_le_bytes(b)
}

/// Instruction discriminators (sha256("global:<name>")[..8], as in solana/idl/launch_curve.json), read as LE u64.
pub mod ix {
    use super::d;
    pub const INITIALIZE: u64 = d([175, 175, 109, 31, 13, 152, 155, 237]);
    pub const LAUNCH: u64 = d([153, 241, 93, 225, 22, 69, 74, 61]);
    pub const BUY: u64 = d([102, 6, 61, 18, 1, 218, 235, 234]);
    pub const SELL: u64 = d([51, 230, 133, 164, 1, 127, 131, 173]);
    pub const GRADUATE: u64 = d([45, 235, 225, 181, 17, 218, 64, 130]);
    pub const LOCK_LIQUIDITY: u64 = d([179, 201, 236, 158, 212, 98, 70, 182]);
    pub const COLLECT_POOL_FEES: u64 = d([145, 113, 45, 27, 77, 181, 241, 69]);
    pub const CLAIM_CREATOR_FEES: u64 = d([0, 23, 125, 234, 156, 118, 134, 89]);
    pub const CLAIM_TREASURY_FEES: u64 = d([79, 251, 63, 99, 240, 215, 0, 252]);
    pub const PROPOSE_FEE_RECIPIENT: u64 = d([74, 125, 125, 217, 237, 152, 21, 243]);
    pub const CANCEL_FEE_RECIPIENT_PROPOSAL: u64 = d([24, 32, 132, 236, 192, 111, 205, 165]);
    pub const ACCEPT_FEE_RECIPIENT: u64 = d([131, 4, 82, 195, 129, 239, 100, 44]);
    pub const SET_PAUSED: u64 = d([91, 60, 125, 192, 176, 225, 166, 218]);
    pub const PROPOSE_PAUSER: u64 = d([71, 207, 229, 8, 217, 251, 234, 163]);
    pub const ACCEPT_PAUSER: u64 = d([224, 60, 244, 96, 126, 18, 162, 64]);
    pub const RENOUNCE_PAUSER: u64 = d([100, 203, 169, 255, 58, 121, 48, 237]);
    pub const QUOTE_BUY: u64 = d([83, 9, 231, 110, 146, 31, 40, 12]);
    pub const QUOTE_SELL: u64 = d([5, 178, 49, 206, 140, 231, 131, 145]);
    /// anchor_lang::idl::IDL_IX_TAG (no in-program IDL: IdlInstructionStub)
    pub const IDL_TAG: u64 = 0x0a69e9a778bcf440;
    /// anchor_lang::event::EVENT_IX_TAG (the event self-CPI)
    pub const EVENT_TAG: u64 = 0x1d9acb512ea545e4;
}

/// Account discriminators (sha256("account:<Name>")[..8]).
pub const CONFIG_DISC: [u8; 8] = [155, 12, 170, 224, 30, 250, 204, 130];
pub const CURVE_DISC: [u8; 8] = [191, 180, 249, 66, 180, 71, 51, 182];

/// Event discriminators (sha256("event:<Name>")[..8]).
pub mod ev {
    pub const CONFIG_INITIALIZED: [u8; 8] = [181, 49, 200, 156, 19, 167, 178, 91];
    pub const CURVE_LAUNCHED: [u8; 8] = [202, 20, 128, 231, 162, 180, 61, 53];
    pub const CURVE_METADATA: [u8; 8] = [44, 0, 106, 190, 230, 217, 169, 51];
    pub const TRADE: [u8; 8] = [24, 254, 218, 152, 253, 43, 18, 81];
    pub const GRADUATED: [u8; 8] = [51, 241, 66, 50, 140, 245, 156, 192];
    pub const LIQUIDITY_LOCKED: [u8; 8] = [150, 201, 204, 183, 217, 13, 119, 185];
    pub const POOL_FEES_COLLECTED: [u8; 8] = [218, 74, 163, 42, 8, 152, 57, 242];
    pub const CREATOR_FEES_CLAIMED: [u8; 8] = [189, 178, 21, 181, 171, 179, 131, 1];
    pub const TREASURY_FEES_CLAIMED: [u8; 8] = [231, 48, 176, 247, 137, 1, 6, 171];
    pub const FEE_RECIPIENT_PROPOSED: [u8; 8] = [39, 153, 121, 120, 156, 94, 78, 224];
    pub const FEE_RECIPIENT_CHANGED: [u8; 8] = [219, 2, 90, 130, 185, 40, 160, 57];
    pub const PAUSED: [u8; 8] = [172, 248, 5, 253, 49, 255, 255, 232];
    pub const PAUSER_PROPOSED: [u8; 8] = [221, 208, 184, 251, 97, 218, 206, 119];
    pub const PAUSER_TRANSFERRED: [u8; 8] = [74, 9, 162, 58, 61, 63, 124, 194];
}
