use serde::{Deserialize, Serialize};

use crate::{
    cashflows::cashflow::{Cashflow, Side},
    core::traits::HasCurrency,
    currencies::enums::Currency,
    rates::interestrate::RateDefinition,
    time::{date::Date, enums::Frequency},
    utils::errors::{AtlasError, Result},
    visitors::traits::HasCashflows,
};

use super::{
    fixedrateinstrument::FixedRateInstrument, floatingrateinstrument::FloatingRateInstrument,
    traits::Structure,
};

/// # `RateType`
/// Represents the type of rate.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateType {
    /// Fixed rate type.
    Fixed,
    /// Floating rate type.
    Floating,
    /// Fixed then floating rate type.
    FixedThenFloating,
    /// Floating then fixed rate type.
    FloatingThenFixed,
    /// Fixed then fixed rate type.
    FixedThenFixed,
    /// Shuffled rate type.
    Suffled,
}

impl TryFrom<String> for RateType {
    type Error = AtlasError;
    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Fixed" => Ok(Self::Fixed),
            "Floating" => Ok(Self::Floating),
            "FixedThenFloating" => Ok(Self::FixedThenFloating),
            "FloatingThenFixed" => Ok(Self::FloatingThenFixed),
            "FixedThenFixed" => Ok(Self::FixedThenFixed),
            "Suffled" => Ok(Self::Suffled),
            _ => Err(AtlasError::InvalidValueErr(format!(
                "Invalid rate type: {s}"
            ))),
        }
    }
}

impl From<RateType> for String {
    fn from(rate_type: RateType) -> Self {
        match rate_type {
            RateType::Fixed => "Fixed".to_string(),
            RateType::Floating => "Floating".to_string(),
            RateType::FixedThenFloating => "FixedThenFloating".to_string(),
            RateType::FloatingThenFixed => "FloatingThenFixed".to_string(),
            RateType::FixedThenFixed => "FixedThenFixed".to_string(),
            RateType::Suffled => "Suffled".to_string(),
        }
    }
}

/// # `Instrument`
/// Represents an instrument. This is a wrapper around the `FixedRateInstrument` and
/// `FloatingRateInstrument` types.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Instrument {
    /// Fixed rate instrument.
    FixedRateInstrument(FixedRateInstrument),
    /// Floating rate instrument.
    FloatingRateInstrument(FloatingRateInstrument),
}

impl HasCashflows for Instrument {
    fn cashflows(&self) -> &[Cashflow] {
        match self {
            Self::FixedRateInstrument(fri) => fri.cashflows(),
            Self::FloatingRateInstrument(fri) => fri.cashflows(),
        }
    }

    fn mut_cashflows(&mut self) -> &mut [Cashflow] {
        match self {
            Self::FixedRateInstrument(fri) => fri.mut_cashflows(),
            Self::FloatingRateInstrument(fri) => fri.mut_cashflows(),
        }
    }
}

impl Instrument {
    /// Returns the notional value of the instrument.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        match self {
            Self::FixedRateInstrument(fri) => fri.notional(),
            Self::FloatingRateInstrument(fri) => fri.notional(),
        }
    }

    /// Returns the start date of the instrument.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        match self {
            Self::FixedRateInstrument(fri) => fri.start_date(),
            Self::FloatingRateInstrument(fri) => fri.start_date(),
        }
    }

    /// Returns the end date of the instrument.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        match self {
            Self::FixedRateInstrument(fri) => fri.end_date(),
            Self::FloatingRateInstrument(fri) => fri.end_date(),
        }
    }

    /// Returns the identifier of the instrument.
    #[must_use]
    pub fn id(&self) -> Option<String> {
        match self {
            Self::FixedRateInstrument(fri) => fri.id(),
            Self::FloatingRateInstrument(fri) => fri.id(),
        }
    }

    /// Returns the structure of the instrument.
    #[must_use]
    pub fn structure(&self) -> Structure {
        match self {
            Self::FixedRateInstrument(fri) => fri.structure(),
            Self::FloatingRateInstrument(fri) => fri.structure(),
        }
    }

    /// Returns the payment frequency of the instrument.
    #[must_use]
    pub const fn payment_frequency(&self) -> Frequency {
        match self {
            Self::FixedRateInstrument(fri) => fri.payment_frequency(),
            Self::FloatingRateInstrument(fri) => fri.payment_frequency(),
        }
    }

    /// Returns the side of the instrument.
    #[must_use]
    pub const fn side(&self) -> Option<Side> {
        match self {
            Self::FixedRateInstrument(fri) => Some(fri.side()),
            Self::FloatingRateInstrument(fri) => Some(fri.side()),
        }
    }

    /// Returns the issue date of the instrument.
    #[must_use]
    pub const fn issue_date(&self) -> Option<Date> {
        match self {
            Self::FixedRateInstrument(fri) => fri.issue_date(),
            Self::FloatingRateInstrument(fri) => fri.issue_date(),
        }
    }

    /// Returns the rate type of the instrument.
    #[must_use]
    pub const fn rate_type(&self) -> RateType {
        match self {
            Self::FixedRateInstrument(_) => RateType::Fixed,
            Self::FloatingRateInstrument(_) => RateType::Floating,
        }
    }

    /// Returns the fixed rate of the instrument, if applicable.
    #[must_use]
    pub fn rate(&self) -> Option<f64> {
        match self {
            Self::FixedRateInstrument(fri) => Some(fri.rate().rate()),
            Self::FloatingRateInstrument(_) => None,
        }
    }

    /// Returns the spread of the instrument, if applicable.
    #[must_use]
    pub fn spread(&self) -> Option<f64> {
        match self {
            Self::FixedRateInstrument(_) => None,
            Self::FloatingRateInstrument(fri) => Some(fri.spread()),
        }
    }

    /// Returns the forecast curve identifier of the instrument.
    #[must_use]
    pub const fn forecast_curve_id(&self) -> Option<usize> {
        match self {
            Self::FixedRateInstrument(_) => None,
            Self::FloatingRateInstrument(fri) => fri.forecast_curve_id(),
        }
    }

    /// Returns the discount curve identifier of the instrument.
    #[must_use]
    pub const fn discount_curve_id(&self) -> Option<usize> {
        match self {
            Self::FixedRateInstrument(fri) => fri.discount_curve_id(),
            Self::FloatingRateInstrument(fri) => fri.discount_curve_id(),
        }
    }

    /// Sets the discount curve identifier for the instrument.
    pub fn set_discount_curve_id(&mut self, id: usize) {
        match self {
            Self::FixedRateInstrument(fri) => fri.set_discount_curve_id(id),
            Self::FloatingRateInstrument(fri) => fri.set_discount_curve_id(id),
        }
    }

    /// Sets the forecast curve identifier for the instrument.
    pub fn set_forecast_curve_id(&mut self, id: usize) {
        match self {
            Self::FloatingRateInstrument(fri) => fri.set_forecast_curve_id(id),
            Self::FixedRateInstrument(_) => {}
        }
    }

    /// Returns the first rate definition of the instrument.
    #[must_use]
    pub const fn first_rate_definition(&self) -> Option<RateDefinition> {
        match self {
            Self::FixedRateInstrument(fri) => Some(fri.rate().rate_definition()),
            Self::FloatingRateInstrument(fri) => Some(fri.rate_definition()),
        }
    }

    /// Returns the second rate definition of the instrument.
    #[must_use]
    pub const fn second_rate_definition(&self) -> Option<RateDefinition> {
        match self {
            Self::FixedRateInstrument(_) | Self::FloatingRateInstrument(_) => None,
        }
    }
}

impl HasCurrency for Instrument {
    fn currency(&self) -> Result<Currency> {
        match self {
            Self::FixedRateInstrument(fri) => fri.currency(),
            Self::FloatingRateInstrument(fri) => fri.currency(),
        }
    }
}
