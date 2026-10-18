//! The bonding curve: a line-for-line copy of programs/launch_curve/src/math.rs (same formulas, same rounding, same
//! u128 intermediates, same errors), with pinocchio's error type.

use crate::consts::*;
use crate::err::{e, EXCEEDS_SOLD, MATH_OVERFLOW};
use pinocchio::program_error::ProgramError;

const SIXTEEN_SUPPLY: u128 = 16 * SUPPLY as u128;

fn ceil_div(a: u128, b: u128) -> u128 {
    if a == 0 {
        0
    } else {
        (a - 1) / b + 1
    }
}

/// Tokens sold after `raised` lamports went into the curve (target `t`), rounded down.
pub fn sold_at(raised: u64, t: u64) -> u64 {
    let raised = raised as u128;
    (SIXTEEN_SUPPLY * raised / (5 * (t as u128 + 3 * raised))) as u64
}

/// Lamports the curve holds when `sold` tokens are out (target `t`), rounded up. `sold <= CURVE_TOKENS`.
pub fn raised_at(sold: u64, t: u64) -> u64 {
    let sold = sold as u128;
    ceil_div(5 * t as u128 * sold, SIXTEEN_SUPPLY - 15 * sold) as u64
}

/// 1% fee, rounded up (every trade pays at least 1 lamport).
pub fn fee(amount: u64) -> u64 {
    ceil_div(amount as u128 * FEE_BPS as u128, BPS as u128) as u64
}

/// The creator's part of a fee (rounded down; the treasury gets the rest).
pub fn creator_part(fee: u64, creator_share_bps: u16) -> u64 {
    (fee as u128 * creator_share_bps as u128 / BPS as u128) as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuyFill {
    pub net: u64,
    pub fee: u64,
    pub refund: u64,
    pub completes: bool,
    pub new_raised: u64,
    pub new_sold: u64,
    pub tokens_out: u64,
}

pub fn buy_fill(raised: u64, sold: u64, t: u64, gross: u64) -> Result<BuyFill, ProgramError> {
    let mut f = fee(gross);
    let mut net = gross - f;
    let remaining = t.checked_sub(raised).ok_or(e(MATH_OVERFLOW))?;
    let mut refund = 0;
    let completes = net >= remaining;
    if completes {
        net = remaining;
        f = ceil_div(net as u128 * FEE_BPS as u128, (BPS - FEE_BPS) as u128) as u64;
        refund = gross.checked_sub(net + f).ok_or(e(MATH_OVERFLOW))?;
    }
    let new_raised = raised + net;
    let new_sold = if completes { CURVE_TOKENS } else { sold_at(new_raised, t) };
    let tokens_out = new_sold.checked_sub(sold).ok_or(e(MATH_OVERFLOW))?;
    Ok(BuyFill { net, fee: f, refund, completes, new_raised, new_sold, tokens_out })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SellFill {
    pub gross: u64,
    pub fee: u64,
    pub lamports_out: u64,
    pub new_raised: u64,
    pub new_sold: u64,
}

pub fn sell_fill(raised: u64, sold: u64, t: u64, tokens_in: u64) -> Result<SellFill, ProgramError> {
    let new_sold = sold.checked_sub(tokens_in).ok_or(e(EXCEEDS_SOLD))?;
    let new_raised = raised_at(new_sold, t);
    let gross = raised.checked_sub(new_raised).ok_or(e(MATH_OVERFLOW))?;
    let f = fee(gross);
    Ok(SellFill { gross, fee: f, lamports_out: gross - f, new_raised, new_sold })
}

/// The largest launch buy that does not complete the curve.
#[allow(dead_code)]
pub fn max_launch_buy(t: u64) -> u64 {
    t + (t - 1) / (BPS / FEE_BPS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: u64 = 85_000_000_000;

    #[test]
    fn endpoints_are_exact() {
        assert_eq!(sold_at(0, T), 0);
        assert_eq!(raised_at(0, T), 0);
        assert_eq!(sold_at(T, T), CURVE_TOKENS);
        assert_eq!(raised_at(CURVE_TOKENS, T), T);
    }

    #[test]
    fn rounding_favours_the_curve() {
        let mut r: u64 = 1;
        while r < T {
            let s = sold_at(r, T);
            assert!(raised_at(s, T) <= r, "r={r}");
            r = r * 3 / 2 + 7;
        }
    }

    #[test]
    fn completing_buy_caps_and_refunds() {
        let f = buy_fill(0, 0, T, T * 2).unwrap();
        assert!(f.completes);
        assert_eq!(f.net, T);
        assert_eq!(f.new_sold, CURVE_TOKENS);
        assert_eq!(f.net + f.fee + f.refund, T * 2);
        assert!(f.fee >= T / 99);
    }

    #[test]
    fn max_launch_buy_is_the_boundary() {
        for t in [MIN_TARGET, 1_150_000_000, T, 115_000_000_000, MAX_TARGET] {
            let d = max_launch_buy(t);
            assert!(!buy_fill(0, 0, t, d).unwrap().completes);
            assert!(buy_fill(0, 0, t, d + 1).unwrap().completes);
        }
    }

    #[test]
    fn buy_then_sell_never_profits() {
        let b = buy_fill(0, 0, T, 1_000_000_000).unwrap();
        let s = sell_fill(b.new_raised, b.new_sold, T, b.tokens_out).unwrap();
        assert!(s.lamports_out < 1_000_000_000);
        assert_eq!(s.new_raised, 0);
    }

    #[test]
    fn extremes_do_not_overflow() {
        let f = buy_fill(0, 0, MAX_TARGET, u64::MAX).unwrap();
        assert!(f.completes);
        let s = sell_fill(MAX_TARGET, CURVE_TOKENS, MAX_TARGET, CURVE_TOKENS).unwrap();
        assert_eq!(s.gross, MAX_TARGET);
        assert_eq!(fee(u64::MAX), (u64::MAX as u128 / 100 + 1) as u64);
    }

    /// xorshift64*: a fixed-seed generator, so a failure reproduces.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Property test: random targets, random buys and sells. The curve state always sits on the curve, the vault
    /// always holds exactly `raised` (+ fees, kept apart), rounding never pays a trader more than the closed forms.
    #[test]
    fn random_walks_keep_the_invariants() {
        let mut g = Rng(0x9e37_79b9_7f4a_7c15);
        for _ in 0..300 {
            let t = MIN_TARGET + g.below(MAX_TARGET - MIN_TARGET + 1);
            let (mut raised, mut sold, mut vault) = (0u64, 0u64, 0u128);
            let mut held: [u64; 4] = [0; 4];
            for _ in 0..200 {
                let who = g.below(4) as usize;
                if g.below(3) > 0 && raised < t {
                    let gross = 1 + match g.below(4) {
                        0 => g.below(1_000),
                        1 => g.below(t / 50 + 1),
                        2 => g.below(t),
                        _ => g.below(u64::MAX / 2),
                    };
                    let b = buy_fill(raised, sold, t, gross).unwrap();
                    assert_eq!(b.net + b.fee + b.refund, gross);
                    assert!(b.fee >= 1 && b.new_raised <= t && b.new_sold <= CURVE_TOKENS);
                    assert_eq!(b.completes, b.new_raised == t);
                    if !b.completes {
                        assert_eq!(b.new_sold, sold_at(b.new_raised, t));
                    }
                    assert!(b.completes || raised_at(b.new_sold, t) <= b.new_raised);
                    vault += b.net as u128;
                    (raised, sold) = (b.new_raised, b.new_sold);
                    held[who] += b.tokens_out;
                } else if held[who] > 0 {
                    let amount = 1 + g.below(held[who]);
                    let s = sell_fill(raised, sold, t, amount).unwrap();
                    assert_eq!(s.new_raised, raised_at(s.new_sold, t));
                    assert_eq!(s.gross, s.fee + s.lamports_out);
                    assert!(s.fee >= (s.gross > 0) as u64);
                    vault -= s.gross as u128;
                    (raised, sold) = (s.new_raised, s.new_sold);
                    held[who] -= amount;
                }
                assert_eq!(vault, raised as u128, "the curve holds exactly what it raised");
                assert_eq!(held.iter().sum::<u64>(), sold, "every sold token is held by someone");
                assert!(sold_at(raised, t) >= sold || raised == t, "sold above the curve");
            }
            // everyone sells out: the curve ends empty and owes nothing
            for h in held.iter_mut() {
                if *h > 0 {
                    let s = sell_fill(raised, sold, t, *h).unwrap();
                    (raised, sold) = (s.new_raised, s.new_sold);
                    *h = 0;
                }
            }
            assert_eq!((raised, sold), (0, 0));
            assert!(sell_fill(raised, sold, t, 1).is_err());
        }
    }

    /// The closed forms are monotone and round towards the curve at every target.
    #[test]
    fn random_points_round_towards_the_curve() {
        let mut g = Rng(42);
        for _ in 0..20_000 {
            let t = MIN_TARGET + g.below(MAX_TARGET - MIN_TARGET + 1);
            let r = g.below(t + 1);
            let s = sold_at(r, t);
            assert!(s <= CURVE_TOKENS && raised_at(s, t) <= r);
            assert!(r == t || sold_at(r + 1, t) >= s);
            let q = g.below(CURVE_TOKENS + 1);
            let back = raised_at(q, t);
            assert!(back <= t && sold_at(back, t) >= q);
            assert!(q == CURVE_TOKENS || raised_at(q + 1, t) >= back);
        }
    }
}
