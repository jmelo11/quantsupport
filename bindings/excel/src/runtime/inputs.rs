//! Typed worksheet values shared by range readers and portable input validation.

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum Cell {
    Blank,
    Number(f64),
    Text(String),
    Boolean(bool),
}

pub type Table = Vec<Vec<Cell>>;

/// Reads one field independently of its worksheet orientation. Blanks keep
/// their positions so related arrays cannot silently shift out of alignment.
pub fn vector(array: &Table, name: &str) -> Result<Vec<Cell>, String> {
    if array.len() <= 1 {
        Ok(array.first().cloned().unwrap_or_default())
    } else if array.iter().all(|row| row.len() == 1) {
        Ok(array.iter().map(|row| row[0].clone()).collect())
    } else {
        Err(format!(
            "{name} must be a scalar, row array or column array"
        ))
    }
}

/// Zips independent fields into internal records. Omitted optional fields are
/// blank; supplied fields must have exactly the primary field's entry count.
pub fn aligned(fields: &[(&str, &[Cell])]) -> Result<Table, String> {
    let Some((primary_name, primary)) = fields.first() else {
        return Ok(Vec::new());
    };
    let mut records = vec![vec![Cell::Blank; fields.len()]; primary.len()];
    for (column, (name, values)) in fields.iter().enumerate() {
        if values.is_empty() {
            continue;
        }
        if values.len() != primary.len() {
            return Err(format!(
                "{name} must have the same number of entries as {primary_name}"
            ));
        }
        for (row, value) in records.iter_mut().zip(*values) {
            row[column] = value.clone();
        }
    }
    Ok(records)
}

impl Cell {
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Blank) || matches!(self, Self::Text(value) if value.trim().is_empty())
    }

    pub fn text(&self, label: &str) -> Result<&str, String> {
        match self {
            Self::Text(value) => Ok(value.trim()),
            _ => Err(format!("{label} must be text")),
        }
    }

    pub fn number(&self, label: &str) -> Result<f64, String> {
        match self {
            Self::Number(value) if value.is_finite() => Ok(*value),
            _ => Err(format!("{label} must be a finite numeric cell")),
        }
    }

    pub fn count(&self, label: &str) -> Result<usize, String> {
        let value = self.number(label)?;
        if value < 0.0 || value.fract() != 0.0 || value > f64::from(i32::MAX) {
            return Err(format!("{label} must be a nonnegative whole number"));
        }
        Ok(value as usize)
    }

    pub fn flag(&self, label: &str) -> Result<bool, String> {
        match self {
            Self::Boolean(value) => Ok(*value),
            Self::Number(0.0) => Ok(false),
            Self::Number(1.0) => Ok(true),
            _ => Err(format!("{label} must be TRUE/FALSE or numeric 0/1")),
        }
    }
}

pub fn label(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn rows<'a>(table: &'a Table, columns: usize, name: &str) -> Result<Vec<&'a [Cell]>, String> {
    if table.iter().flatten().all(Cell::is_blank) {
        return Ok(Vec::new());
    }
    if table.iter().any(|row| row.len() != columns) {
        return Err(format!("{name} must be a {columns}-column worksheet range"));
    }
    Ok(table
        .iter()
        .filter(|row| !row.iter().all(Cell::is_blank))
        .map(Vec::as_slice)
        .collect())
}

/// Canonical worksheet index text, including parameterized native index variants.
pub fn market_index(value: &str) -> Result<quantsupport::prelude::MarketIndex, String> {
    use quantsupport::prelude::{Currency, FxPair, MarketIndex};
    let value = value.trim();
    let currency = |text: &str| {
        text.trim()
            .parse::<Currency>()
            .map_err(|error| error.to_string())
    };
    for (prefix, equity) in [("Equity:", true), ("Credit:", false)] {
        if let Some(name) = value.strip_prefix(prefix) {
            return Ok(if equity {
                MarketIndex::Equity(name.to_string())
            } else {
                MarketIndex::Credit(name.to_string())
            });
        }
    }
    if let Some(pair) = value.strip_prefix("FX:") {
        let (base, quote) = pair
            .split_once('/')
            .ok_or_else(|| format!("invalid FX market index '{value}'"))?;
        return FxPair::new(currency(base)?, currency(quote)?)
            .map(MarketIndex::FxPair)
            .map_err(|error| error.to_string());
    }
    if let Some(pair) = value.strip_prefix("Collateral:") {
        let (base, collateral) = pair
            .split_once('/')
            .ok_or_else(|| format!("invalid collateral market index '{value}'"))?;
        return Ok(MarketIndex::Collateral(
            currency(base)?,
            currency(collateral)?,
        ));
    }
    Ok(value.parse().unwrap_or_else(|never| match never {}))
}

#[cfg(test)]
mod array_tests {
    use super::*;
    use crate::runtime::{loan_inputs, market_inputs, pricing};
    use quantsupport::prelude::{Currency, Date, Level, MarketIndex};

    fn text(value: &str) -> Cell {
        Cell::Text(value.into())
    }
    fn column(values: &[Cell]) -> Table {
        values.iter().cloned().map(|value| vec![value]).collect()
    }

    #[test]
    fn fields_can_be_independently_transposed_without_changing_quotes_or_missing_sides() {
        let ids = vec![
            text("FixedRateDeposit_USD_SOFR_6M"),
            text("FixedRateDeposit_USD_SOFR_1Y"),
        ];
        let mid = vec![Cell::Number(0.04), Cell::Blank];
        let ask = vec![Cell::Blank, Cell::Number(0.05)];
        let first = aligned(&[
            ("identifiers", &vector(&column(&ids), "ids").unwrap()),
            ("mid", &vector(&vec![mid.clone()], "mid").unwrap()),
            ("bid", &[]),
            ("ask", &vector(&column(&ask), "ask").unwrap()),
        ])
        .unwrap();
        let transposed = aligned(&[
            ("identifiers", &vector(&vec![ids], "ids").unwrap()),
            ("mid", &vector(&column(&mid), "mid").unwrap()),
            ("bid", &[]),
            ("ask", &vector(&vec![ask], "ask").unwrap()),
        ])
        .unwrap();
        assert_eq!(first, transposed);
        let store = market_inputs::quotes(Date::new(2026, 1, 15), &first).unwrap();
        assert_eq!(
            store
                .quote("FixedRateDeposit_USD_SOFR_6M")
                .unwrap()
                .levels()
                .value(Level::Mid)
                .unwrap(),
            0.04
        );
        assert!(store
            .quote("FixedRateDeposit_USD_SOFR_1Y")
            .unwrap()
            .levels()
            .value(Level::Mid)
            .is_err());
        assert_eq!(
            store
                .quote("FixedRateDeposit_USD_SOFR_1Y")
                .unwrap()
                .levels()
                .value(Level::Ask)
                .unwrap(),
            0.05
        );
    }

    #[test]
    fn independent_fixing_fx_and_discount_arrays_reach_native_objects() {
        let indices = vector(&vec![vec![text("SOFR"), text("SOFR")]], "indices").unwrap();
        let dates = vector(
            &vec![vec![text("2026-01-14")], vec![text("2026-01-15")]],
            "dates",
        )
        .unwrap();
        let values = vec![Cell::Number(0.03), Cell::Number(0.04)];
        let history = market_inputs::fixings(
            &aligned(&[
                ("indices", &indices),
                ("dates", &dates),
                ("values", &values),
            ])
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            history
                .fixing(&MarketIndex::SOFR, Date::new(2026, 1, 15))
                .unwrap(),
            0.04
        );
        let currencies = vec![text("USD"), text("EUR")];
        let other = vec![text("EUR"), text("USD")];
        let rates = vec![Cell::Number(0.9), Cell::Number(1.1)];
        let fx = market_inputs::fx(
            &aligned(&[("base", &currencies), ("quote", &other), ("rates", &rates)]).unwrap(),
        )
        .unwrap();
        assert_eq!((fx[0].base, fx[1].quote), (Currency::USD, Currency::USD));
        let curves = vec![text("SOFR"), text("ESTR")];
        let policy = pricing::fixed_income_policy(
            &aligned(&[("currencies", &currencies), ("curves", &curves)]).unwrap(),
            false,
        )
        .unwrap();
        use quantsupport::prelude::DiscountPolicy;
        assert_eq!(policy.discount_indices().len(), 2);
    }

    #[test]
    fn loan_arrays_preserve_a_blank_final_period_and_dated_cpr_alignment() {
        let periods = vector(&vec![vec![Cell::Number(6.0), Cell::Blank]], "periods").unwrap();
        let rates = vector(
            &vec![vec![Cell::Number(0.03)], vec![Cell::Number(0.04)]],
            "rates",
        )
        .unwrap();
        let coupons =
            aligned(&[("periods", &periods), ("rates", &rates), ("indices", &[])]).unwrap();
        let dates = vec![text("2026-01-15"), text("2026-07-15")];
        let cprs = vec![Cell::Number(0.1), Cell::Number(0.2)];
        let prepayments = aligned(&[("dates", &dates), ("cprs", &cprs)]).unwrap();
        let options = loan_inputs::options(&vec![], &coupons, &prepayments).unwrap();
        let segments = options.coupon_segments.unwrap();
        assert_eq!(segments[0].periods, Some(6));
        assert_eq!(segments[1].periods, None);
        assert_eq!(segments[1].rate, 0.04);
        assert!(options.prepayment.is_some());
    }

    #[test]
    fn mismatched_arrays_and_incomplete_rows_cannot_shift_observations() {
        let ids = vec![text("SOFR"), Cell::Blank, text("SOFR")];
        let dates = vec![text("2026-01-13"), text("2026-01-14"), text("2026-01-15")];
        let values = vec![Cell::Number(0.01), Cell::Number(0.02), Cell::Number(0.03)];
        assert!(aligned(&[("indices", &ids), ("dates", &dates[..2])]).is_err());
        assert!(aligned(&[("indices", &[]), ("values", &values)]).is_err());
        let records =
            aligned(&[("indices", &ids), ("dates", &dates), ("values", &values)]).unwrap();
        assert!(market_inputs::fixings(&records).is_err());
        assert_eq!(records[2][2], Cell::Number(0.03));
        assert!(vector(
            &vec![
                vec![text("SOFR"), text("ESTR")],
                vec![text("SOFR"), text("ESTR")]
            ],
            "indices"
        )
        .is_err());
    }
}
