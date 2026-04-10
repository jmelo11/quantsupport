use crate::{
    quotes::quote::{CalibrationInstrumentType, Level, Quote},
    time::date::Date,
};

/// A calibration instrument is a quote that has been turned into a
/// concrete [`CalibrationInstrumentType`] with a known pillar date and scalar quote
/// value.
#[derive(Clone)]
pub struct CalibrationInstrument {
    quote: Quote,
    level: Level,
    built: CalibrationInstrumentType,
    quote_value: f64,
    pillar_date: Date,
}

impl CalibrationInstrument {
    /// Creates a calibration instrument.
    #[must_use]
    pub const fn new(
        quote: Quote,
        level: Level,
        built: CalibrationInstrumentType,
        quote_value: f64,
        pillar_date: Date,
    ) -> Self {
        Self {
            quote,
            level,
            built,
            quote_value,
            pillar_date,
        }
    }

    /// Returns the source quote.
    #[must_use]
    pub const fn quote(&self) -> &Quote {
        &self.quote
    }

    /// Returns the quote level.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Returns the built instrument.
    #[must_use]
    pub const fn built(&self) -> &CalibrationInstrumentType {
        &self.built
    }

    /// Returns the market input value.
    #[must_use]
    pub const fn quote_value(&self) -> f64 {
        self.quote_value
    }

    /// Returns the pillar date.
    #[must_use]
    pub const fn pillar_date(&self) -> Date {
        self.pillar_date
    }

    /// Returns the reporting label associated with this calibration input.
    #[must_use]
    pub fn pillar_label(&self) -> String {
        self.quote.details().identifier()
    }
}
