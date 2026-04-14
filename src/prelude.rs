//! Prelude module for convenient imports.
//!
//! Re-exports the most commonly used types so that
//! `use quantsupport::prelude::*;` brings everything into scope.

pub use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
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
            fxoption::{FxOption, FxOptionTrade, FxOptionType},
            makefxforward::MakeFxForward,
            makefxoption::MakeFxOption,
        },
        rates::{
            basisswap::{BasisSwap, BasisSwapTrade},
            capfloor::{CapFloor, CapFloorTrade, CapFloorType},
            capletfloorlet::{CapletFloorlet, CapletFloorletTrade, CapletFloorletType},
            crosscurrencyswap::{FixFloatCrossCurrencySwap, FixFloatCrossCurrencySwapTrade},
            europeanswaption::{EuropeanSwaption, EuropeanSwaptionTrade, SwaptionType},
            floatfloatcrosscurrencyswap::{
                FloatFloatCrossCurrencySwap, FloatFloatCrossCurrencySwapTrade,
            },
            makebasisswap::MakeBasisSwap,
            makecapfloor::MakeCapFloor,
            makeeuropeanswaption::MakeSwaption,
            makefixfloatcrosscurrencyswap::MakeFixFloatCrossCurrencySwap,
            makefloatfloatcrosscurrencyswap::MakeFloatFloatCrossCurrencySwap,
            makeratefutures::MakeRateFutures,
            makeswap::MakeSwap,
            ratefutures::{RateFutures, RateFuturesTrade},
            swap::{Swap, SwapTrade},
        },
    },
    math::{
        interpolation::interpolator::{Interpolate, Interpolator, StaticInterpolate},
        probability::norm_cdf::NormCDF,
        solvers::{bisection::Bisection, newtonraphson::NewtonRaphson},
    },
    models::{
        hullwhite::{
            hullwhitecalibration::HullWhiteTimeDependentVolatility,
            hullwhitecalibrationquality::{
                HullWhiteCalibrationQuality, HullWhiteCalibrationRecord,
            },
            hullwhitemodel::HullWhite,
        },
        lgm::{
            lgmcomponents::{LgmFxModel, LgmRateModel},
            lgmmarketmodel::LgmMarketModel,
        },
        montecarloengine::{PathGenerator, TimeDependentVolatility},
    },
    pricers::{
        cashflows::discountedcashflowpricer::DiscountedCashflowPricer,
        equity::blackeuropeanoptionpricer::BlackEuropeanOptionPricer,
        fx::fxforwardpricer::FxForwardPricer,
        rates::{
            closedformblackcapletpricer::ClosedFormBlackCapletPricer,
            ratefuturespricer::RateFuturesPricer,
        },
    },
    quotes::{
        fixingstore::FixingStore,
        fxstore::FxStore,
        quote::{Level, Quote, QuoteDetails, QuoteInstrument, QuoteLevels},
        quoteselector::QuoteSelector,
        quotestore::QuoteStore,
    },
    rates::{
        bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            curveconfiguration::CurveConfiguration, multicurvebootstrapper::MultiCurveBootstrapper,
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
    utils::plot::Plot,
    volatility::{
        interpolatedvolatilitycube::InterpolatedVolatilityCube,
        interpolatedvolatilitysurface::InterpolatedVolatilitySurface,
        modelcalibration::{CalibrationSource, ModelCalibrationConfiguration},
        volatilitycube::VolatilityCube,
        volatilitycubebuilder::VolatilityCubeBuilder,
        volatilitycubeconfiguration::VolatilityCubeConfiguration,
        volatilityindexing::{SmileType, Strike, VolatilityType},
        volatilitysurface::VolatilitySurface,
        volatilitysurfacebuilder::VolatilitySurfaceBuilder,
        volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
    },
    xva::{
        contigentclaim::ContingentClaim,
        engine::{XvaEngine, XvaEngineConfig},
        makecontigentclaim::IntoContingentClaims,
        nettingset::NettingSet,
        visitors::{
            claimpreprocessor::ClaimPreprocessor,
            claimcompressionpreprocessor::ClaimCompressionPreprocessor,
            exposureevaluator::{ExposureEvaluator, ExposureResult, NpvCube},
            fixingpreprocessor::FixingPreprocessor,
            marketmodel::MarketModel,
            preprocessorexecutor::PreprocessorExecutor,
        },
    },
};
