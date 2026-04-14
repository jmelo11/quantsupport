/// Cholesky decomposition of a symmetric positive-definite matrix.
///
/// Returns the lower-triangular factor **L** such that `A = L * Lᵀ`.
/// Negative diagonal remainders are clamped to zero for numerical safety.
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
