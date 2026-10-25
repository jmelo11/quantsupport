"""Market quote fixtures shared by native binding tests."""

import quantsupport as qs


def zero_rate_quotes(index, pillars, rates, currency="EUR"):
    return [{"identifier": f"ZeroRate_{currency}_{index}_{pillar}", "mid": rate}
            for pillar, rate in zip(pillars, rates)]


def append_quotes(store, observations):
    replaced = {record["identifier"] for record in observations}
    records = [{"identifier": identifier,
                **dict(zip(("mid", "bid", "ask"), store.levels(identifier)))}
               for identifier in store.identifiers() if identifier not in replaced]
    return qs.QuoteStore.from_dict({
        "reference_date": str(store.reference_date), "quotes": records + observations,
    })
