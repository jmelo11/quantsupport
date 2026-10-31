# Large-workbook pane responsiveness

Opening Pricing previously paused Excel while native dropdown windows inserted
20,000 instruments/trades and 10,000 dated contexts. Browser JSON was also decoded
synchronously. The selectors now use searchable virtual grids; snapshot decoding
runs in the background, and matching refresh requests share one pending load.

Measured in desktop Excel after pricing the same synthetic 10,000-instrument,
10,000-origination-date batch workbook used by the pricing benchmark. The browser
contains 40,000 objects. [Raw timings and binary hashes](ribbon.json) accompany
this report.

| Operation | Before | After |
| --- | ---: | ---: |
| Capture and decode browser snapshot on Excel's UI thread | 1,321 ms | 325 ms capture; 888 ms decoding in background |
| First Pricing tab activation, with snapshot already loaded | 2,889 ms | 227 ms |
| First Curves tab activation, with snapshot already loaded | 918 ms | 72 ms |
| Return to Objects | 38 ms | 38 ms |

The separate refresh regression exercises the actual asynchronous pane flow:

- First open returns in **786 ms**, including control construction and capture.
  The selected trade is displayed after **1.87 s** total; Excel can process input
  while decoding runs.
- Refresh with an unchanged snapshot completes in **53 ms**. A fresh capture
  returns in **333 ms**, followed by background decoding.
- The full 20,000-object pricing selector opens in **39 ms**; searching for the
  last trade takes **11 ms**. The 10,000-context selector opens in **25 ms**, and
  searching for the last context takes **8 ms**.
- Maximum COM heartbeat latency during the refresh/lifecycle sequence is
  **489 ms**. Native capture and control creation still produce short UI pauses.
  The profile that deliberately switches all three tabs in one macro has a
  separate **760 ms** maximum; it combines work normally triggered separately.

The regression also checks all 10,000 PVs are finite, all contexts and curves
were built, no pricing jobs are submitted by browsing, and Manual calculation
mode and the RTD throttle remain intact. It verifies deferred inspection,
selection retention, empty/cleared searches, Enter/Escape, outside-click cleanup,
coalesced refreshes, and discarding loads after invalidation or disposal.

Broader checks exposed a ready-price delivery race: a recalculation could return
`#N/A` while unchanged definitions were revalidated, leaving the old generation
marked as delivered. The native bridge now requests delivery whenever the UDF
returns pending. A regression repeats definition revalidation five times and
checks the original price is delivered without submitting additional pricing jobs.

All **20 desktop Excel tests**, **123 native tests** (one standalone benchmark
ignored), managed contracts and ribbon layout smoke checks passed. The optional
probe commands are excluded from the production companion.

To reproduce the desktop checks, build the native add-in and a probe companion
into a separate directory, then run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/excel/build.ps1 -Dist target/ribbon-check
dotnet build bindings/excel/ribbon/QuantSupport.Ribbon.csproj -c Release -p:QuantSupportLiveTests=true
Copy-Item bindings/excel/ribbon/bin/Release/net462/publish/QuantSupport.Ribbon64.xll target/ribbon-check -Force
$env:QS_XLL_PATH = (Resolve-Path target/ribbon-check/quantsupport.xll).Path
$env:QS_PROBE_XLL = (Resolve-Path target/ribbon-check/QuantSupport.Ribbon64.xll).Path
python -m unittest discover -s bindings/excel/examples/tests -p '*live.py' -v
```

The tests create their own Excel instances and generate temporary workbooks.
`QS.PanelProbe.ProfileBrowser` provides the tab-stage profile;
`QS.PanelProbe.RefreshLargeBrowser` provides the refresh measurements recorded
by the large-workbook test. Use a fresh output directory if an earlier Excel
instance still has an XLL mapped.
