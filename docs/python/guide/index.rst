Python User Guide
=================

This guide develops a valuation from its economic inputs rather than from a
list of classes. A contract specifies promised payments, a trade specifies the
position held, and a market supplies the observations and conventions needed
to value that position. Keeping these three objects separate is essential for
reproducible prices, sensitivities, and comparisons between scenarios.

The examples use explicit dates and small in-memory markets. No Book chapter,
spreadsheet, downloaded quote file, or notebook is required to follow them.
The :doc:`computational studies <../studies/index>` extend the same ideas to
larger investigations; the :doc:`API reference <../../autoapi/quantsupport/index>`
remains the authority for individual declarations and supported argument types.

.. toctree::
   :maxdepth: 2

   first-valuation
   markets
   valuation
   risk

Reading Path
------------

Begin with :doc:`first-valuation`, even if familiar with derivatives. Its unit,
direction, and lifecycle conventions apply throughout the interface. Continue
with :doc:`markets` before changing curve or calibration assumptions.
:doc:`valuation` distinguishes instrument-specific quotation and model choices;
:doc:`risk` develops scenario checks, netting, and extension interfaces.

Python 3.9 or later is required by the package. The documentation build uses
Python 3.11 or later and does not need the native binding. Running the examples
does need the binding and pandas; simulation studies additionally use the
dependencies described with their canonical notebooks.
