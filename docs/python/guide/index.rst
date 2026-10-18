Python user guide
=================

A Python valuation begins with a contract, a position and a market. The
contract defines the payments, the trade records the position held, and the
market supplies quotes, curves and historical observations. A pricer combines
these inputs to calculate the requested results.

This guide builds that workflow using explicit dates and small in-memory
markets. It explains the conventions needed to interpret prices and
sensitivities, then develops product-specific valuation and risk calculations.
The :doc:`computational studies <../studies/index>` provide larger examples,
and the :doc:`API reference <../../autoapi/quantsupport/index>` describes
individual classes, methods and accepted argument types.

.. toctree::
   :maxdepth: 2

   first-valuation
   markets
   valuation
   risk

Reading path
------------

Begin with :doc:`first-valuation` for installation, position direction,
result units and context lifetime. Continue
with :doc:`markets` before changing curve or calibration assumptions.
:doc:`valuation` distinguishes instrument-specific quotation and model choices;
:doc:`risk` develops scenario checks, netting, and extension interfaces.

Python 3.9 or later is required by the package. The documentation build uses
Python 3.11 or later and does not need the native binding. Running the examples
does need the binding and pandas; simulation studies additionally use the
dependencies described with their canonical notebooks.
