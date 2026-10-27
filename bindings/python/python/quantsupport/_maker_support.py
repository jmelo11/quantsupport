"""Named maker factories installed in the native module during initialization."""


def _instrument_maker(name, product, make_instrument):
    def __new__(cls, **parameters):
        return make_instrument.__new__(cls, product, parameters)
    def __init__(self, **parameters):
        pass
    @classmethod
    def from_dict(cls, parameters):
        return cls(**parameters)
    return type(name, (make_instrument,), {
        "__new__": __new__, "__init__": __init__,
        "from_dict": from_dict,
        "__module__": "quantsupport",
        "_quantsupport_constructor": True,
        "__doc__": f"Reusable maker for {product.__name__}; check is_resolved(), then call build() or build_using(context).",
    })

def _initialize_makers(namespace):
    """Create named makers from the registered native product types."""
    products = {
        "MakeSwap": "Swap",
        "MakeFloatFloatCrossCurrencySwap": "CrossCurrencySwap",
        "MakeCrossCurrencySwap": "CrossCurrencySwap",
        "MakeBasisSwap": "BasisSwap",
        "MakeFixFloatCrossCurrencySwap": "FixFloatCrossCurrencySwap",
        "MakeFixedRateBond": "FixedRateBond",
        "MakeSteppedCouponBond": "SteppedCouponBond",
        "MakeFloatingRateNote": "FloatingRateNote",
        "MakeFixedRateDeposit": "FixedRateDeposit",
        "MakeLoan": "Loan",
        "MakeFxForward": "FxForward",
        "MakeFxEuropeanOption": "FxOption",
        "MakeFxOption": "FxOption",
        "MakeEquityEuropeanOption": "EquityOption",
        "MakeEquityOption": "EquityOption",
        "MakeCreditDefaultSwap": "CreditDefaultSwap",
        "MakeCapFloor": "CapFloor",
        "MakeCapletFloorlet": "CapletFloorlet",
        "MakeRateFutures": "RateFutures",
        "MakeSwaption": "EuropeanSwaption",
        "MakeEuropeanSwaption": "EuropeanSwaption",
        "MakeEquityForward": "EquityForward",
        "MakeScriptedProduct": "ScriptedProduct",
        "MakeTrade": "Trade",
    }
    return {
        name: _instrument_maker(name, namespace[product], namespace["MakeInstrument"])
        for name, product in products.items()
    }
