//! Shared spill orientation, preserving each cell's type and blank values.

/// Removes the first (header) row before applying the requested transpose.
pub fn prepare<T>(
    mut cells: Vec<T>,
    mut rows: usize,
    columns: usize,
    transpose: bool,
    headers: bool,
    mut discard: impl FnMut(T),
) -> (Vec<T>, usize, usize) {
    assert_eq!(cells.len(), rows * columns);
    if !headers && rows > 0 {
        cells.drain(..columns).for_each(&mut discard);
        rows -= 1;
    }
    orient(cells, rows, columns, transpose)
}

pub fn orient<T>(
    cells: Vec<T>,
    rows: usize,
    columns: usize,
    transpose: bool,
) -> (Vec<T>, usize, usize) {
    assert_eq!(cells.len(), rows * columns);
    if !transpose {
        return (cells, rows, columns);
    }
    let mut source: Vec<_> = cells.into_iter().map(Some).collect();
    let mut output = Vec::with_capacity(source.len());
    for column in 0..columns {
        for row in 0..rows {
            output.push(source[row * columns + column].take().unwrap());
        }
    }
    (output, columns, rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn transposes_headers_values_and_blanks_together() {
        let cells = vec![
            Some("Date"),
            Some("Value"),
            Some("2026-01-01"),
            None,
            Some("2026-02-01"),
            Some("12"),
        ];
        let (transposed, rows, columns) = super::orient(cells.clone(), 3, 2, true);
        assert_eq!((rows, columns), (2, 3));
        assert_eq!(
            transposed,
            vec![
                Some("Date"),
                Some("2026-01-01"),
                Some("2026-02-01"),
                Some("Value"),
                None,
                Some("12")
            ]
        );
        assert_eq!(super::orient(transposed, rows, columns, true).0, cells);
        assert_eq!(
            super::orient(vec![1, 2, 3], 3, 1, true),
            (vec![1, 2, 3], 1, 3)
        );
        assert_eq!(super::orient(vec![1, 2], 2, 1, false), (vec![1, 2], 2, 1));
        assert_eq!(super::orient(Vec::<i32>::new(), 0, 1, true), (vec![], 1, 0));
        assert_eq!(
            super::prepare(
                vec!["Date", "Value", "Jan", "12", "Feb", "13"],
                3,
                2,
                true,
                false,
                |_| {}
            ),
            (vec!["Jan", "Feb", "12", "13"], 2, 2)
        );
        assert_eq!(
            super::prepare(vec!["Date", "Value"], 1, 2, false, false, |_| {}),
            (vec![], 0, 2)
        );
    }
}
