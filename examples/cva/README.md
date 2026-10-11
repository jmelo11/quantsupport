# Rust CVA/FVA Example

Run from the repository root:

```sh
cargo run -p cva
cargo test -p cva
```

The portfolio contains a five-year USD IRS and a five-year CLP/USD
cross-currency swap, each with a USD-equivalent notional of 10 million.
Both trades share one USD-collateralized netting set.

## Credit and Funding

`data/credit_curve_specs.json` configures a native `CreditCurveConfiguration`
for `{"Credit":"CORPORATE-A"}`, discounted on SOFR with 40% recovery.
Illustrative CDS par spreads in `data/quotes.json` are 80, 100, 120 and
150 basis points at 1Y, 3Y, 5Y and 10Y. The credit curve covers the portfolio
horizon without extrapolation. The CSA's `credit_index` selects its survival
probabilities rather than the retained legacy flat `credit_spread`.

FVA preserves the bootstrapped `TermSOFR3m` funding index plus the existing
40/50/60-basis-point funding-spread overlay. Both contribute sensitivities.
The yield-curve loader accepts the existing data's legacy `currency` metadata
by dropping that field before native configuration deserialization; the
market index determines curve currency.

## Variation Margin

Variation margin is enabled with counterparty and own thresholds of
USD 100,000 and a minimum transfer amount of USD 10,000. These represent
1% and 0.1% of each trade's USD-equivalent notional. Margin calls occur on
each monthly simulation date. CVA/FVA aggregates use margin-adjusted
exposures; printed raw cube profiles remain pre-margin and can exceed the
thresholds. This example does not model initial margin, a margin period of
risk, or settlement lag.

## Calibration

Both LGM calibrations remain enabled. The SOFR ATM caplet basket explicitly
uses the available 6M, 1Y, 2Y, 3Y and 4Y expiries. The ICP ATM swaption basket
uses 6M, 1Y, 2Y and 3Y expiries with a 1Y tenor. These queries run within the
existing strike and expiry grids; no volatility extrapolation or additional
cube-boundary workaround is introduced. Calibration ends before the 5Y
portfolio maturity, so the final calibrated sigma bucket governs the
remaining horizon.
