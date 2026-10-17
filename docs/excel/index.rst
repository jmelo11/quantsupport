Excel Worksheet API
===================

`Download quantsupport.xll for 64-bit Windows Excel <https://github.com/jmelo11/quantsupport/releases/latest/download/quantsupport.xll>`_

Every function has a separate declaration page with ordered parameters,
worksheet input types, blank/default behavior, results, errors, aliases, and
a link to its Rust implementation. Formula names and parameter order are taken
from the add-in's registration attributes, including macro-generated functions.

.. toctree::
   :maxdepth: 2

   functions/index

Handles and Calculation
-----------------------

Object constructors return revisioned text handles, such as
``QSObject:QuoteStore:usd_quotes:3``. These are owned by the add-in and valid
only in the current Excel process. Changed inputs advance the revision;
functions reject stale handles. Optional cells may be blank only where the
parameter description permits it.

Scalar date queries may be thread-safe. Functions that access the object registry
run on Excel's main calculation thread. Each function page states its registration
mode. ``#VALUE!`` reports invalid inputs or failed construction/evaluation;
``#N/A`` is used by queries when a requested result is absent, as specified on
the individual page.

Excel for macOS and Excel in the browser cannot load the XLL. Installation and
build instructions are in the
`Excel binding README <https://github.com/jmelo11/quantsupport/blob/main/bindings/excel/README.md>`_.
See the `workbook walkthrough <../../excel/workbook.html>`_ for formula composition.