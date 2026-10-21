//! Validate worksheet configuration inputs before mutating a live collection.

use quantsupport::prelude::{
    CreditCurveConfiguration, CurveConfiguration, CurveConstruction, QuoteDetails,
    VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};
use std::collections::HashSet;

pub fn quote_identifiers(identifiers: &[String]) -> Result<(), String> {
    if identifiers.is_empty() {
        return Err("a bootstrapped configuration requires quote identifiers".into());
    }
    let mut seen = HashSet::new();
    for identifier in identifiers {
        identifier
            .parse::<QuoteDetails>()
            .map_err(|error| error.to_string())?;
        if !seen.insert(identifier.to_ascii_lowercase()) {
            return Err(format!("duplicate quote identifier: {identifier}"));
        }
    }
    Ok(())
}

pub trait WorksheetConfiguration {
    fn validate(&self) -> Result<(), String>;
}

impl WorksheetConfiguration for CurveConfiguration {
    fn validate(&self) -> Result<(), String> {
        match self.construction() {
            CurveConstruction::Bootstrapped => quote_identifiers(self.quotes()),
            CurveConstruction::ZeroRate { pillars, rates, .. } => {
                if pillars.is_empty() || pillars.len() != rates.len() {
                    return Err(
                        "zero-rate curves require equally sized, nonempty pillars and rates".into(),
                    );
                }
                if rates.iter().any(|rate| !rate.is_finite()) {
                    return Err("zero rates must be finite".into());
                }
                Ok(())
            }
            CurveConstruction::Composite {
                base_index,
                spread_index,
            } => {
                if base_index == self.market_index() || spread_index == self.market_index() {
                    return Err("a composite curve cannot depend on itself".into());
                }
                Ok(())
            }
        }
    }
}

impl WorksheetConfiguration for CreditCurveConfiguration {
    fn validate(&self) -> Result<(), String> {
        quote_identifiers(self.quotes())?;
        if !self.recovery().is_finite() || !(0.0..=1.0).contains(&self.recovery()) {
            return Err("recovery must be in [0, 1]".into());
        }
        Ok(())
    }
}
impl WorksheetConfiguration for VolatilitySurfaceConfiguration {
    fn validate(&self) -> Result<(), String> {
        quote_identifiers(self.quotes())
    }
}
impl WorksheetConfiguration for VolatilityCubeConfiguration {
    fn validate(&self) -> Result<(), String> {
        quote_identifiers(self.quotes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_are_validated_before_a_store_is_touched() {
        assert!(quote_identifiers(&[]).is_err());
        assert!(quote_identifiers(&["bad quote".into()]).is_err());
        assert!(quote_identifiers(&["OIS_USD_SOFR_1Y".into(), "ois_usd_sofr_1y".into()]).is_err());
        assert!(quote_identifiers(&["ois_usd_sofr_1y".into()]).is_ok());
    }
}
