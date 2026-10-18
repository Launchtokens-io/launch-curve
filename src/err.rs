//! Error numbers: the program's own (6000+, same order as launch_curve's `LaunchError`) and the Anchor built-ins the
//! Anchor program returns for the same failed checks. Every error is logged like Anchor logs it
//! ("AnchorError occurred. Error Code: <Name>. Error Number: <n>. Error Message: <msg>."), so clients that read
//! "Error Code: X" from the logs (src/solana/errors.ts, tests/launch_curve.ts) see the same names.

use pinocchio::program_error::ProgramError;

#[inline(always)]
pub const fn e(code: u32) -> ProgramError {
    ProgramError::Custom(code)
}

// launch_curve's own errors (LaunchError, 6000 + index)
pub const NOT_UPGRADE_AUTHORITY: u32 = 6000;
pub const INVALID_PARAMETER: u32 = 6001;
pub const LAUNCHES_PAUSED: u32 = 6002;
pub const NOT_PAUSER: u32 = 6003;
pub const INVALID_NAME: u32 = 6004;
pub const INVALID_SYMBOL: u32 = 6005;
pub const METADATA_TOO_LONG: u32 = 6006;
pub const ALREADY_GRADUATED: u32 = 6007;
pub const CURVE_COMPLETE: u32 = 6008;
pub const CURVE_NOT_COMPLETE: u32 = 6009;
pub const NOT_GRADUATED: u32 = 6010;
pub const ALREADY_LOCKED: u32 = 6011;
pub const NOT_LOCKED: u32 = 6012;
pub const EXPIRED: u32 = 6013;
pub const ZERO_AMOUNT: u32 = 6014;
pub const NOTHING_BOUGHT: u32 = 6015;
pub const SLIPPAGE_EXCEEDED: u32 = 6016;
pub const EXCEEDS_SOLD: u32 = 6017;
pub const NOTHING_TO_CLAIM: u32 = 6018;
pub const LAUNCH_BUY_WOULD_COMPLETE: u32 = 6019;
pub const HOLDING_CAP_EXCEEDED: u32 = 6020;
pub const NOT_FEE_RECIPIENT: u32 = 6021;
pub const NOT_PROPOSED_RECIPIENT: u32 = 6022;
pub const NO_PROPOSAL: u32 = 6023;
pub const ACCOUNT_MISMATCH: u32 = 6024;
pub const NOT_PROPOSED_PAUSER: u32 = 6025;
pub const WRONG_CLUSTER: u32 = 6026;
pub const MATH_OVERFLOW: u32 = 6027;

// Anchor built-ins (anchor_lang::error::ErrorCode)
pub const INSTRUCTION_FALLBACK_NOT_FOUND: u32 = 101;
pub const INSTRUCTION_DID_NOT_DESERIALIZE: u32 = 102;
pub const IDL_INSTRUCTION_STUB: u32 = 1000;
pub const CONSTRAINT_MUT: u32 = 2000;
pub const CONSTRAINT_HAS_ONE: u32 = 2001;
pub const CONSTRAINT_SIGNER: u32 = 2002;
pub const CONSTRAINT_RENT_EXEMPT: u32 = 2005;
pub const CONSTRAINT_SEEDS: u32 = 2006;
pub const CONSTRAINT_ASSOCIATED: u32 = 2009;
pub const CONSTRAINT_ADDRESS: u32 = 2012;
pub const CONSTRAINT_TOKEN_MINT: u32 = 2014;
pub const CONSTRAINT_TOKEN_OWNER: u32 = 2015;
pub const CONSTRAINT_ASSOCIATED_TOKEN_TOKEN_PROGRAM: u32 = 2023;
pub const ACCOUNT_DISCRIMINATOR_NOT_FOUND: u32 = 3001;
pub const ACCOUNT_DISCRIMINATOR_MISMATCH: u32 = 3002;
pub const ACCOUNT_DID_NOT_DESERIALIZE: u32 = 3003;
pub const ACCOUNT_NOT_ENOUGH_KEYS: u32 = 3005;
pub const ACCOUNT_OWNED_BY_WRONG_PROGRAM: u32 = 3007;
pub const INVALID_PROGRAM_ID: u32 = 3008;
pub const INVALID_PROGRAM_EXECUTABLE: u32 = 3009;
pub const ACCOUNT_NOT_SIGNER: u32 = 3010;
pub const ACCOUNT_NOT_SYSTEM_OWNED: u32 = 3011;
pub const ACCOUNT_NOT_INITIALIZED: u32 = 3012;
pub const ACCOUNT_NOT_ASSOCIATED_TOKEN_ACCOUNT: u32 = 3014;
pub const ACCOUNT_SYSVAR_MISMATCH: u32 = 3015;
pub const DECLARED_PROGRAM_ID_MISMATCH: u32 = 4100;
pub const TRYING_TO_INIT_PAYER_AS_PROGRAM_ACCOUNT: u32 = 4101;

/// One "\n<number>|<Name>|<message>" row per error: the IDL's errors and Anchor's messages for the built-ins (a
/// single string, so the table costs no relocations).
static TABLE: &str = concat!(
    "\n6000|NotUpgradeAuthority|only the program's upgrade authority can initialize",
    "\n6001|InvalidParameter|a parameter is out of bounds",
    "\n6002|LaunchesPaused|new launches and buys are paused",
    "\n6003|NotPauser|signer is not the pauser",
    "\n6004|InvalidName|name must be 1-32 bytes",
    "\n6005|InvalidSymbol|symbol must be 1-10 bytes",
    "\n6006|MetadataTooLong|metadata URI is longer than 200 bytes",
    "\n6007|AlreadyGraduated|the curve has graduated",
    "\n6008|CurveComplete|the curve is complete and waits for graduation",
    "\n6009|CurveNotComplete|the curve is not complete yet",
    "\n6010|NotGraduated|the curve has not graduated yet",
    "\n6011|AlreadyLocked|the liquidity is already locked",
    "\n6012|NotLocked|the liquidity is not locked yet",
    "\n6013|Expired|transaction deadline passed",
    "\n6014|ZeroAmount|amount is zero",
    "\n6015|NothingBought|the buy gets no tokens",
    "\n6016|SlippageExceeded|slippage limit exceeded",
    "\n6017|ExceedsSold|more tokens than the curve has sold",
    "\n6018|NothingToClaim|nothing to claim",
    "\n6019|LaunchBuyWouldComplete|the launch buy would complete the curve",
    "\n6020|HoldingCapExceeded|wallet would hold more than the launch-protection cap",
    "\n6021|NotFeeRecipient|signer is not the fee recipient",
    "\n6022|NotProposedRecipient|signer is not the proposed fee recipient",
    "\n6023|NoProposal|no fee recipient proposal",
    "\n6024|AccountMismatch|account does not match the expected address",
    "\n6025|NotProposedPauser|signer is not the proposed pauser",
    "\n6026|WrongCluster|a DEX account is not the expected program or config for this build's cluster",
    "\n6027|MathOverflow|math overflow",
    "\n101|InstructionFallbackNotFound|Fallback functions are not supported",
    "\n102|InstructionDidNotDeserialize|The program could not deserialize the given instruction",
    "\n1000|IdlInstructionStub|The program was compiled without idl instructions",
    "\n2000|ConstraintMut|A mut constraint was violated",
    "\n2001|ConstraintHasOne|A has one constraint was violated",
    "\n2002|ConstraintSigner|A signer constraint was violated",
    "\n2005|ConstraintRentExempt|A rent exemption constraint was violated",
    "\n2006|ConstraintSeeds|A seeds constraint was violated",
    "\n2009|ConstraintAssociated|An associated constraint was violated",
    "\n2012|ConstraintAddress|An address constraint was violated",
    "\n2014|ConstraintTokenMint|A token mint constraint was violated",
    "\n2015|ConstraintTokenOwner|A token owner constraint was violated",
    "\n2023|ConstraintAssociatedTokenTokenProgram|An associated token account token program constraint was violated",
    "\n3001|AccountDiscriminatorNotFound|No discriminator was found on the account",
    "\n3002|AccountDiscriminatorMismatch|Account discriminator did not match what was expected",
    "\n3003|AccountDidNotDeserialize|Failed to deserialize the account",
    "\n3005|AccountNotEnoughKeys|Not enough account keys given to the instruction",
    "\n3007|AccountOwnedByWrongProgram|The given account is owned by a different program than expected",
    "\n3008|InvalidProgramId|Program ID was not as expected",
    "\n3009|InvalidProgramExecutable|Program account is not executable",
    "\n3010|AccountNotSigner|The given account did not sign",
    "\n3011|AccountNotSystemOwned|The given account is not owned by the system program",
    "\n3012|AccountNotInitialized|The program expected this account to be already initialized",
    "\n3014|AccountNotAssociatedTokenAccount|The given account is not the associated token account",
    "\n3015|AccountSysvarMismatch|The given public key does not match the required sysvar",
    "\n4100|DeclaredProgramIdMismatch|The declared program id does not match the actual program id",
    "\n4101|TryingToInitPayerAsProgramAccount|You cannot/should not initialize the payer account as a program account",
);

const LOG_CAP: usize = 224;

struct Buf {
    b: [u8; LOG_CAP],
    n: usize,
}

impl Buf {
    fn new() -> Self {
        Buf { b: [0; LOG_CAP], n: 0 }
    }
    /// Appends `s`; a piece that does not fit is refused (nothing written), never cut short.
    fn put(&mut self, s: &[u8]) -> Result<(), ProgramError> {
        if s.len() > LOG_CAP - self.n {
            return Err(ProgramError::BorshIoError);
        }
        // SAFETY: n + s.len() <= LOG_CAP, checked above
        unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), self.b.as_mut_ptr().add(self.n), s.len()) };
        self.n += s.len();
        Ok(())
    }
    fn as_str(&self) -> &str {
        // SAFETY: only ASCII pieces are ever written (the static table, digits, fixed text), n <= LOG_CAP
        unsafe { core::str::from_utf8_unchecked(self.b.get_unchecked(..self.n)) }
    }
}

/// `AnchorError occurred. Error Code: <name>. Error Number: <num>. Error Message: <msg>.`, or an error if it does
/// not fit the buffer (the longest line of the table, 4101, is 180 bytes).
fn line(name: &[u8], num: &[u8], msg: &[u8]) -> Result<Buf, ProgramError> {
    let mut b = Buf::new();
    b.put(b"AnchorError occurred. Error Code: ")?;
    b.put(name)?;
    b.put(b". Error Number: ")?;
    b.put(num)?;
    b.put(b". Error Message: ")?;
    b.put(msg)?;
    b.put(b".")?;
    Ok(b)
}

/// Logs a program error the way Anchor does (the account name / source line Anchor adds is left out).
#[inline(never)]
pub fn log(err: &ProgramError) {
    let ProgramError::Custom(code) = err else { return };
    let mut digits = [0u8; 10];
    let mut i = digits.len();
    let mut v = *code;
    while i > 0 {
        i -= 1;
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let num = digits.get(i..).unwrap_or(&[]);
    for row in TABLE.as_bytes().split(|c| *c == b'\n') {
        let Some([b'|', rest @ ..]) = row.strip_prefix(num) else { continue };
        let mut parts = rest.splitn(2, |c| *c == b'|');
        let (Some(name), Some(msg)) = (parts.next(), parts.next()) else { return };
        match line(name, num, msg) {
            Ok(b) => pinocchio::log::sol_log(b.as_str()),
            // a row too long for the buffer (none is): log the number alone rather than a cut line
            Err(_) => {
                let mut b = Buf::new();
                let _ = b.put(b"AnchorError occurred. Error Number: ").and_then(|_| b.put(num)).and_then(|_| b.put(b"."));
                pinocchio::log::sol_log(b.as_str());
            }
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_row_fits_the_log_buffer() {
        let mut rows = 0;
        for row in TABLE.as_bytes().split(|c| *c == b'\n').filter(|r| !r.is_empty()) {
            let mut p = row.splitn(3, |c| *c == b'|');
            let (num, name, msg) = (p.next().unwrap(), p.next().unwrap(), p.next().unwrap());
            let b = line(name, num, msg).expect("row fits");
            assert!(b.as_str().starts_with("AnchorError occurred. Error Code: ") && b.as_str().ends_with('.'));
            rows += 1;
        }
        assert!(rows > 50);
    }

    #[test]
    fn an_overlong_line_is_refused_not_cut() {
        let long = [b'x'; LOG_CAP];
        assert_eq!(line(b"Name", b"6000", &long).err(), Some(ProgramError::BorshIoError));
        let mut b = Buf::new();
        assert!(b.put(&[b'a'; LOG_CAP]).is_ok());
        assert_eq!(b.put(b"b"), Err(ProgramError::BorshIoError));
        assert_eq!(b.n, LOG_CAP);
    }
}
