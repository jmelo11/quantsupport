use quantsupport::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct JsonCurveSpecs {
    curve_specs: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct JsonCreditCurveSpecs {
    credit_curve_specs: Vec<CreditCurveConfiguration>,
}

/// Volatility surface and cube configurations loaded from `vol_specs.json`.
#[derive(serde::Deserialize)]
pub struct VolSpecs {
    pub volatility_surfaces: Vec<VolatilitySurfaceConfiguration>,
    pub volatility_cubes: Vec<VolatilityCubeConfiguration>,
}

pub fn load_quotes(path: &PathBuf) -> Result<QuoteStore> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let records: QuoteStoreRecords =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    QuoteStore::try_from(records)
}

pub fn load_curve_specs(path: &PathBuf) -> Result<Vec<CurveConfiguration>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    json.curve_specs
        .into_iter()
        .map(|mut spec| {
            if let Some(fields) = spec.as_object_mut() {
                fields.remove("currency");
            }
            serde_json::from_value(spec).map_err(|e| QSError::InvalidValueErr(e.to_string()))
        })
        .collect()
}

pub fn load_credit_curve_specs(path: &PathBuf) -> Result<Vec<CreditCurveConfiguration>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCreditCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    Ok(json.credit_curve_specs)
}

pub fn load_vol_specs(path: &PathBuf) -> Result<VolSpecs> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    #[test]
    fn legacy_yield_curve_metadata_preserves_funding_curve() {
        let specs = load_curve_specs(&data_dir().join("curve_specs.json")).unwrap();
        assert_eq!(specs.len(), 5);
        assert!(specs
            .iter()
            .any(|spec| *spec.market_index() == MarketIndex::TermSOFR3m));
        assert!(specs
            .iter()
            .any(|spec| *spec.market_index() == MarketIndex::ICP));
    }

    #[test]
    fn credit_curve_and_margin_terms_are_consistent() {
        let specs = load_credit_curve_specs(&data_dir().join("credit_curve_specs.json")).unwrap();
        assert_eq!(specs.len(), 1);
        let credit = &specs[0];
        assert_eq!(
            credit.market_index(),
            &MarketIndex::Credit("CORPORATE-A".into())
        );
        assert_eq!(credit.currency(), Currency::USD);
        assert_eq!(credit.discount_index(), &MarketIndex::SOFR);
        assert_eq!(credit.recovery(), 0.4);
        assert!(!credit.enable_extrapolation());
        assert_eq!(
            credit.quotes(),
            [
                "Cds_CORPORATE-A_USD_1Y",
                "Cds_CORPORATE-A_USD_3Y",
                "Cds_CORPORATE-A_USD_5Y",
                "Cds_CORPORATE-A_USD_10Y",
            ]
        );

        let csa: CsaTerms =
            serde_json::from_reader(File::open(data_dir().join("csa_terms.json")).unwrap())
                .unwrap();
        csa.validate().unwrap();
        assert_eq!(csa.credit_index.as_ref(), Some(credit.market_index()));
        assert_eq!(csa.recovery, credit.recovery());
        assert_eq!(csa.collateral_currency, credit.currency());
        assert!(csa.variation_margin);
        assert_eq!(csa.counterparty_threshold, 100_000.0);
        assert_eq!(csa.own_threshold, 100_000.0);
        assert_eq!(csa.minimum_transfer_amount, 10_000.0);
        assert_eq!(csa.funding_index, Some(MarketIndex::TermSOFR3m));
        let funding = csa.funding_spread_curve.unwrap();
        funding.validate().unwrap();
        assert_eq!(funding.spreads, [0.004, 0.005, 0.006]);
    }
}
