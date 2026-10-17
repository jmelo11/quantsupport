//! Validated worksheet tables with canonical loan-column names and typed cells.

use std::collections::{HashMap, HashSet};

/// A worksheet cell retaining its original value type, including blanks.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// An empty worksheet cell.
    Blank,
    /// Text to trim when read, without coercing it to a number.
    Text(String),
    /// A numeric worksheet value; checked for finiteness when read.
    Number(f64),
    /// A worksheet boolean, distinct from a numeric or text value.
    Boolean(bool),
}

/// Nonempty rectangular input indexed by unique, normalized column names.
#[derive(Debug)]
pub struct Table {
    columns: HashMap<String, usize>,
    rows: Vec<Vec<Cell>>,
}

/// Trims a header, normalizes case/separators, and resolves supported loan aliases.
///
/// Unknown names are normalized without being accepted; `Table::new` checks
/// them against the caller's allowed schema.
fn normalize_loan_column_header(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    match normalized.as_str() {
        "id" | "loan_id" | "identifier" => "id",
        "start" | "starts" | "start_date" => "start_date",
        "end" | "end_date" | "maturity" | "maturity_date" => "maturity_date",
        "structure" | "repayment" => "repayment",
        "client_rate" | "fixed_rate" => "client_rate",
        "floating_index" | "rate_index" | "index" => "index",
        "balloon_fraction" | "residual_fraction" => "balloon_fraction",
        other => return other.to_string(),
    }
    .to_string()
}

impl Table {
    /// Validates headers and rows against canonical allowed/required column names.
    ///
    /// # Errors
    /// Rejects empty tables, unknown or duplicate columns (including alias
    /// collisions), absent required columns, ragged rows, and rows of blank cells.
    /// Row numbers in error messages are one-based data-row positions.
    pub fn new(
        headers: Vec<String>,
        rows: Vec<Vec<Cell>>,
        allowed: &[&str],
        required: &[&str],
    ) -> Result<Self, String> {
        if headers.is_empty() || rows.is_empty() {
            return Err("headers and data must both be nonempty".to_string());
        }
        let allowed: HashSet<_> = allowed.iter().copied().collect();
        let mut columns = HashMap::new();
        for header in &headers {
            let name = normalize_loan_column_header(header);
            if !allowed.contains(name.as_str()) {
                return Err(format!("unknown column '{header}'"));
            }
            if columns.insert(name.clone(), columns.len()).is_some() {
                return Err(format!("duplicate column '{name}' (including aliases)"));
            }
        }
        for name in required {
            if !columns.contains_key(*name) {
                return Err(format!("missing required column '{name}'"));
            }
        }
        for (index, row) in rows.iter().enumerate() {
            if row.len() != headers.len() {
                return Err(format!(
                    "data row {} has {} columns; headers have {}",
                    index + 1,
                    row.len(),
                    headers.len()
                ));
            }
            if row.iter().all(|cell| matches!(cell, Cell::Blank)) {
                return Err(format!("data row {} is blank", index + 1));
            }
        }
        Ok(Self { columns, rows })
    }

    /// Returns the number of validated data rows, excluding headers.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Reads a zero-based row by canonical name, or a blank for absent columns.
    ///
    /// # Panics
    /// Panics if the column exists and `row` is outside the table's data rows.
    pub fn cell_or_blank(&self, row: usize, name: &str) -> &Cell {
        self.columns
            .get(name)
            .map_or(&Cell::Blank, |column| &self.rows[row][*column])
    }

    /// Reads trimmed text, returning `None` for missing, blank, or empty-text cells.
    ///
    /// # Errors
    /// Rejects non-text values instead of coercing numbers or booleans to strings.
    ///
    /// # Panics
    /// Panics if the column exists and `row` is outside the table's data rows.
    pub fn read_optional_text(
        &self,
        row: usize,
        name: &str,
    ) -> Result<Option<&str>, String> {
        match self.cell_or_blank(row, name) {
            Cell::Blank => Ok(None),
            Cell::Text(value) if value.trim().is_empty() => Ok(None),
            Cell::Text(value) => Ok(Some(value.trim())),
            _ => Err(format!("'{name}' must be text")),
        }
    }

    /// Reads a finite number, returning `None` for missing, blank, or empty-text cells.
    ///
    /// # Errors
    /// Rejects nonfinite numbers and nonnumeric values, including numeric text.
    ///
    /// # Panics
    /// Panics if the column exists and `row` is outside the table's data rows.
    pub fn read_optional_number(
        &self,
        row: usize,
        name: &str,
    ) -> Result<Option<f64>, String> {
        match self.cell_or_blank(row, name) {
            Cell::Blank => Ok(None),
            Cell::Text(value) if value.trim().is_empty() => Ok(None),
            Cell::Number(value) if value.is_finite() => Ok(Some(*value)),
            _ => Err(format!("'{name}' must be a finite numeric cell")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_rejects_ragged_rows_and_alias_collisions() {
        let allowed = &["start_date", "notional"];
        assert!(Table::new(
            vec!["starts".into(), "notional".into()],
            vec![vec![Cell::Text("2026-01-01".into())]],
            allowed,
            &[]
        )
        .unwrap_err()
        .contains("data row 1"));
        assert!(Table::new(
            vec!["starts".into(), "start_date".into()],
            vec![vec![Cell::Blank, Cell::Number(1.0)]],
            allowed,
            &[]
        )
        .unwrap_err()
        .contains("duplicate"));
    }

    #[test]
    fn table_preserves_blank_cells_and_requires_numeric_types() {
        let table = Table::new(
            vec!["notional".into(), "id".into()],
            vec![vec![Cell::Number(100.0), Cell::Blank]],
            &["notional", "id"],
            &["notional"],
        )
        .unwrap();
        assert_eq!(table.row_count(), 1);
        assert_eq!(
            table.read_optional_number(0, "notional").unwrap(),
            Some(100.0)
        );
        assert_eq!(table.read_optional_text(0, "id").unwrap(), None);
        assert!(table.read_optional_text(0, "notional").is_err());
    }
}
