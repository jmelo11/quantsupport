# Historical pricing

Historical pricing evaluates a trade against a sequence of recorded markets.
The Excel workflow uses the same native snapshot archive and selection rules
as Python. Each snapshot records the observations and construction inputs for
one market instant.

Load the XLL before opening [the example
workbook](https://jmelo11.github.io/quantsupport/downloads/historical_pricing_example.xlsx),
then press **Ctrl+Alt+F9** to reconstruct its objects. Regenerate both Excel
examples with `python bindings/excel/examples/scripts/create_examples.py` after
installing `bindings/excel/examples/requirements.txt`. The historical workbook
uses explicit zero rates; the same snapshot workflow accepts bootstrapped
markets, volatility, FX, simulation, and model configurations.

## Capture each market

Build a separate context for each market reference date with the usual
`QS.QUOTES.*`, curve configurations, and `QS.CONTEXT.CREATE`. Give every
constructor a workbook-unique name. Store the context handles in `B12` and
`C12`, and capture their observations:

```excel
B13 =QS.SNAPSHOT.FROM.CONTEXT("history_snapshot_20260115","2026-01-15T16:00:00Z",B12)
C13 =QS.SNAPSHOT.FROM.CONTEXT("history_snapshot_20260116","2026-01-16T16:00:00Z",C12)
B15 =QS.SNAPSHOTS.CREATE("history_archive",B13:C13)
```

The archive is reconstructed from the **full range**. Changes to market inputs
advance snapshot and archive handles; deleting a row removes that observation.
Blank range cells are skipped. Duplicate instants, including equivalent timezone
offsets, are errors. A missing or blank scalar input creates an empty archive.
Unchanged recalculation reuses current handles without reconstructing markets.

Each snapshot contains one complete set of native construction inputs. To use
curves already supplied as zero rates in a sheet, place ISO date text or tenor
text in `A5:A7` and annual decimal rates in `B5:B7`:

```excel
B10 =QS.CURVE.CONFIGS.CREATE("history_curves_20260115")
B11 =QS.CURVE.CONFIGS.ADD.ZERO(B10,"SOFR","Actual365","Linear",FALSE,A5:A7,B5:B7,"Continuous","Annual")
```

Pass `B11` as the curve configurations when creating the context. The pillar
and rate ranges must have equal counts; rates must be finite numbers. Pillars
must resolve in increasing date order. Composite construction is also available:

```excel
=QS.CURVE.CONFIGS.ADD.COMPOSITE(configurations,"TermSOFR6m","SOFR","TermSOFR3m")
```

The two component curves must be configured in the same market. Direct
`QS.CURVE.FLAT` / constructed-element objects are runtime values and cannot be
captured. An explicit zero curve with one `0D` pillar and extrapolation enabled
provides serializable flat-curve construction inputs.

## Select and price

Keep the trade handle in `B17`. Create one selector/context pair for each
requested timestamp and reuse the same trade:

```excel
B20 =QS.SNAPSHOT.SELECTOR("history_selector_20260115","2026-01-15T16:00:00Z","Exact","Mid")
B21 =QS.CONTEXT.FROM.SNAPSHOT("history_selected_20260115",$B$15,B20,)
B22 =QS.CONTEXT.SNAPSHOT.TIMESTAMP(B21)
B23 =QS.CONTEXT.EVALUATION.DATE(B21)
B31 =QS.DISCOUNT.POLICY.CSA("history_policy","USD","SOFR")
B32 =QS.PRICER.CREATE("history_pricer","DiscountedCashflowPricer",B31)
B33 =QS.EVALUATE("history_results",B32,$B$17,B21,"Value")
B24 =QS.RESULT.VALUE(B33)
```

The optional fourth context argument is a `FixingStore` handle. Fixings are
supplied separately, without publication-vintage filtering. Use the correct
historical fixing dataset when publication times or revisions affect a result.

| Selector rule | Behavior |
| --- | --- |
| `Exact` | Requires that exact UTC instant. |
| `Previous` (default) | Latest snapshot at or before the request. |
| `Closest` | Nearest snapshot; ties prefer the earlier instant; may select future data. |

The default quote side is `Mid`; `Bid` and `Ask` rebuild quoted construction and
calibration inputs at that side. A consumed missing side raises an error. FX
spots are side-independent; zero-rate quotes use the selected level.

Selection preserves the matched snapshot's **market reference date** as the
valuation date. Requesting Monday with `Previous` and matching Friday prices at
Friday's market date; it does not roll curves to Monday. The observation instant
and market business date are distinct, including across UTC midnight. Inspect
both context properties when displaying historical results.

`QS.SNAPSHOTS.SELECT(name,store,selector)` returns the matched snapshot handle
without constructing a context. Inspect it with `QS.SNAPSHOT.TIMESTAMP` and
`QS.SNAPSHOT.REFERENCE.DATE`. `QS.SNAPSHOTS.TIMESTAMPS(store)` lists available
instants, while `QS.CONTEXT.QUOTE.LEVEL(context)` reports the construction side.
`QS.DATETIME.PARSE` normalizes offset-aware timestamp text to UTC;
`QS.DATETIME.DATE` returns its UTC calendar day. Neither accepts naive timestamps
or Excel serial datetimes.

## Python and Rust interoperability

`QS.SNAPSHOTS.FROM.JSON(name,json_text)` imports the same native snapshot-array
schema saved by Python's `MarketSnapshotStore.to_json`.
`QS.SNAPSHOTS.TO.JSON(store)` returns JSON text, excluding fixing histories,
constructed curves, and AD state. These functions exchange JSON as worksheet
text. Export fails above Excel's 32767 UTF-16-unit cell limit.

The archive captures base market configurations. Materialized scenario contexts
with supplied runtime elements are rejected; capture the original base context.
The [workbook chapter](workbook.md) relates these handles to ordinary valuation
and recalculation. The [worksheet
reference](https://jmelo11.github.io/quantsupport/api/excel/functions/index.html)
provides exact argument lists.
