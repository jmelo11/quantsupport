"""Execute the tutorials with this interpreter, or an explicitly selected kernel."""

import argparse
from pathlib import Path
import sys

from jupyter_client import KernelManager
from nbclient import NotebookClient
import nbformat


EXAMPLES = Path(__file__).resolve().parent


def execute_notebook(path, *, kernel=None, timeout=600, output=None):
    """Execute a notebook in its directory and return the number of code cells.

    Each call uses a fresh kernel. Without ``kernel``, use this interpreter;
    otherwise use the named kernelspec. ``timeout`` is the per-cell limit in
    seconds. Write executed outputs only when an ``output`` path is supplied.
    Validation and execution errors propagate; the kernel is shut down on exit.
    """
    notebook = nbformat.read(path, as_version=4)
    nbformat.validate(notebook)
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
        client.execute()
        if output is not None:
            output.parent.mkdir(parents=True, exist_ok=True)
            nbformat.write(notebook, output)
    finally:
        if manager.has_kernel:
            manager.shutdown_kernel(now=True)
    return sum(cell.cell_type == "code" for cell in notebook.cells)


def main():
    """Run selected notebooks/scopes, optionally saving outputs as the CLI directs.

    Paths are relative to the examples directory. Omitted selectors run every
    notebook; overlapping selections are deduplicated and checkpoints skipped.
    Invalid paths terminate with an argparse error and execution failures propagate.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("notebooks", nargs="*", help="Notebook or scope-directory paths relative to bindings/python/examples")
    parser.add_argument("--kernel", help="Use this named Jupyter kernel instead of the current interpreter")
    parser.add_argument("--timeout", type=int, default=600, help="Seconds allowed per cell (default: 600)")
    output_options = parser.add_mutually_exclusive_group()
    output_options.add_argument("--write", action="store_true", help="Save executed outputs back into the notebooks")
    output_options.add_argument("--output-dir", type=Path, help="Save executed copies under this directory")
    arguments = parser.parse_args()
    paths = []
    for selector in arguments.notebooks or [""]:
        selected = (EXAMPLES / selector).resolve()
        if selected.is_dir():
            paths.extend(sorted(
                path for path in selected.rglob("*.ipynb")
                if ".ipynb_checkpoints" not in path.parts
            ))
        elif selected.is_file() and selected.suffix == ".ipynb":
            paths.append(selected)
        else:
            parser.error(f"Notebook or scope directory does not exist: {selected}")
    paths = list(dict.fromkeys(paths))
    for path in paths:
        path = path.resolve()
        if not path.is_file() or path.suffix != ".ipynb":
            parser.error(f"Notebook does not exist: {path}")
        print(f"Executing {path.name} ...", flush=True)
        output = path if arguments.write else None
        if arguments.output_dir:
            output = arguments.output_dir / path.relative_to(EXAMPLES)
        cells = execute_notebook(
            path, kernel=arguments.kernel, timeout=arguments.timeout, output=output
        )
        print(f"Passed: {path.name} ({cells} code cells)", flush=True)


if __name__ == "__main__":
    main()
