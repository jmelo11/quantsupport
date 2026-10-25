"""Execute the tutorials with this interpreter, or an explicitly selected kernel."""

import argparse
import ast
import inspect
from pathlib import Path
import sys

from jupyter_client import KernelManager
from nbclient import NotebookClient
import nbformat


EXAMPLES = Path(__file__).resolve().parent
BENCHMARK = EXAMPLES.parents[2] / "examples" / "sensitivity" / "bechmark" / "sensitivity.ipynb"


def _environment_identity():
    """Return import identities without resolving a virtualenv executable symlink."""
    from importlib.metadata import PackageNotFoundError, version
    import sys

    identity = {
        "executable": sys.executable,
        "prefix": sys.prefix,
        "python": sys.version.split()[0],
    }
    try:
        import quantsupport as qs
    except ImportError as error:
        identity["quantsupport_import_error"] = str(error)
    else:
        identity["quantsupport_file"] = qs.__file__
        try:
            identity["quantsupport_version"] = version("quantsupport")
        except PackageNotFoundError:
            identity["quantsupport_version"] = "unpackaged"
        identity["FixedRateCoupon"] = hasattr(qs, "FixedRateCoupon")
        identity["Shock"] = hasattr(qs, "Shock")
        identity["Calendar_TARGET"] = hasattr(getattr(qs, "Calendar", None), "TARGET")
    return identity


def _kernel_identity(manager):
    """Inspect a running kernel without creating a notebook cell or history entry."""
    probe = manager.blocking_client()
    probe.start_channels()
    try:
        probe.wait_for_ready(timeout=60)
        message_id = probe.execute(
            inspect.getsource(_environment_identity),
            user_expressions={"identity": "_environment_identity()"},
            silent=True,
            store_history=False,
        )
        while True:
            reply = probe.get_shell_msg(timeout=60)
            if reply.get("parent_header", {}).get("msg_id") == message_id:
                break
        content = reply["content"]
        expression = content.get("user_expressions", {}).get("identity", {})
        if content.get("status") != "ok" or expression.get("status") != "ok":
            raise RuntimeError(f"Could not inspect notebook kernel: {content}")
        return ast.literal_eval(expression["data"]["text/plain"])
    finally:
        probe.stop_channels()


def execute_notebook(path, *, kernel=None, timeout=600, output=None):
    """Execute a notebook in its directory and return the number of code cells.

    Each call uses a fresh kernel. Without ``kernel``, use this interpreter;
    otherwise use the named kernelspec. ``timeout`` is the per-cell limit in
    seconds. The default kernel's interpreter and binding imports must match
    the caller. Write executed outputs only when an ``output`` path is supplied.
    Validation and execution errors propagate; the kernel is shut down on exit.
    """
    notebook = nbformat.read(path, as_version=4)
    nbformat.validate(notebook)
    expected = _environment_identity()
    manager = KernelManager(kernel_name=kernel or "python3")
    if kernel is None:
        # A global 'python3' kernelspec may point at a different environment.
        # Use the interpreter with which the user invoked this runner.
        manager.kernel_spec.argv = [
            sys.executable, "-m", "ipykernel_launcher", "-f", "{connection_file}"
        ]
    client = NotebookClient(
        notebook, km=manager, timeout=timeout,
        resources={"metadata": {"path": str(path.parent)}},
    )
    try:
        manager.start_kernel(
            cwd=str(path.parent), extra_arguments=["--HistoryManager.hist_file=:memory:"]
        )
        actual = _kernel_identity(manager)
        if kernel is None and actual != expected:
            raise RuntimeError(
                "Notebook kernel does not match the runner environment.\n"
                f"Runner: {expected}\nKernel: {actual}"
            )
        if path.resolve() != BENCHMARK and not all(
            actual.get(name) for name in ("FixedRateCoupon", "Shock", "Calendar_TARGET")
        ):
            raise RuntimeError(
                "The kernel has a missing or stale quantsupport binding. Build/install "
                "the current checkout in the intended environment, then invoke its "
                "Python executable without resolving the virtualenv symlink.\n"
                f"Kernel: {actual}"
            )
        print(
            f"Kernel: {actual['executable']}; "
            f"quantsupport {actual.get('quantsupport_version', 'unavailable')} "
            f"at {actual.get('quantsupport_file', 'unavailable')}; "
            f"FixedRateCoupon={actual.get('FixedRateCoupon', False)}, "
            f"Shock={actual.get('Shock', False)}, "
            f"Calendar.TARGET={actual.get('Calendar_TARGET', False)}",
            flush=True,
        )
        client.execute()
        final_identity = _kernel_identity(manager)
        if final_identity != actual:
            raise RuntimeError(
                "Notebook kernel environment changed during execution.\n"
                f"Before: {actual}\nAfter: {final_identity}"
            )
        notebook.metadata["execution_environment"] = {
            "python": actual["python"],
            "quantsupport": actual.get("quantsupport_version", "unavailable"),
            "matches_runner": actual == expected,
        }
        if output is not None:
            output.parent.mkdir(parents=True, exist_ok=True)
            nbformat.write(notebook, output)
    finally:
        if client.kc is not None:
            client.kc.stop_channels()
        if manager.has_kernel:
            manager.shutdown_kernel(now=True)
    return sum(cell.cell_type == "code" for cell in notebook.cells)


def main():
    """Run selected notebooks/scopes, optionally saving outputs as the CLI directs.

    Paths are relative to the examples directory; "benchmark" selects the
    optional historical study. Omitted selectors run all binding
    studies. Overlapping selections are deduplicated and checkpoints skipped.
    Invalid paths terminate with an argparse error and execution failures propagate.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("notebooks", nargs="*", help="Notebook/scope paths relative to bindings/python/examples, or 'benchmark'")
    parser.add_argument("--include-benchmark", action="store_true", help="Also run the historical ORE/CurveEngine benchmark (optional dependencies)")
    parser.add_argument("--kernel", help="Use this named Jupyter kernel instead of the current interpreter")
    parser.add_argument("--timeout", type=int, default=600, help="Seconds allowed per cell (default: 600)")
    output_options = parser.add_mutually_exclusive_group()
    output_options.add_argument("--write", action="store_true", help="Save executed outputs back into the notebooks")
    output_options.add_argument("--output-dir", type=Path, help="Save executed copies under this directory")
    arguments = parser.parse_args()
    if arguments.timeout <= 0:
        parser.error("--timeout must be a positive number of seconds")
    paths = []
    for selector in arguments.notebooks or [""]:
        selected = BENCHMARK if selector == "benchmark" else (EXAMPLES / selector).resolve()
        if selected != BENCHMARK and not selected.is_relative_to(EXAMPLES):
            parser.error("Select a notebook inside the Python examples, or use 'benchmark'")
        if selected.is_dir():
            paths.extend(sorted(
                path for path in selected.rglob("*.ipynb")
                if ".ipynb_checkpoints" not in path.parts
            ))
        elif selected.is_file() and selected.suffix == ".ipynb":
            paths.append(selected)
        else:
            parser.error(f"Notebook or scope directory does not exist: {selected}")
    if arguments.include_benchmark:
        paths.append(BENCHMARK)
    paths = list(dict.fromkeys(path.resolve() for path in paths))
    for path in paths:
        path = path.resolve()
        if not path.is_file() or path.suffix != ".ipynb":
            parser.error(f"Notebook does not exist: {path}")
        label = "benchmark/sensitivity.ipynb" if path == BENCHMARK else str(path.relative_to(EXAMPLES))
        print(f"Executing {label} ...", flush=True)
        output = path if arguments.write else None
        if arguments.output_dir:
            relative = Path("benchmark/sensitivity.ipynb") if path == BENCHMARK else path.relative_to(EXAMPLES)
            output = arguments.output_dir / relative
        cells = execute_notebook(
            path, kernel=arguments.kernel, timeout=arguments.timeout, output=output
        )
        print(f"Passed: {label} ({cells} code cells)", flush=True)
    print(f"Passed {len(paths)} notebook(s).", flush=True)


if __name__ == "__main__":
    main()
