"""Folder ZIPs contain only QuantSupport and leave dependencies to the user."""

import json
import os
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools/python"))

import package_portable as portable


class PortablePackageTests(unittest.TestCase):
    def test_builds_offline_without_resolving_or_bundling_dependencies(self):
        metadata = (
            "Metadata-Version: 2.1\nName: quantsupport\nVersion: 0.1.12\n"
            "Requires-Dist: dependency-that-must-not-be-bundled>=1\n\n"
        )
        smoke = '''
from pathlib import Path
import sys
packages = Path(sys.argv[1]) / "site-packages"
sys.path.insert(0, str(packages))
import quantsupport
assert quantsupport.VALUE == 42
assert Path(quantsupport.__file__).resolve().is_relative_to(packages.resolve())
'''
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheel = root / "quantsupport-0.1.12-py3-none-any.whl"
            info = "quantsupport-0.1.12.dist-info"
            with ZipFile(wheel, "w") as archive:
                archive.writestr("quantsupport/__init__.py", "VALUE = 42\n")
                archive.writestr(f"{info}/METADATA", metadata)
                archive.writestr(f"{info}/WHEEL", (
                    "Wheel-Version: 1.0\nGenerator: test\n"
                    "Root-Is-Purelib: true\nTag: py3-none-any\n\n"
                ))
                archive.writestr(f"{info}/RECORD", "")
            with patch.dict(os.environ, {
                "PIP_NO_INDEX": "1", "PIP_DISABLE_PIP_VERSION_CHECK": "1",
            }), patch.object(portable, "SMOKE_TEST", smoke):
                result = portable.package(root, root / "output")
            with ZipFile(result) as archive:
                names = archive.namelist()
                self.assertEqual({name.split("/")[0] for name in names}, {
                    "site-packages", "BUILD-INFO.json", "README.txt",
                })
                self.assertEqual({name.split("/")[1] for name in names
                                  if name.startswith("site-packages/")}, {
                    "quantsupport", info,
                })
                self.assertFalse(any(name.endswith(".pyc") for name in names))
                self.assertEqual(
                    archive.read(f"site-packages/{info}/METADATA").decode(), metadata,
                )
                build = json.loads(archive.read("BUILD-INFO.json"))
                self.assertEqual(build["packages"], {"quantsupport": "0.1.12"})
                self.assertEqual(build["requires_dist"], [
                    "dependency-that-must-not-be-bundled>=1",
                ])
                self.assertIn("python -m pip install pandas",
                              archive.read("README.txt").decode())


if __name__ == "__main__":
    unittest.main()
