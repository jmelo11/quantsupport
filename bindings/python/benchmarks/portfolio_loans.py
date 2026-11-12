"""Benchmark Python loan portfolios using an optimized extension.

From the repository root, build an optimized extension and run:
    maturin develop -m bindings/python/Cargo.toml --release
    python bindings/python/benchmarks/portfolio_loans.py --trades 100

JSON lines include median evaluation time and numeric results for reconciliation.
Contract construction and market initialization are outside the timed region.
"""

import argparse
import json
import statistics
import time

import quantsupport as qs


REFERENCE = "2026-02-24"
START = "2026-03-02"
REQUESTS = {
    "value": ["Value"],
    "cashflows": ["Cashflows"],
    "risk": ["Value", "Sensitivities"],
    "all": ["Value", "Sensitivities", "Cashflows"],
}


def market():
    indices = ("ESTR", "EURIBOR3m")
    pillars = ("0D", "5Y", "30Y", "50Y")
    quotes = qs.QuoteStore.from_dict({
        "reference_date": REFERENCE,
        "quotes": [{"identifier": f"ZeroRate_EUR_{index}_{pillar}", "mid": 0.03}
                   for index in indices for pillar in pillars],
    })
    curves = [qs.CurveConfiguration(
        market_index=index, day_counter="Actual360", interpolator="Linear",
        quotes=[f"ZeroRate_EUR_{index}_{pillar}" for pillar in pillars],
    ) for index in indices]
    return qs.PricingContext(
        quotes, curves, csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
    )


def portfolio(count, years, floating):
    coupon = (qs.FloatingRateCoupon(index="EURIBOR3m", spread=0.02) if floating
              else qs.FixedRateCoupon(qs.InterestRate(0.05, "Actual360", "Simple")))
    trades = [qs.Trade(qs.Loan(
        identifier=f"LOAN-{i}", start_date=START, maturity_date=f"{2026 + years}-03-02",
        notional=100_000.0 + i, coupons=[coupon], currency="EUR",
        payment_frequency="Monthly", fixing_frequency="Quarterly", fixing_lag=0,
        amortization_rate=0.05, calendar="NullCalendar",
        repayment=qs.LoanRepaymentStructure.french(),
    ), REFERENCE) for i in range(count)]
    return qs.Portfolio("LOANS", trades)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--trades", type=int, default=100)
    parser.add_argument("--repetitions", type=int, default=5)
    args = parser.parse_args()
    if args.trades < 1 or args.repetitions < 1:
        parser.error("trades and repetitions must be positive")
    for years in (5, 30):
        for floating in (False, True):
            book = portfolio(args.trades, years, floating)
            with market() as context:
                for measure, requests in REQUESTS.items():
                    context.evaluate(book, requests)
                    samples = []
                    for _ in range(args.repetitions):
                        start = time.perf_counter()
                        result = context.evaluate(book, requests)
                        samples.append(1000 * (time.perf_counter() - start))
                    print(json.dumps({
                        "years": years, "floating": floating, "trades": args.trades,
                        "measure": measure, "median_ms": statistics.median(samples),
                        "price": result.price, "sensitivities": result.raw_sensitivities,
                    }), flush=True)


if __name__ == "__main__":
    main()
