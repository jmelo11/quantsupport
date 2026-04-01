//! Prelude module for convenient imports.
//!
//! Re-exports the most commonly used types so that
//! `use quantsupport::prelude::*;` brings everything into scope.

pub use crate::{
    ad::{
        adreal::{ADForward, Const, Dual, DualFwd, Expr, FloatExt, InnerScalar, IsReal, Scalar},
        tape::Tape,
    },
    core::{
        collateral::{
            DiscountPolicy, Discountable, FixedIncomeDiscountPolicy, SingleCurveCSADiscountPolicy,
        },
        elements::{
            curveelement::{ADCurveElement, DiscountCurveElement, DividendCurveElement},
            montecarlosimulationelement::{
                ADMonteCarloSimulationElement, MonteCarloSimulationElement,
            },
            volatilitycubelement::{ADVolatilityCubeElement, VolatilityCubeElement},
            volatilitysurfaceelement::{ADVolatilitySurfaceElement, VolatilitySurfaceElement},
        },
        evaluationresults::{CashflowsTable, EvaluationResults, SensitivityMap},
        instrument::{AssetClass, Instrument},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            constructedelementstore::{ConstructedElementStore, SharedElement},
            fixingrequest::FixingRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricer::Pricer,
        pricerstate::PricerState,
        pricingcontext::PricingContext,
        request::{
            HandleCashflows, HandleFairRate, HandleModifiedDuration, HandleSensitivities,
            HandleValue, HandleYieldToMaturity, LegsProvider, Request,
        },
        trade::{Side, Trade},
        visitable::{Visitable, Visitor},
    },
    currencies::{currency::Currency, currencydetails::CurrencyDetails},
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::RateIndexDetails,
    },
    instruments::{
        cashflows::{
            cashflow::{Cashflow, SimpleCashflow},
            cashflowtype::CashflowType,
            coupons::{LinearCoupon, NonLinearCoupon},
            fixedratecoupon::FixedRateCoupon,
            floatingratecoupon::FloatingRateCoupon,
            leg::Leg,
            makeleg::{MakeLeg, PaymentStructure, RateType},
        },
        equity::{
            equityeuropeanoption::{
                EquityEuropeanOption, EquityEuropeanOptionTrade, EuroOptionType,
            },
            equityforward::{EquityForward, EquityForwardTrade},
            futures::{Futures, FuturesTrade},
            makeequityforward::MakeEquityForward,
            makefutures::MakeFutures,
        },
        fixedincome::{
            fixedratebond::{FixedRateBond, FixedRateBondTrade},
            fixedratedeposit::{FixedRateDeposit, FixedRateDepositTrade},
            floatingratenote::{FloatingRateNote, FloatingRateNoteTrade},
            makefixedratebond::MakeFixedRateBond,
            makefixedratedeposit::MakeFixedRateDeposit,
            makefloatingratenote::MakeFloatingRateNote,
        },
        fx::{
            fxforward::{FxForward, FxForwardSettlement, FxForwardTrade},
            makefxforward::MakeFxForward,
        },
        rates::{
            basisswap::{BasisSwap, BasisSwapTrade},
            capfloor::{CapFloor, CapFloorTrade, CapFloorType},
            capletfloorlet::{CapletFloorlet, CapletFloorletTrade, CapletFloorletType},
            crosscurrencyswap::{FixFloatCrossCurrencySwap, FixFloatCrossCurrencySwapTrade},
            floatfloatcrosscurrencyswap::{
                FloatFloatCrossCurrencySwap, FloatFloatCrossCurrencySwapTrade,
            },
            makebasisswap::MakeBasisSwap,
            makecapfloor::MakeCapFloor,
            makefixfloatcrosscurrencyswap::MakeFixFloatCrossCurrencySwap,
            makefloatfloatcrosscurrencyswap::MakeFloatFloatCrossCurrencySwap,
            makeratefutures::MakeRateFutures,
            makeswap::MakeSwap,
            makeswaption::MakeSwaption,
            ratefutures::{RateFutures, RateFuturesTrade},
            swap::{Swap, SwapTrade},
            swaption::{Swaption, SwaptionExerciseType, SwaptionTrade, SwaptionType},
        },
    },
    math::{
        interpolation::interpolator::{Interpolate, Interpolator, StaticInterpolate},
        probability::norm_cdf::NormCDF,
        solvers::{bisection::Bisection, newtonraphson::NewtonRaphson},
    },
    pricers::{
        cashflows::discountedcashflowpricer::DiscountedCashflowPricer,
        equity::blackeuropeanoptionpricer::BlackEuropeanOptionPricer,
        fx::fxforwardpricer::FxForwardPricer,
        rates::{blackcapletpricer::BlackCapletPricer, ratefuturespricer::RateFuturesPricer},
    },
    quotes::{
        fixingstore::FixingStore,
        quote::{Level, Quote, QuoteDetails, QuoteInstrument, QuoteLevels},
        quotestore::QuoteStore,
    },
    rates::{
        bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            calibrationinstrument::CalibrationInstrument,
            curveconfiguration::{CurveConfiguration, QuoteSelector},
            multicurvebootstrapper::MultiCurveBootstrapper,
        },
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
        yieldtermstructure::{
            discounttermstructure::DiscountTermStructure,
            flatforwardtermstructure::FlatForwardTermStructure,
            interestratestermstructure::InterestRatesTermStructure,
        },
    },
    simulations::simulation::MonteCarloSimulation,
    time::{
        calendar::Calendar,
        date::{Date, NaiveDateExt},
        daycounter::DayCounter,
        enums::{
            BusinessDayConvention, DateGenerationRule, Frequency, IMMMonth, Month, TimeUnit,
            Weekday,
        },
        imm::IMM,
        period::Period,
        schedule::{MakeSchedule, Schedule},
    },
    utils::errors::{QSError, Result},
    volatility::{
        interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
        volatilitycube::VolatilityCube,
        volatilityindexing::{SmileType, Strike, VolatilityType},
        volatilitysurface::VolatilitySurface,
    },
};
