/// Lower-triangular Cholesky-style factorization using the input's lower triangle.
///
/// For a valid symmetric positive-definite matrix, returns `L` with
/// `A = L * Lᵀ` up to roundoff. Symmetry and definiteness are not validated.
/// Negative diagonal remainders are clamped to zero, so invalid inputs need
/// not reconstruct the original matrix.
///
/// # Panics
/// Panics if a row is too short for the accessed lower-triangular entries.
#[must_use]
pub fn cholesky(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = matrix.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let sum: f64 = l[i]
                .iter()
                .zip(l[j].iter())
                .take(j)
                .map(|(a, b)| a * b)
                .sum();
            if i == j {
                l[i][j] = (matrix[i][i] - sum).max(0.0).sqrt();
            } else if l[j][j].abs() > 1e-14 {
                l[i][j] = (matrix[i][j] - sum) / l[j][j];
            }
        }
    }
    l
}
