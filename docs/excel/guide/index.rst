Excel User Guide
================

The add-in turns worksheet inputs into native market objects and trade
positions. A cell containing a handle is a dependency on an owned object, not
its serialized contents. A reliable workbook therefore records the market
inputs, construction assumptions, position terms, and valuation requests as an
explicit calculation chain.

This guide is self-contained. It explains installation and a complete
worksheet-native valuation before developing product, risk, and historical
workflows. No Book chapter, Python installation, or JSON file is needed for the
first workbook. Consult the :doc:`worksheet reference <../functions/index>`
for exact argument order, defaults, spill schemas, aliases, and error codes.

.. toctree::
   :maxdepth: 2

   workbook
   markets-and-products
   risk-and-history

Excel is a Windows XLL interface, not a Python runtime. It exposes a deliberate
subset of the native library: :doc:`../../coverage` distinguishes configured
access from products or operations that have no worksheet workflow.
The :doc:`Python computational studies <../../python/studies/index>` are
optional investigations of related financial questions, not prerequisites for
using these formulas.
