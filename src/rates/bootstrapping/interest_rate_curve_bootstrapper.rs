use std::cmp::Ordering;

use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        assetpresets::InterestRateCurvePreset,
        assets::{AssetGenerator, AssetType},
        contextmanager::ContextManager,
        instrument::Instrument,
        pricer::Pricer,
        request::Request,
    },
    indices::{marketindex::MarketIndex, rateindices::rate_index_details},
    instruments::{
        fixedincome::deposit::{Deposit, DepositTrade},
        rates::swap::{InterestRateSwap, InterestRateSwapTrade, SwapDirection},
    },
    marketdata::quote::{Quote, QuoteInstrument},
    math::solvers::{
        bisection::Bisection,
        solvertraits::{ContFunc, OptimizerSolution},
    },
    pricers::{
        fixedincome::discountdepositpricer::DiscountDepositPricer,
        rates::discountinterestrateswappricer::DiscountInterestRateSwapPricer,
    },
    rates::{
        interest_rate_curve::InterestRateCurveAsset,
        yieldtermstructure::discounttermstructure::DiscountTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, period::Period},
    utils::errors::{AtlasError, Result},
};

/// Bootstraps interest rate curves using deposit and swap instruments.
pub struct InterestRateCurveBootstrapper {
    preset: InterestRateCurvePreset,
}

enum BootstrapInstrument {
    Deposit(DepositTrade, DiscountDepositPricer),
    Swap(InterestRateSwapTrade, DiscountInterestRateSwapPricer),
}

impl BootstrapInstrument {
    fn price(&self, ctx: &ContextManager) -> Result<f64> {
        match self {
            Self::Deposit(trade, pricer) => {
                let results = pricer.evaluate(trade, &[Request::Value], ctx)?;
                results
                    .price()
                    .ok_or(AtlasError::ValueNotSetErr("Price not set.".into()))
            }
            Self::Swap(trade, pricer) => {
                let results = pricer.evaluate(trade, &[Request::Value], ctx)?;
                results
                    .price()
                    .ok_or(AtlasError::ValueNotSetErr("Price not set.".into()))
            }
        }
    }
}

struct BootstrappingProblem<'a> {
    ctx: &'a ContextManager,
    market_index: MarketIndex,
    day_counter: DayCounter,
    interpolation: crate::math::interpolation::interpolator::Interpolator,
    enable_extrapolation: bool,
    dates: Vec<Date>,
    dfs: Vec<ADReal>,
    maturity: Date,
    instrument: &'a BootstrapInstrument,
}

impl ContFunc<f64> for BootstrappingProblem<'_> {
    fn call(&self, x: &f64) -> Result<f64> {
        let mut dates = self.dates.clone();
        let mut dfs = self.dfs.clone();
        if let Some(pos) = dates.iter().position(|d| *d == self.maturity) {
            dfs[pos] = ADReal::new(*x);
        } else {
            dates.push(self.maturity);
            dfs.push(ADReal::new(*x));
        }

        let curve = DiscountTermStructure::<ADReal>::new(
            dates,
            dfs,
            self.day_counter,
            self.interpolation,
            self.enable_extrapolation,
        )?;
        let asset = InterestRateCurveAsset::new(self.market_index.clone(), curve, Vec::new());
        self.ctx.assets().insert(
            self.market_index.clone(),
            AssetType::InterestRateCurve(std::sync::Arc::new(asset)),
        );

        self.instrument.price(self.ctx)
    }
}

impl InterestRateCurveBootstrapper {
    /// Creates a new bootstrapper using the supplied preset.
    #[must_use]
    pub const fn new(preset: InterestRateCurvePreset) -> Self {
        Self { preset }
    }

    fn quote_maturity(reference_date: Date, quote: &Quote) -> Result<Date> {
        if let Some(date) = quote.details().maturity() {
            return Ok(date);
        }
        if let Some(tenor) = quote.details().tenor() {
            return Ok(reference_date + tenor);
        }
        Err(AtlasError::NotFoundErr(
            "Quote maturity or tenor missing.".into(),
        ))
    }

    fn collect_quotes(
        &self,
        reference_date: Date,
        quotes: &[Quote],
        identifiers: &[String],
    ) -> Result<Vec<Quote>> {
        let mut filtered: Vec<Quote> = quotes
            .iter()
            .filter(|quote| identifiers.is_empty() || identifiers.contains(&quote.details().identifier()))
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            let a_date = Self::quote_maturity(reference_date, a);
            let b_date = Self::quote_maturity(reference_date, b);
            match (a_date, b_date) {
                (Ok(a_date), Ok(b_date)) => a_date.cmp(&b_date),
                _ => Ordering::Equal,
            }
        });
        Ok(filtered)
    }

    fn solve_df(
        &self,
        ctx: &ContextManager,
        market_index: MarketIndex,
        day_counter: DayCounter,
        interpolation: crate::math::interpolation::interpolator::Interpolator,
        enable_extrapolation: bool,
        dates: Vec<Date>,
        dfs: Vec<ADReal>,
        maturity: Date,
        instrument: &BootstrapInstrument,
    ) -> Result<OptimizerSolution<f64>> {
        let problem = BootstrappingProblem {
            ctx,
            market_index,
            day_counter,
            interpolation,
            enable_extrapolation,
            dates,
            dfs,
            maturity,
            instrument,
        };
        let solver = Bisection::<_>::new(0.0001, 1.5, 100);
        solver.solve(&problem)
    }

    fn bootstrap_discount_factors(
        &self,
        ctx: &ContextManager,
        quotes: &[Quote],
    ) -> Result<(Vec<Date>, Vec<ADReal>, Vec<(String, ADReal)>)> {
        let reference_date = ctx.evaluation_date();
        let index_details = rate_index_details(&self.preset.market_index())
            .ok_or(AtlasError::NotFoundErr("Rate index not found.".into()))?;
        let rate_definition = index_details.rate_definition().ok_or(AtlasError::NotFoundErr(
            "Rate definition not found for index.".into(),
        ))?;
        let fixed_leg_tenor = Period::from_frequency(rate_definition.frequency()).ok_or(
            AtlasError::InvalidValueErr("Unsupported fixed leg frequency.".into()),
        )?;
        let discount_curve_index = self.preset.dependencies().first().cloned();

        let mut dates = vec![reference_date];
        let mut dfs = vec![ADReal::one()];
        let mut inputs = Vec::new();

        for quote in quotes {
            let maturity = Self::quote_maturity(reference_date, quote)?;
            if maturity <= reference_date || dates.contains(&maturity) {
                continue;
            }

            let rate_value = quote.levels().value(&ctx.quote_level())?;
            let rate = ADReal::new(rate_value);
            inputs.push((quote.details().identifier(), rate));

            let instrument = match quote.details().instrument() {
                QuoteInstrument::Deposit => {
                    let deposit = Deposit::new(
                        quote.details().identifier(),
                        1.0,
                        crate::rates::interestrate::InterestRate::from_rate_definition(
                            rate_value,
                            rate_definition,
                        ),
                        reference_date,
                        maturity,
                        self.preset.market_index(),
                    )
                    .resolve(ctx)?;
                    let trade = DepositTrade::new(deposit, reference_date, 1.0);
                    BootstrapInstrument::Deposit(trade, DiscountDepositPricer)
                }
                QuoteInstrument::OIS => {
                    let swap = InterestRateSwap::new(
                        quote.details().identifier(),
                        reference_date,
                        maturity,
                        rate_value,
                        fixed_leg_tenor,
                        fixed_leg_tenor,
                        rate_definition.day_counter(),
                        SwapDirection::PayFixed,
                        self.preset.market_index(),
                    )
                    .resolve(ctx)?;
                    let swap = if let Some(discount_curve_index) = discount_curve_index.clone() {
                        swap.with_discount_curve_index(discount_curve_index)
                    } else {
                        swap
                    };
                    let trade = InterestRateSwapTrade::new(swap, reference_date, 1.0);
                    BootstrapInstrument::Swap(trade, DiscountInterestRateSwapPricer)
                }
                _ => {
                    return Err(AtlasError::InvalidValueErr(
                        "Unsupported quote instrument for bootstrapping.".into(),
                    ))
                }
            };

            let solution = self.solve_df(
                ctx,
                self.preset.market_index(),
                rate_definition.day_counter(),
                self.preset.interpolation(),
                self.preset.enable_extrapolation(),
                dates.clone(),
                dfs.clone(),
                maturity,
                &instrument,
            )?;
            dates.push(maturity);
            dfs.push(ADReal::new(solution.x));
        }

        Ok((dates, dfs, inputs))
    }
}

impl AssetGenerator for InterestRateCurveBootstrapper {
    fn generate_assets(&self, ctx: &ContextManager) -> Vec<(MarketIndex, AssetType)> {
        let market_index = self.preset.market_index();
        let Some(quotes_map) = ctx.market_data_provider().quotes_for_index(&market_index) else {
            return Vec::new();
        };

        let quotes: Vec<Quote> = quotes_map.values().cloned().collect();
        let quotes = match self.collect_quotes(ctx.evaluation_date(), &quotes, self.preset.instruments()) {
            Ok(values) => values,
            Err(_) => Vec::new(),
        };

        let Ok((dates, dfs, inputs)) = self.bootstrap_discount_factors(ctx, &quotes) else {
            return Vec::new();
        };

        let index_details = match rate_index_details(&market_index) {
            Some(details) => details,
            None => return Vec::new(),
        };
        let rate_definition = match index_details.rate_definition() {
            Some(rate_definition) => rate_definition,
            None => return Vec::new(),
        };

        let curve = match DiscountTermStructure::<ADReal>::new(
            dates,
            dfs,
            rate_definition.day_counter(),
            self.preset.interpolation(),
            self.preset.enable_extrapolation(),
        ) {
            Ok(curve) => curve,
            Err(_) => return Vec::new(),
        };

        let asset = InterestRateCurveAsset::new(market_index.clone(), curve, inputs);
        vec![(market_index, AssetType::InterestRateCurve(std::sync::Arc::new(asset)))]
    }
}
