# Worksheet Function Reference

The [Excel function reference](../api/excel/index.html)
provides a page for each worksheet function, including signatures, aliases,
categories, and parameter help. Sphinx generates these pages from the portable
Rust catalog exporter using the XLL's registration metadata, including
macro-generated functions.

The **Math / Interpolation** reference covers `QS.INTERPOLATE` (selectable
`Linear`, `LogLinear`, or `CubicSpline`), the named one-dimensional functions
`QS.INTERPOLATE.LINEAR`, `QS.INTERPOLATE.LOG.LINEAR`, and
`QS.INTERPOLATE.CUBIC.SPLINE`, and the aligned point-range functions
`QS.INTERPOLATE.BILINEAR` and `QS.INTERPOLATE.TRILINEAR`. They return numeric
scalars directly; no object handles are required. Extrapolation is rejected by
default, with an optional native extrapolation flag for one dimension or
`Reject`/`Flat` policies for multidimensional slices.

For the add-in itself, see [Download and Install](installation.md), then follow
the [workbook walkthrough](workbook.md). The
[API reference index](../api/index.html)
also links to the Python reference.
