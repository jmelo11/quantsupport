Excel
=====

`Download the Excel add-in and IntelliSense ZIP for 64-bit Windows Excel <https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip>`_
or the `standalone quantsupport.xll <https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll>`_.

Every function has a separate declaration page with ordered parameters,
worksheet input types, blank/default behavior, results, errors, aliases, and
a link to its Rust implementation. Formula names and parameter order are taken
from the add-in's registration attributes, including macro-generated functions.

.. toctree::
   :maxdepth: 2

   guide/index
   Worksheet reference <functions/index>

Handles and calculation
-----------------------

Object constructors return revisioned text handles, such as
``QSObject:QuoteStore:usd_quotes:3``. These are owned by the add-in and valid
only in the current Excel process/calculation thread. Changed inputs advance
the revision; readers reject stale handles, while mutation chains can accept
older revisions of the same object identity. Optional cells may be blank only where the
parameter description permits it.

Pure date, schedule, and interpolation queries may be thread-safe and can return
scalars or spills. Functions that access the object registry
run on Excel's main calculation thread. Each function page states its registration
mode. ``#VALUE!`` reports invalid inputs or failed construction/evaluation;
``#N/A`` is used by queries when a requested result is absent, as specified on
the individual page.

Excel for macOS and Excel in the browser cannot load the XLL. Installation,
formula composition, and a complete valuation are explained locally in
:doc:`guide/workbook`. The
`binding README <https://github.com/jmelo11/quantsupport/blob/main/bindings/excel/README.md>`_
provides an additional developer entry point.
