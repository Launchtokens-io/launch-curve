# launch_curve (Solana)

The on-chain program behind bonding-curve launches on [launchtokens.io](https://launchtokens.io) on Solana mainnet.

| | |
|---|---|
| Program | [`7AxEAL5aCUMiPN12Q7qx2ZLS7QWuVG8pqWNyKXj7qJAv`](https://solscan.io/account/7AxEAL5aCUMiPN12Q7qx2ZLS7QWuVG8pqWNyKXj7qJAv) |
| Config | [`7QvoBjcSsSdEUd2siGWxZtAHyB34q7e1znS9fxkw64D1`](https://solscan.io/account/7QvoBjcSsSdEUd2siGWxZtAHyB34q7e1znS9fxkw64D1) |
| Executable hash | `6ef7ac4f38b16ef2e6b11dd972ed4166f2f432a64ff25c807d06b1a77632dde4` |

## What it does

- Launches a classic SPL token (6 decimals, 1B supply) on a bonding curve: 800M tokens on the curve, 200M for the pool.
- Buys and sells on the curve with a 1% trade fee.
- At the graduation target (85 SOL) it moves the curve's SOL and the pool tokens into a Raydium CPMM pool at the curve's last price and locks all of the LP in Raydium's LP lock forever.
- Short launch protection: for the first 60 seconds a wallet can hold at most 2% of supply.

## What it can't do

- No one can withdraw buyers' SOL from a curve. It leaves only through sells or graduation into the locked pool.
- The pauser can stop new launches and buys only. Selling, graduation, the LP lock, fee collection and claims always work.
- The program is immutable: its upgrade authority is revoked.

## Verify

Build in Docker and compare with the hash above:

```bash
solana-verify build --library-name launch_curve_lean \
  --base-image solanafoundation/solana-verifiable-build@sha256:c78fd4ff5399ee5ea3e154881e8991bd8064899ca162a59cafc682da5a5bd516
solana-verify get-executable-hash target/deploy/launch_curve_lean.so
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com 7AxEAL5aCUMiPN12Q7qx2ZLS7QWuVG8pqWNyKXj7qJAv
```

Never build with `--features devnet`: that switches to devnet DEX addresses.
