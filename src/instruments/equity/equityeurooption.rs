use crate::{
    core::{contextmanager::ContextManager, instrument::Instrument, trade::Trade},
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::Result,
};

#[derive(Clone)]
pub enum EuroOptionType {
    Call,
    Put,
}

#[derive(Clone)]
pub struct EquityEuroOption {
    market_index: MarketIndex,
    expiry_date: Date,
    strike: f64,
    option_type: EuroOptionType,
    identifier: String,
}

pub struct EquityEuroOptionTrade {
    instrument: EquityEuroOption,
    notional: f64,
    trade_date: Date,
}

impl Instrument for EquityEuroOption {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn resolve(&self, _: &ContextManager) -> Result<Self> {
        Ok(self.clone())
    }
}

impl Trade<EquityEuroOption> for EquityEuroOptionTrade {
    fn instrument(&self) -> EquityEuroOption {
        self.instrument.clone()
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }
}
