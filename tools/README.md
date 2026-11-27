# Repository tooling

Run maintenance commands from the repository root. Scripts live here by purpose;
their Python tests live in `tools/tests/`.

| Directory | Purpose |
| --- | --- |
| `book/` | Validate the Book's runnable Rust examples. |
| `benchmarks/` | Compare old/new implementations with matched pricing requests, fixtures, dependencies and scaling workloads. |
| `release/` | Reserve release versions and stage published Excel/Python downloads. |
| `python/` | Package a Python wheel as an extracted-folder distribution. |
| `excel/` | Build and package the Windows Excel add-ins. |
| `excel/examples/` | Generate workbooks, migrate formulas, automate Excel and run benchmarks. |

```sh
python -m unittest discover -s tools/tests -v
RUSTUP_TOOLCHAIN=stable python tools/book/check_rust.py
python tools/python/package_portable.py --wheel-directory dist --output dist
python tools/excel/examples/create_examples.py
```

The Book runner requires Cargo and mdBook. Workbook generators use
`bindings/excel/examples/requirements.txt` and write to
`bindings/excel/examples/workbooks/` by default. Windows automation and benchmarks
also need `bindings/excel/examples/requirements-live.txt` and desktop Excel.

The AD scaling runner requires Python 3.11 or later and the stable Rust
toolchain. See the [benchmark guide](../docs/benchmarks.md) for
isolated-worktree setup, request/portfolio scaling and serial/parallel path
comparisons. Its `--smoke` mode checks the harness; measured runs use multiple
fresh processes and compare complete numerical snapshots.

Build and package the add-ins on Windows with Rust and the .NET SDK installed:

```powershell
./tools/excel/build.ps1 -Profile release
```

The default build output remains `bindings/excel/dist/`. See the binding and Book
READMEs for prerequisites and additional validation commands. CI workflows call
these same tools and discover their tests separately.

`benchmarks/prepared.py` compares explicit prepared DCF snapshots with the old
ordinary tape API, recording cold preparation, warm evaluation and changed-market
refresh separately. It uses the scaling runner's prepared baseline fixtures and
requires separate build targets. See the prepared DCF section in
[benchmark guide](../docs/benchmarks.md#prepared-dcf)
for the full command and the interpretation of snapshot reuse.
