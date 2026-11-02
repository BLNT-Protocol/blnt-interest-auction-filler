# BLNT V3 Auction Filler System Specification

## 1. Purpose and scope

The BLNT V3 auction filler is optional operational infrastructure. It lets a caller fund any canonical BLNT V3 interest-auction bid or a protocol-fee-auction bid with USDC while ensuring bid-token acquisition and the auction fill succeed or fail atomically.

The helper does not create auctions, select interest tiers, alter auction pricing, control a pool, custody protocol funds, or modify BLNT V3. The deployed V3 pool privately selects an interest tier, determines the bid and lot, applies the auction curve, transfers the lot, and accounts for the bid exactly as it would for a direct fill.

## 2. Immutable compatibility boundary

Each helper instance binds at construction:

- one lot-recovery owner;
- the BLNT V3 backstop;
- canonical BLNT, USDC, and XLM;
- canonical BLNT:USDC Comet V1.1; and
- canonical BLNT:XLM Comet V1.1.

All six protocol addresses must be distinct. The owner-authorized caller supplies the BLNT V3 pool, matching the V2 filler trust model. The helper invokes that pool's public type-2/type-3 auction getter and request-type-8/request-type-10 fill interfaces. The bot must validate configured pools before use, and the helper does not accept caller-selected routing contracts.

## 3. USDC-funded bid routes

### 3.1 Plain USDC interest tier

Transfer `max_usdc_in`, retain the exact execution-ledger bid, and refund the difference before filling. The fixed transfer amount keeps caller authorization independent of the execution ledger.

### 3.2 BLNT:USDC interest tier

Transfer `max_usdc_in`, then use the canonical BLNT:USDC pool's exact-output single-sided deposit to mint exactly the execution-ledger LP bid. Refund unused USDC before filling.

### 3.3 Plain XLM interest tier

Transfer `max_usdc_in`. Acquire exactly `max_blnt_in` BLNT from canonical BLNT:USDC, then acquire exactly the execution-ledger XLM bid from canonical BLNT:XLM using at most that BLNT. Refund unused USDC and BLNT before filling. `max_blnt_in` is therefore both the exact intermediate acquired and the second swap's maximum input.

### 3.4 BLNT:XLM interest tier

Transfer `max_usdc_in`. Acquire exactly `max_blnt_in` BLNT from canonical BLNT:USDC, then mint exactly the execution-ledger BLNT:XLM LP bid with a single-sided BLNT deposit capped by that intermediate. Refund unused USDC and BLNT before filling.

### 3.5 Protocol-fee auction

Transfer `max_usdc_in` and acquire exactly the execution-ledger BLNT bid from canonical BLNT:USDC. Refund unused USDC before filling. The pool transfers that BLNT from the helper and burns it under the immutable protocol-fee-auction rules.

Any other bid token is rejected. Every exact-output Comet operation remains subject to Comet V1.1's own maximum in/out ratios and current liquidity.

## 4. Auction identity and scaling

The caller supplies the expected auction start ledger. Interest calls also supply the expected bid token. The helper reads the active auction using the bound backstop address and requires:

- `fill_percent` is from 1 through 100;
- auction type is interest (2) or protocol fee (3), matching the selected entrypoint;
- the active auction start ledger equals the expected ledger;
- the bid map contains exactly one positive entry;
- an interest bid token equals the caller's expected token and one of the four canonical tier assets; and
- a protocol-fee bid token equals canonical BLNT.

The helper computes the execution-ledger bid using the same seven-decimal, ceiling-rounded V3 curve: percentage selection rounds up; the bid remains 100% through elapsed ledger 200, falls by 0.5% per ledger afterward, and is zero at elapsed ledger 400 or later. Arithmetic failure aborts.

## 5. Atomic fill sequence

For a positive bid, one invocation performs:

1. configured-owner authorization;
2. auction-identity validation against the owner-supplied pool;
3. exact execution-ledger bid calculation;
4. bounded USDC transfer;
5. the route-specific exact-output swap or LP acquisition;
6. unused-input refund without touching pre-existing helper balances;
7. interest: exact bid-token approval and exact tier-donation authorization; or protocol fee: exact BLNT-transfer authorization;
8. pool submission with `from`, `spender`, and `to` equal to the helper; and
9. retention of the successful auction lot for owner claim.

If any step fails, Soroban transaction atomicity reverts every earlier state and balance change. A failed or raced fill cannot leave newly acquired LP, XLM, or BLNT in the caller's wallet or helper. At a zero-bid endpoint no USDC is transferred and the helper may still submit the fill if the pool accepts it.

## 6. Private interest-tier authorization

V3 intentionally keeps an active auction's selected tier private, and multiple tiers may use the same plain USDC or XLM token. Bid-token identity therefore cannot reliably reveal the tier.

The helper authorizes `backstop.donate` for FirstLoss, SecondLoss, and ThirdLoss, each with the same exact helper address, owner-supplied pool, and execution-ledger amount. It grants the backstop an allowance for exactly one donation. The supplied pool invokes only its privately selected tier; after one donation the allowance is exhausted, so no second authorization can move funds. A mismatched amount or pool fails atomically.

## 7. Authorization and custody

The configured owner is the only valid `from` address. The owner authorizes the top-level fill and its required USDC transfer. The helper authorizes exact Comet approvals, the bounded tier donation, or the protocol-fee BLNT transfer. Comet approval expiration reproduces the official Comet V1.1 rounding to the next 100,000-ledger boundary.

Successful lots are held by the helper. Only the current owner may invoke `claim(to, assets)` or transfer ownership. Ownership affects lot recovery only and grants no authority over pools, the backstop, Comet, auction creation, or auction selection.

## 8. Safety properties

- `max_usdc_in` caps the owner's USDC input.
- `max_blnt_in` caps the second leg of XLM and BLNT:XLM acquisition.
- Expected auction identity prevents silently filling a replacement auction.
- Exact-output acquisition prevents deliberate bid-token over-minting.
- Exact allowances prevent more than one tier donation.
- Owner authorization makes the caller responsible for supplying a valid BLNT V3 pool.
- Immutable canonical Comet bindings reject arbitrary routing contracts.
- Delta refunds exclude pre-existing helper balances and retain the exact bid.
- Release builds retain checked overflow behavior.

## 9. Auctioneer integration

The auctioneer bot configuration contains the helper address and all immutable asset, backstop, and Comet bindings from the pinned deployment record. Startup validation must confirm the current helper owner, each configured pool's backstop, and both configured Comet token pairs. For interest auctions the bot simulates route costs, supplies the observed auction block/token, and supplies conservative maximum USDC and intermediate BLNT amounts. For protocol-fee auctions it values the reserve lot, supplies a conservative maximum USDC amount, and invokes `fill_protocol_fee`.

After confirmation, the bot records the fill and submits an owner-authorized claim for the lot assets. The bot must not run a separate token-acquisition transaction before using this helper.

## 10. Required verification

Unit and integration tests must cover all four interest-tier assets, protocol-fee BLNT acquisition, unused-input refunds, insufficient caps, replacement auctions, noncanonical tokens, another filler winning first, Comet failure, pool failure after acquisition, zero-bid fills, duplicate tier tokens, claim authorization, ownership transfer, and a four-asset protocol-fee lot.

Before production deployment, an integration test must exercise optimized helper WASM against the exact deployed-compatible BLNT V3 pool, backstop, token, and official Comet V1.1 artifacts. Deployment records must pin the helper WASM hash, constructor bindings, source commit, and transaction hashes.
