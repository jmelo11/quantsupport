//! Bulk market-data imports from ordinary worksheet ranges.

use std::collections::HashSet;

use quantsupport::prelude::{Date, FixingStore, Quote, QuoteDetails, QuoteLevels, QuoteStore};

use super::{
    inputs::{self, Cell, Table},
    registry::{QsObject, Registry},
};

/// Aligns separate row/column arrays without dropping blank quote sides.
/// Every supplied field must be a scalar, row array or column array.
#[cfg(test)]
pub fn quote_arrays(
    identifiers: &Table,
    mid: Option<&Table>,
    bid: Option<&Table>,
    ask: Option<&Table>,
) -> Result<Table, String> {
    let identifiers = inputs::vector(identifiers, "identifiers")?;
    let mid = mid
        .map(|array| inputs::vector(array, "mid"))
        .transpose()?
        .unwrap_or_default();
    let bid = bid
        .map(|array| inputs::vector(array, "bid"))
        .transpose()?
        .unwrap_or_default();
    let ask = ask
        .map(|array| inputs::vector(array, "ask"))
        .transpose()?
        .unwrap_or_default();
    quote_rows(&identifiers, &mid, &bid, &ask)
}

pub fn quote_rows(
    identifiers: &[Cell],
    mid: &[Cell],
    bid: &[Cell],
    ask: &[Cell],
) -> Result<Table, String> {
    let mut rows = inputs::aligned(&[
        ("identifiers", identifiers),
        ("mid", mid),
        ("bid", bid),
        ("ask", ask),
    ])?;
    // A matching identifier header makes omitted side headers explicit too.
    if let Some(first) = rows.first_mut() {
        if first[0]
            .text("identifier")
            .is_ok_and(|value| inputs::label(value) == "identifier")
        {
            for (column, label) in ["Mid", "Bid", "Ask"].into_iter().enumerate() {
                if first[column + 1].is_blank() {
                    first[column + 1] = Cell::Text(label.into());
                }
            }
        }
    }
    Ok(rows)
}

/// A curve may supply its quote sides, or reference identifiers already added
/// to the context. These are internal records assembled from separate arrays.
pub fn definition_quotes(date: Date, records: &Table) -> Result<(Vec<String>, QuoteStore), String> {
    if records.iter().all(|row| row.len() == 1) {
        let identifiers = inputs::rows(records, 1, "identifiers")?
            .into_iter()
            .map(|row| row[0].text("quote identifier").map(str::to_string))
            .collect::<Result<Vec<_>, _>>()?;
        if !identifiers.is_empty() {
            super::configuration_inputs::quote_identifiers(&identifiers)?;
        }
        return Ok((identifiers, QuoteStore::new(date)));
    }
    let quotes = quotes(date, records)?;
    let mut identifiers: Vec<_> = quotes.quotes().keys().cloned().collect();
    identifiers.sort();
    Ok((identifiers, quotes))
}

pub fn fx(records: &Table) -> Result<Vec<quantsupport::prelude::FxRateRecord>, String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for row in inputs::rows(records, 3, "FX arrays")? {
        let base = row[0]
            .text("base currency")?
            .to_ascii_uppercase()
            .parse()
            .map_err(|e: quantsupport::prelude::QSError| e.to_string())?;
        let quote = row[1]
            .text("quote currency")?
            .to_ascii_uppercase()
            .parse()
            .map_err(|e: quantsupport::prelude::QSError| e.to_string())?;
        let rate = row[2].number("FX rate")?;
        if rate <= 0.0 {
            return Err("FX rate must be positive".into());
        }
        if !seen.insert((base, quote)) {
            return Err("duplicate FX pair".into());
        }
        result.push(quantsupport::prelude::FxRateRecord { base, quote, rate });
    }
    Ok(result)
}

/// Builds one quote store from Identifier/Mid[/Bid/Ask] columns.
pub fn quotes(date: Date, table: &Table) -> Result<QuoteStore, String> {
    let mut store = QuoteStore::new(date);
    if table.iter().flatten().all(Cell::is_blank) {
        return Ok(store);
    }
    let columns = table.first().map_or(0, Vec::len);
    if !(2..=4).contains(&columns) {
        return Err("quotes must have Identifier/Mid columns and optional Bid/Ask columns".into());
    }
    let mut seen = HashSet::new();
    for (position, row) in inputs::rows(table, columns, "quotes")?
        .into_iter()
        .enumerate()
    {
        if position == 0
            && row
                .iter()
                .zip(["identifier", "mid", "bid", "ask"])
                .all(|(cell, expected)| {
                    cell.text("header")
                        .is_ok_and(|text| inputs::label(text) == expected)
                })
        {
            continue;
        }
        let identifier = row[0].text("quote identifier")?;
        let details = identifier
            .parse::<QuoteDetails>()
            .map_err(|error| error.to_string())?;
        if !seen.insert(details.identifier().to_ascii_lowercase()) {
            return Err(format!("duplicate quote identifier: {identifier}"));
        }
        let level = |column: usize, name: &str| match row.get(column) {
            Some(cell) if !cell.is_blank() => cell.number(name).map(Some),
            _ => Ok(None),
        };
        store.add_quote(Quote::new(
            details,
            QuoteLevels::new(level(1, "Mid")?, level(2, "Bid")?, level(3, "Ask")?),
        ));
    }
    Ok(store)
}

/// Builds one fixing store from Market index/Date/Value columns; dates are ISO text.
pub fn fixings(table: &Table) -> Result<FixingStore, String> {
    let mut store = FixingStore::default();
    let mut seen = HashSet::new();
    for (position, row) in inputs::rows(table, 3, "fixings")?.into_iter().enumerate() {
        if position == 0
            && row
                .iter()
                .zip(["marketindex", "date", "value"])
                .all(|(cell, expected)| {
                    cell.text("header")
                        .is_ok_and(|text| inputs::label(text) == expected)
                })
        {
            continue;
        }
        let index_text = row[0].text("market index")?;
        if index_text.is_empty() {
            return Err("market index must not be blank".into());
        }
        let index = inputs::market_index(index_text)?;
        let date = Date::from_str(row[1].text("fixing date")?, "%Y-%m-%d")
            .map_err(|error| error.to_string())?;
        let value = row[2].number("fixing value")?;
        if !seen.insert((index.clone(), date)) {
            return Err(format!("duplicate fixing: {index} on {date}"));
        }
        store.add_fixing(&index, date, value);
    }
    Ok(store)
}

/// Replaces the complete range contribution, reusing unchanged definitions.
pub fn import_quotes(
    registry: &mut Registry,
    name: &str,
    date: Date,
    table: &Table,
) -> Result<String, String> {
    let key = serde_json::to_string(&(date, table)).map_err(|error| error.to_string())?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "QuoteStore", &key)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let store = quotes(date, table)?;
    registry
        .upsert_if_changed(name, QsObject::QuoteStore(Box::new(store)), key)
        .map_err(|error| error.to_string())
}

/// Replaces the complete fixing history, reusing unchanged definitions.
pub fn import_fixings(
    registry: &mut Registry,
    name: &str,
    table: &Table,
) -> Result<String, String> {
    let key = serde_json::to_string(table).map_err(|error| error.to_string())?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "FixingStore", &key)
        .map_err(|error| error.to_string())?
    {
        return Ok(handle);
    }
    let store = fixings(table)?;
    registry
        .upsert_if_changed(name, QsObject::FixingStore(Box::new(store)), key)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quantsupport::prelude::{Level, MarketIndex, PricingContext};

    fn text(value: &str) -> Cell {
        Cell::Text(value.into())
    }

    fn fixing_rows() -> Table {
        vec![
            vec![text("Market index"), text("Date"), text("Value")],
            vec![text("SOFR"), text("2026-01-15"), Cell::Number(0.04)],
            vec![Cell::Blank; 3],
            vec![text("SOFR"), text("2026-01-16"), Cell::Number(0.05)],
        ]
    }

    #[test]
    fn separate_quote_arrays_align_rows_columns_scalars_and_missing_sides() {
        let date = Date::new(2026, 1, 15);
        let identifiers = vec![
            vec![text("FixedRateDeposit_USD_SOFR_6M")],
            vec![text("FixedRateDeposit_USD_SOFR_1Y")],
        ];
        let mid = vec![vec![Cell::Number(0.04), Cell::Blank]];
        let ask = vec![vec![Cell::Blank], vec![Cell::Number(0.05)]];
        let rows = quote_arrays(&identifiers, Some(&mid), None, Some(&ask)).unwrap();
        let store = quotes(date, &rows).unwrap();
        let first = store.quote("FixedRateDeposit_USD_SOFR_6M").unwrap();
        assert_eq!(first.levels().value(Level::Mid).unwrap(), 0.04);
        assert!(first.levels().value(Level::Bid).is_err());
        assert!(first.levels().value(Level::Ask).is_err());
        let second = store.quote("FixedRateDeposit_USD_SOFR_1Y").unwrap();
        assert!(second.levels().value(Level::Mid).is_err());
        assert_eq!(second.levels().value(Level::Ask).unwrap(), 0.05);

        let scalar = quote_arrays(
            &vec![identifiers[0].clone()],
            Some(&vec![vec![Cell::Number(0.06)]]),
            None,
            None,
        )
        .unwrap();
        assert_eq!(quotes(date, &scalar).unwrap().quotes().len(), 1);
        let bid_only = quote_arrays(&identifiers, None, Some(&ask), None).unwrap();
        assert_eq!(
            quotes(date, &bid_only)
                .unwrap()
                .quote("FixedRateDeposit_USD_SOFR_1Y")
                .unwrap()
                .levels()
                .value(Level::Bid)
                .unwrap(),
            0.05
        );
    }

    #[test]
    fn quote_arrays_reject_misalignment_and_rectangular_inputs() {
        let identifiers = vec![
            vec![text("Identifier")],
            vec![text("FixedRateDeposit_USD_SOFR_6M")],
        ];
        let mid = vec![vec![text("Mid"), Cell::Number(0.04)]];
        let rows = quote_arrays(&identifiers, Some(&mid), None, None).unwrap();
        let date = Date::new(2026, 1, 15);
        assert_eq!(quotes(date, &rows).unwrap().quotes().len(), 1);
        assert!(quote_arrays(&rows, None, None, None).is_err());
        assert!(quote_arrays(
            &identifiers,
            Some(&vec![vec![Cell::Number(0.04)]]),
            None,
            None
        )
        .is_err());
        assert!(quote_arrays(
            &identifiers,
            Some(&vec![vec![Cell::Number(0.04); 2]; 2]),
            None,
            None
        )
        .is_err());
        assert!(quote_arrays(&rows, Some(&mid), None, None).is_err());
        let empty: Table = vec![];
        assert!(quote_arrays(&empty, Some(&empty), None, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn quote_ranges_preserve_missing_sides_and_skip_headers_and_blank_rows() {
        let date = Date::new(2026, 1, 15);
        let table = vec![
            vec![text("Identifier"), text("Mid"), text("Bid"), text("Ask")],
            vec![
                text("FixedRateDeposit_USD_SOFR_6M"),
                Cell::Number(0.04),
                Cell::Blank,
                Cell::Number(0.05),
            ],
            vec![Cell::Blank; 4],
        ];
        let store = quotes(date, &table).unwrap();
        assert_eq!(store.reference_date(), date);
        assert_eq!(store.quotes().len(), 1);
        let quote = store.quote("FixedRateDeposit_USD_SOFR_6M").unwrap();
        assert_eq!(quote.levels().value(Level::Mid).unwrap(), 0.04);
        assert!(quote.levels().value(Level::Bid).is_err());
        assert_eq!(quote.levels().value(Level::Ask).unwrap(), 0.05);
        for columns in 2..=4 {
            let rows = table.iter().map(|row| row[..columns].to_vec()).collect();
            assert_eq!(quotes(date, &rows).unwrap().quotes().len(), 1);
        }
    }

    #[test]
    fn invalid_quotes_and_case_insensitive_duplicates_are_rejected() {
        let date = Date::new(2026, 1, 15);
        let row = vec![text("FixedRateDeposit_USD_SOFR_6M"), Cell::Number(0.04)];
        let duplicate = vec![text("fixedratedeposit_usd_sofr_6m"), Cell::Number(0.05)];
        assert!(quotes(date, &vec![row.clone(), duplicate])
            .unwrap_err()
            .contains("duplicate"));
        assert!(quotes(
            date,
            &vec![vec![text("bad identifier"), Cell::Number(0.04)]]
        )
        .is_err());
        for value in [text("0.04"), Cell::Boolean(true), Cell::Number(f64::NAN)] {
            assert!(quotes(date, &vec![vec![row[0].clone(), value]]).is_err());
        }
        assert!(quotes(date, &vec![vec![row[0].clone()]]).is_err());
        assert!(quotes(date, &vec![row, vec![Cell::Number(0.05)]]).is_err());
    }

    #[test]
    fn range_imports_reuse_handles_and_remove_deleted_observations() {
        let mut registry = Registry::default();
        let mut table = fixing_rows();
        let first = import_fixings(&mut registry, "history", &table).unwrap();
        assert_eq!(
            import_fixings(&mut registry, "history", &table).unwrap(),
            first
        );
        let original = registry.fixing_store(&first).unwrap().clone();
        table[1][2] = Cell::Number(0.06);
        table.pop();
        let next = import_fixings(&mut registry, "history", &table).unwrap();
        assert_ne!(next, first);
        assert!(registry.fixing_store(&first).is_err());
        let updated = registry.fixing_store(&next).unwrap();
        assert_eq!(
            updated
                .fixing(&MarketIndex::SOFR, Date::new(2026, 1, 15))
                .unwrap(),
            0.06
        );
        assert!(updated
            .fixing(&MarketIndex::SOFR, Date::new(2026, 1, 16))
            .is_err());
        assert_eq!(
            original
                .fixing(&MarketIndex::SOFR, Date::new(2026, 1, 16))
                .unwrap(),
            0.05
        );
        let empty = import_fixings(&mut registry, "history", &vec![vec![Cell::Blank; 3]]).unwrap();
        assert!(registry
            .fixing_store(&empty)
            .unwrap()
            .fixings(&MarketIndex::SOFR)
            .is_err());

        let date = Date::new(2026, 1, 15);
        let mut rows = vec![
            vec![text("FixedRateDeposit_USD_SOFR_6M"), Cell::Number(0.04)],
            vec![text("FixedRateDeposit_USD_SOFR_1Y"), Cell::Number(0.05)],
        ];
        let first = import_quotes(&mut registry, "quotes", date, &rows).unwrap();
        assert_eq!(
            import_quotes(&mut registry, "quotes", date, &rows).unwrap(),
            first
        );
        rows.pop();
        rows[0][1] = Cell::Number(0.06);
        let next = import_quotes(&mut registry, "quotes", date, &rows).unwrap();
        assert_ne!(next, first);
        assert_eq!(registry.quote_store(&next).unwrap().quotes().len(), 1);
        let empty = import_quotes(&mut registry, "quotes", date, &vec![]).unwrap();
        assert!(registry.quote_store(&empty).unwrap().quotes().is_empty());
    }

    #[test]
    fn fixing_ranges_reject_duplicate_dates_and_invalid_values_before_replacement() {
        let mut registry = Registry::default();
        let mut rows = fixing_rows();
        let handle = import_fixings(&mut registry, "history", &rows).unwrap();
        rows.push(rows[1].clone());
        assert!(import_fixings(&mut registry, "history", &rows)
            .unwrap_err()
            .contains("duplicate"));
        assert!(registry.fixing_store(&handle).is_ok());
        for value in [text("bad"), Cell::Blank, Cell::Number(f64::INFINITY)] {
            let mut rows = fixing_rows();
            rows[1][2] = value;
            assert!(fixings(&rows).is_err());
        }
        for date in [text("2026-02-30"), Cell::Number(46_037.0)] {
            let mut rows = fixing_rows();
            rows[1][1] = date;
            assert!(fixings(&rows).is_err());
        }
        let mut rows = fixing_rows();
        rows[1][0] = text(" ");
        assert!(fixings(&rows).is_err());
    }

    #[test]
    fn hundreds_of_dated_contexts_share_one_fixing_history_and_keep_inputs_isolated() {
        let mut history = FixingStore::default();
        let start = Date::new(2000, 1, 1);
        for day in 0..10_000 {
            history.add_fixing(&MarketIndex::SOFR, start + day, 0.04);
        }
        let contexts: Vec<_> = (0..250)
            .map(|day| {
                PricingContext::new()
                    .with_quote_store(QuoteStore::new(start + 10_000 + day))
                    .with_fixing_store(history.clone())
            })
            .collect();
        for context in &contexts {
            assert!(std::ptr::eq(
                history.fixings(&MarketIndex::SOFR).unwrap(),
                context.fixing_store().fixings(&MarketIndex::SOFR).unwrap(),
            ));
        }
        history.add_fixing(&MarketIndex::SOFR, start, 0.09);
        for context in &contexts {
            assert_eq!(
                context
                    .fixing_store()
                    .fixing(&MarketIndex::SOFR, start)
                    .unwrap(),
                0.04
            );
        }
    }
}
