# Amount Validation — Smallest Currency Unit

## The Rule

ALL monetary amounts are in the **smallest unit** of the currency:
- USD: cents ($50.00 = **5000**)
- EUR: cents (€25.50 = **2550**)
- VND: dong (100,000₫ = **100000** — VND has no subunit)
- JPY: yen (¥1000 = **1000** — JPY has no subunit)
- GBP: pence (£10 = **1000**)

## Zero-Decimal Currencies (no subunit)

These currencies use 1:1 (amount = face value):
BIF, CLP, DJF, GNF, ISK, JPY, KMF, KRW, PYG, RWF, UGX, VND, VUV, XAF, XOF, XPF

## Quick Check

Before submitting `tx_add`, verify:
- Is the amount suspiciously small? $50 as `50` is almost certainly wrong — should be `5000`
- Is the amount suspiciously large? 5,000,000 cents = $50,000 — verify with user
- Amounts MUST be positive (> 0). The `type` field (expense/income) determines direction.
