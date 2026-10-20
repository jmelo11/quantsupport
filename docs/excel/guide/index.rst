Excel user guide
================

The Excel add-in provides worksheet functions for market construction,
trade definition, valuation and risk. A pricing workbook records the inputs
and connects them through formulas, so changing a quote recalculates the
market and its dependent valuations. Handles returned by construction
functions refer to native objects owned by the add-in.

This guide begins with installation and a complete curve-to-swap calculation,
then develops product, risk and historical workflows. The first workbook uses
visible worksheet inputs. Consult the
:doc:`worksheet reference <../functions/index>` for argument order, defaults,
result schemas, aliases and error codes.

.. toctree::
   :maxdepth: 2

   workbook
   markets-and-products
   loans
   risk-and-history

The XLL requires Windows desktop Excel. For related numerical studies, see the
:doc:`Python notebooks <../../python/studies/index>`.
