"""Package only QuantSupport for use from an extracted folder."""

import argparse
from email.parser import Parser
import json
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
from zipfile import ZIP_DEFLATED, ZipFile


SMOKE_TEST = '''
from pathlib import Path
import sys

sys.path.insert(0, str(Path(sys.argv[1]) / "site-packages"))

import quantsupport as qs
import pandas as pd

packages = (Path(sys.argv[1]) / "site-packages").resolve()
assert Path(qs.__file__).resolve().is_relative_to(packages), qs.__file__
assert not Path(pd.__file__).resolve().is_relative_to(packages), pd.__file__

reference = qs.Date(2026, 2, 24)
quotes = qs.QuoteStore.from_dict({
    "reference_date": str(reference), "quotes": [
        {"identifier": f"ZeroRate_USD_SOFR_{tenor}", "mid": 0.03}
        for tenor in ["0D", "10Y"]
    ],
})
curve = qs.CurveConfiguration(
    market_index="SOFR", day_counter="Actual360", interpolator="Linear",
    quotes=quotes.identifiers(),
)
context = qs.PricingContext(
    quotes, [curve], csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
)
swap = qs.Swap("example", reference, reference + "5Y", 1000000, 0.03, "USD", "SOFR")
with context:
    result = context.evaluate(qs.Trade(swap, reference, qs.Side.LongReceive), ["Value"])
    print(pd.DataFrame({"PV": [result.price]}))
print("QuantSupport loaded from", qs.__file__)
'''


README = '''QuantSupport extracted folder package

This ZIP contains only QuantSupport. Third-party dependencies are not included.
Use the Python version, operating system and architecture named in the ZIP.

Install the runtime dependencies into your Python environment:

    python -m pip install pandas

The complete dependency declarations remain in site-packages/quantsupport-*.dist-info/METADATA.

Extract the ZIP and add its site-packages directory before importing QuantSupport:

    import sys
    sys.path.insert(0, "/path/to/extracted-folder/site-packages")
    import quantsupport as qs
'''


def package(wheel_directory, output):
    wheels = list(Path(wheel_directory).glob("quantsupport-*.whl"))
    if len(wheels) != 1:
        raise ValueError(f"Expected exactly one QuantSupport wheel, found {len(wheels)}")
    wheel = wheels[0].resolve()
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    archive_path = output / f"{wheel.stem}-portable.zip"
    with TemporaryDirectory(prefix="quantsupport-portable-") as temporary:
        bundle = Path(temporary) / "bundle"
        packages = bundle / "site-packages"
        arguments = [
            sys.executable, "-m", "pip", "install", "--no-deps", "--no-index",
            "--no-compile", "--target", str(packages), str(wheel),
        ]
        subprocess.run(arguments, check=True)
        versions = {}
        requirements = []
        for metadata in sorted(packages.glob("*.dist-info/METADATA")):
            info = Parser().parsestr(metadata.read_text(encoding="utf-8"))
            versions[info["Name"]] = info["Version"]
            requirements.extend(info.get_all("Requires-Dist", []))
        (bundle / "BUILD-INFO.json").write_text(json.dumps({
            "wheel": wheel.name, "python": sys.version, "packages": versions,
            "requires_dist": requirements,
        }, indent=2) + "\n", encoding="utf-8")
        (bundle / "README.txt").write_text(README, encoding="utf-8")
        # Console entry points have builder-specific shebangs; this bundle is imported.
        shutil.rmtree(packages / "bin", ignore_errors=True)
        with ZipFile(archive_path, "w", compression=ZIP_DEFLATED) as archive:
            for path in sorted(bundle.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(bundle))
        # Load QuantSupport from the relocated ZIP and dependencies from the environment.
        relocated = Path(temporary) / "extracted folder"
        with ZipFile(archive_path) as archive:
            archive.extractall(relocated)
        subprocess.run([sys.executable, "-I", "-c", SMOKE_TEST, str(relocated)],
                       cwd=relocated, check=True)
    return archive_path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wheel-directory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(package(args.wheel_directory, args.output))


if __name__ == "__main__":
    main()
