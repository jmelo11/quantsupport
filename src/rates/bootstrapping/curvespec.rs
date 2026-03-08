use crate::{
    ad::adreal::{ADReal, IsReal},
    core::request::LegsProvider,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::cashflowtype::CashflowType,
    math::interpolation::interpolator::{Interpolate as _, Interpolator},
    quotes::quote::{BuiltInstrument, Level, Quote},
    rates::{
        bootstrapping::resolvedcurvespec::{ResolvedCurveSpec, ResolvedInstrument},
        compounding::Compounding,
        interestrate::InterestRate,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency, period::Period},
    utils::errors::{QSError, Result},
};

/// Selects market quotes for a given curve index and tenor.
pub trait QuoteSelector {
    /// Returns the quote matching `market_index` and `tenor`.
    fn select(&self, market_index: &MarketIndex, tenor: &Period) -> Option<Quote>;
}

/// User-defined bootstrap specification for a curve, with instrument and settings to be used in the bootstrapping process.
pub struct CurveSpec {
    market_index: MarketIndex,
    currency: Currency,
    day_counter: DayCounter,
    interpolator: Interpolator,
    enable_extrapolation: bool,
    deposits: Vec<Period>,
    futures: Vec<Period>,
    swaps: Vec<Period>,
    basis_swaps: Vec<Period>,
    xccy_swaps: Vec<Period>,
    fx_forwards: Vec<Period>,
    fx_forward_points: Vec<Period>,
}

impl CurveSpec {
    /// Creates a curve specification.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        currency: Currency,
        day_counter: DayCounter,
        interpolator: Interpolator,
        enable_extrapolation: bool,
        deposits: Vec<Period>,
        futures: Vec<Period>,
        swaps: Vec<Period>,
        basis_swaps: Vec<Period>,
        xccy_swaps: Vec<Period>,
        fx_forwards: Vec<Period>,
        fx_forward_points: Vec<Period>,
    ) -> Self {
        Self {
            market_index,
            currency,
            day_counter,
            interpolator,
            enable_extrapolation,
            deposits,
            futures,
            swaps,
            basis_swaps,
            xccy_swaps,
            fx_forwards,
            fx_forward_points,
        }
    }

    /// Returns the market index for this spec.
    #[must_use]
    pub fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the currency of this spec.
    #[must_use]
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Resolves configured tenors into concrete calibration instruments.
    ///
    /// # Errors
    /// Returns an error if quote levels are missing or a pillar date cannot be inferred.
    pub fn resolve(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<ResolvedCurveSpec> {
        let mut instruments = Vec::new();

        self.collect_quotes(selector, level, &self.deposits, &mut instruments)?;
        self.collect_quotes(selector, level, &self.futures, &mut instruments)?;
        self.collect_quotes(selector, level, &self.swaps, &mut instruments)?;
        self.collect_quotes(selector, level, &self.basis_swaps, &mut instruments)?;
        self.collect_quotes(selector, level, &self.xccy_swaps, &mut instruments)?;
        self.collect_quotes(selector, level, &self.fx_forwards, &mut instruments)?;
        self.collect_quotes(selector, level, &self.fx_forward_points, &mut instruments)?;

        let _pillar_dates = instruments
            .iter()
            .map(|x| x.pillar_date())
            .collect::<Vec<Date>>();

        // check if repeated pillar dates are present and log a warning if so, as this may lead to issues in the bootstrapping process

        instruments.sort_by_key(|x| x.pillar_date());

        Ok(ResolvedCurveSpec::new(
            self.market_index.clone(),
            self.currency,
            self.day_counter,
            self.interpolator,
            self.enable_extrapolation,
            instruments,
        ))
    }

    /// Collects quotes and builds instruments for a given bucket of tenors and calibration kind.
    ///
    /// # Errors
    /// Returns an error if quote levels are missing or pillar dates cannot be resolved.
    fn collect_quotes(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
        tenors: &[Period],
        out: &mut Vec<ResolvedInstrument>,
    ) -> Result<()> {
        for tenor in tenors {
            let Some(quote) = selector.select(&self.market_index, tenor) else {
                continue;
            };

            let fallback_value = ADReal::new(quote.levels().value(level)?);
            let built = quote.build_instrument(level)?;
            let pillar_date = self.resolve_pillar_dates(&built)?;

            // When the tape is recording, the instrument build creates
            // ADReal leaf nodes for its internal rates.  We extract that
            // same ADReal so `pillar_values` shares the identical tape
            // node used in the bootstrap residual computation, giving
            // end-to-end AD connectivity from quote → solver → DF.
            let quote_value = Self::extract_primary_rate(&built).unwrap_or(fallback_value);

            out.push(ResolvedInstrument::new(
                quote,
                level,
                built,
                quote_value,
                pillar_date,
            ));
        }
        Ok(())
    }

    /// Extracts the primary AD-enabled rate from a built instrument.
    ///
    /// For instruments whose quote enters the residual through the
    /// coupon-level `InterestRate` (deposits, swaps), this returns the
    /// `ADReal` stored in the first [`FixedRateCoupon`].  Because
    /// [`ADReal`] is `Copy` with a shared tape-node pointer, this is the
    /// *same* tape leaf used by every coupon of the instrument.
    ///
    /// For instruments where the quote is used directly in the residual
    /// function (FX forwards, rate futures), the fallback `quote_value`
    /// created in [`collect_quotes`] is used instead.
    fn extract_primary_rate(built: &BuiltInstrument) -> Option<ADReal> {
        match built {
            BuiltInstrument::FixedRateDeposit(dep) => dep.leg().cashflows().iter().find_map(|cf| {
                if let CashflowType::FixedRateCoupon(c) = cf {
                    Some(c.rate().rate())
                } else {
                    None
                }
            }),
            BuiltInstrument::Swap(swap) => {
                for leg in swap.legs() {
                    for cf in leg.cashflows() {
                        if let CashflowType::FixedRateCoupon(c) = cf {
                            return Some(c.rate().rate());
                        }
                    }
                }
                None
            }
            BuiltInstrument::BasisSwap(bs) => {
                // For basis swaps the quote is typically a spread on
                // one of the floating legs.
                for leg in bs.legs() {
                    for cf in leg.cashflows() {
                        if let CashflowType::FloatingRateCoupon(c) = cf {
                            let s = c.spread();
                            if s.value().abs() > 1e-18 {
                                return Some(s);
                            }
                        }
                    }
                }
                None
            }
            // Rate futures and FX forwards: the quote value is passed
            // directly to the residual function, so we rely on the
            // fallback created in collect_quotes.
            _ => None,
        }
    }

    /// Resolves the pillar date for a given built instrument.
    fn resolve_pillar_dates(&self, built: &BuiltInstrument) -> Result<Date> {
        match built {
            BuiltInstrument::FixedRateDeposit(x) => Ok(x.leg().last_payment_date()),
            BuiltInstrument::Swap(x) => Ok(x
                .legs()
                .iter()
                .map(|leg| leg.last_payment_date())
                .max()
                .unwrap()),
            BuiltInstrument::BasisSwap(x) => Ok(x
                .legs()
                .iter()
                .map(|leg| leg.last_payment_date())
                .max()
                .unwrap()),
            BuiltInstrument::CrossCurrencySwap(x) => Ok(x
                .legs()
                .iter()
                .map(|leg| leg.last_payment_date())
                .max()
                .unwrap()),
            BuiltInstrument::RateFutures(x) => Ok(x.end_date()),
            BuiltInstrument::FxForward(x) => Ok(x.delivery_date()),
            _ => Err(QSError::InvalidValueErr("Instrument not supported".into())),
        }
    }
}

/// Curve state carrying current obtained values during bootstrapping.
#[derive(Clone)]
pub struct BootstrappedCurve {
    times: Vec<f64>,
    discount_factors: Vec<ADReal>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    reference_date: Date,
}

impl BootstrappedCurve {
    /// Creates a curve with flat discount factors = 1.0 at every pillar.
    #[must_use]
    pub fn new(
        reference_date: Date,
        times: Vec<f64>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Self {
        let discount_factors = vec![ADReal::one(); times.len()];

        Self {
            reference_date,
            times,
            discount_factors,
            day_counter,
            interpolator,
        }
    }

    /// Creates a curve with explicit discount factors.
    ///
    /// `times` and `discount_factors` must have the same length.
    #[must_use]
    pub fn new_with_dfs(
        reference_date: Date,
        times: Vec<f64>,
        discount_factors: Vec<ADReal>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Self {
        Self {
            reference_date,
            times,
            discount_factors,
            day_counter,
            interpolator,
        }
    }

    /// Returns the reference date.
    #[must_use]
    pub fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the day counter.
    #[must_use]
    pub fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the interpolator.
    #[must_use]
    pub fn interpolator(&self) -> Interpolator {
        self.interpolator
    }

    /// Returns the pillar times.
    #[must_use]
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Returns the discount factors.
    #[must_use]
    pub fn discount_factors(&self) -> &[ADReal] {
        &self.discount_factors
    }

    /// Replaces all discount factors.
    pub fn set_discount_factors(&mut self, discount_factors: &[ADReal]) {
        self.discount_factors = discount_factors.to_owned();
    }

    /// Computes the discount factor at `date` by interpolating the curve.
    ///
    /// # Errors
    /// Returns an error if interpolation fails.
    pub fn discount_factor(&self, date: Date) -> Result<ADReal> {
        let year_fraction = ADReal::new(self.day_counter.year_fraction(self.reference_date, date));

        let tmp_yfs = self
            .times
            .iter()
            .copied()
            .map(ADReal::new)
            .collect::<Vec<ADReal>>();

        let discount_factor =
            self.interpolator
                .interpolate(year_fraction, &tmp_yfs, &self.discount_factors, true)?;
        Ok(discount_factor)
    }

    /// Computes the simply-compounded forward rate between two dates.
    ///
    /// # Errors
    /// Returns an error if the underlying discount-factor interpolation fails.
    pub fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<ADReal> {
        let discount_factor_to_star = self.discount_factor(start_date)?;
        let discount_factor_to_end = self.discount_factor(end_date)?;

        let comp_factor = discount_factor_to_star / discount_factor_to_end;
        let t = self.day_counter.year_fraction(start_date, end_date);

        Ok(InterestRate::<ADReal>::implied_rate(
            comp_factor.into(),
            self.day_counter,
            comp,
            freq,
            t,
        )?
        .rate())
    }
}
