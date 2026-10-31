"""Run the Book's Rust examples against Cargo's selected library artifacts."""

import json
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[2]


def main():
    build = subprocess.run(
        ["cargo", "build", "-p", "quantsupport", "--message-format=json"],
        cwd=ROOT, check=True, text=True, stdout=subprocess.PIPE,
    )
    with tempfile.TemporaryDirectory(prefix="quantsupport-book-") as directory:
        libraries = Path(directory)
        for line in build.stdout.splitlines():
            artifact = json.loads(line)
            if artifact.get("reason") != "compiler-artifact":
                continue
            files = [Path(filename) for filename in artifact["filenames"]]
            metadata = next((path for path in files if path.suffix == ".rmeta"), None)
            for library in files:
                if library.suffix not in {".rlib", ".rmeta", ".so", ".dylib", ".dll"}:
                    continue
                # Keep the rlib and its metadata under the same crate filename.
                name = library.name
                if library.suffix == ".rlib" and metadata:
                    name = metadata.with_suffix(".rlib").name
                (libraries / name).symlink_to(library)
        return subprocess.run(
            ["mdbook", "test", "-L", str(libraries)], cwd=ROOT,
        ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
