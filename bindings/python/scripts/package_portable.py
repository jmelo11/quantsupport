"""Bundle a wheel and its binary dependencies for use from an extracted folder."""

import argparse
from email.parser import Parser
import json
from pathlib import Path
import re
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

reference = qs.Date(2026, 2, 24)
quotes = qs.QuoteStore.from_dict({"reference_date": str(reference), "quotes": []})
curve = qs.CurveConfiguration(
    market_index="SOFR", day_counter="Actual360", interpolator="Linear",
    construction=qs.CurveConstruction.zero_rate(
        pillars=["0D", "10Y"], rates=[0.03, 0.03],
    ),
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

def compatible_platforms(wheel):
    """Resolve dependencies for the wheel's minimum OS, not the newer CI host."""
    platforms = set(Path(wheel).stem.rsplit("-", 1)[1].split("."))
    for platform in list(platforms):
        linux = re.fullmatch(r"manylinux_2_(\d+)_(x86_64|aarch64)", platform)
        mac = re.fullmatch(r"macosx_(\d+)_(\d+)_(x86_64|arm64)", platform)
        if linux:
            minor, arch = int(linux[1]), linux[2]
            platforms.update(f"manylinux_2_{i}_{arch}" for i in range(5, minor + 1))
            for baseline, alias in ((17, "manylinux2014"), (12, "manylinux2010"), (5, "manylinux1")):
                if minor >= baseline and (arch == "x86_64" or baseline == 17):
                    platforms.add(f"{alias}_{arch}")
        elif mac:
            major, minor, arch = int(mac[1]), int(mac[2]), mac[3]
            versions = [(i, 0) for i in range(11, major + 1)]
            versions += [(10, i) for i in range(4, (minor if major == 10 else 16) + 1)]
            for target_major, target_minor in versions:
                platforms.add(f"macosx_{target_major}_{target_minor}_universal2")
                if arch == "x86_64" or target_major >= 11:
                    platforms.add(f"macosx_{target_major}_{target_minor}_{arch}")
    return sorted(platforms)


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
            sys.executable, "-m", "pip", "install", "--only-binary=:all:",
            "--no-compile", "--target", str(packages), str(wheel),
        ]
        for platform in compatible_platforms(wheel):
            arguments.extend(["--platform", platform])
        subprocess.run(arguments, check=True)
        versions = {}
        for metadata in sorted(packages.glob("*.dist-info/METADATA")):
            info = Parser().parsestr(metadata.read_text(encoding="utf-8"))
            versions[info["Name"]] = info["Version"]
        (bundle / "BUILD-INFO.json").write_text(json.dumps({
            "wheel": wheel.name, "python": sys.version, "dependencies": versions,
        }, indent=2) + "\n", encoding="utf-8")
        # Console entry points have builder-specific shebangs; this bundle is imported.
        shutil.rmtree(packages / "bin", ignore_errors=True)
        with ZipFile(archive_path, "w", compression=ZIP_DEFLATED) as archive:
            for path in sorted(bundle.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(bundle))
        # Verify the delivered ZIP after relocation, without any installed packages.
        relocated = Path(temporary) / "extracted folder"
        with ZipFile(archive_path) as archive:
            archive.extractall(relocated)
        subprocess.run([sys.executable, "-I", "-S", "-c", SMOKE_TEST, str(relocated)],
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
