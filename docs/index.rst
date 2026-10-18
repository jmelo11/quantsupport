QuantSupport documentation
==========================

QuantSupport (QS) is a quantitative finance library written in Rust, with
Python and Excel interfaces. It supports market construction, instrument
valuation, automatic differentiation, simulation and valuation adjustments.
These guides explain how to perform those calculations through each binding,
from defining a trade and its market inputs to interpreting the results.

.. toctree::
   :maxdepth: 2

   Python <python/index>
   Excel <excel/index>

Start with the :doc:`Python user guide <python/guide/index>` or
:doc:`Excel user guide <excel/guide/index>`. The
:doc:`computational studies <python/studies/index>` investigate curve
construction, valuation, simulation, and risk using the official notebooks.

The :doc:`Python reference <autoapi/quantsupport/index>` and
:doc:`Excel reference <excel/functions/index>` describe exact callable
interfaces: signatures, parameter meanings and units, defaults, result schemas,
and errors. They are generated from the packaged Python stubs and the Excel
registration metadata.

The `QuantSupport Book <https://jmelo11.github.io/quantsupport/>`_ develops the
library's financial concepts and architecture across all three interfaces.
The `Rust reference <https://docs.rs/quantsupport>`_ documents the native API.

.. rubric:: Indexes

* :ref:`genindex`
* :ref:`py-modindex`
* :ref:`excel-functionindex`
* :ref:`search`
