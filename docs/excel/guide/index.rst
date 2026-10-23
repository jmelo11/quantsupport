Excel user guide
================

The Excel add-in provides worksheet functions for market construction,
trade definition, valuation and risk. A pricing workbook records the inputs
and connects them through formulas, so changing a quote recalculates the
market and its dependent valuations. Handles returned by construction
functions refer to native objects owned by the add-in.

Start with the context-first workflow for direct prices and historical batches.
The installation walkthrough also explains the explicit component workflow
used by the detailed product studies. Consult the
:doc:`worksheet reference <../functions/index>` for argument order, defaults,
result schemas, aliases and error codes.

.. toctree::
   :maxdepth: 2

   context-first
   workbook
   markets-and-products
   loans
   risk-and-history

The XLL requires Windows desktop Excel. For related numerical studies, see the
:doc:`Python notebooks <../../python/studies/index>`.
