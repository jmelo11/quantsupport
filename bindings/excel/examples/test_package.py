"""Exercise the real PowerShell ZIP packager with isolated build fixtures."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import unittest
from urllib.request import urlopen
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[3]
EXCEL = ROOT / "bindings/excel"
ARCHIVE = "quantsupport-excel-windows-x64.zip"
COMPANION = "ExcelDna.IntelliSense64.xll"


class DistributionTests(unittest.TestCase):
    def test_release_artifact_and_website_include_the_download_bundle(self):
        bindings = (ROOT / ".github/workflows/bindings.yml").read_text()
        pages = (ROOT / ".github/workflows/pages.yml").read_text()
        release = (ROOT / ".github/workflows/release.yml").read_text()
        for name in (ARCHIVE, COMPANION, "quantsupport.xll", "quantsupport.intellisense.xml", "QuantSupport.Ribbon64.xll"):
            self.assertIn("bindings/excel/dist/" + name, bindings)
            self.assertIn('"' + name + '"', (ROOT / ".github/scripts/excel_downloads.py").read_text())
        self.assertIn('"$RUNNER_TEMP/excel_downloads.py" stage', pages)
        self.assertIn("gh release upload", release)
        self.assertIn("dist/*", release)
        self.assertIn('"package.ps1"', (EXCEL / "scripts/build.ps1").read_text())


class PackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.powershell = os.environ.get("QS_PWSH_PATH") or shutil.which("pwsh") or shutil.which("powershell")
        if not cls.powershell:
            raise unittest.SkipTest("PowerShell is required to execute the ZIP packager")
        cls.dependency = json.loads((EXCEL / "third-party/intellisense.json").read_text())
        with urlopen(cls.dependency["url"], timeout=60) as response:
            cls.companion = response.read()
        if hashlib.sha256(cls.companion).hexdigest() != cls.dependency["sha256"]:
            raise AssertionError("The pinned upstream companion digest changed")

    def setUp(self):
        directory = TemporaryDirectory(prefix="Excel package ")
        self.addCleanup(directory.cleanup)
        self.dist = Path(directory.name)
        self.xll = b"Isolated QuantSupport build fixture, never published"
        self.xml = b'<?xml version="1.0"?><IntelliSense xmlns="http://schemas.excel-dna.net/intellisense/1.0"/>'
        (self.dist / "quantsupport.xll").write_bytes(self.xll)
        (self.dist / "quantsupport.intellisense.xml").write_bytes(self.xml)
        self.ribbon = b"Isolated packed ribbon fixture, never published"
        (self.dist / "QuantSupport.Ribbon64.xll").write_bytes(self.ribbon)

    def package(self, *arguments):
        return subprocess.run([self.powershell, "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                               str(EXCEL / "scripts/package.ps1"), "-Dist", str(self.dist), *map(str, arguments)],
                              text=True, capture_output=True, timeout=90)

    def test_bundle_downloads_and_preserves_all_addin_files_at_zip_root(self):
        result = self.package()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        with ZipFile(self.dist / ARCHIVE) as archive:
            self.assertIsNone(archive.testzip())
            self.assertEqual(set(archive.namelist()), {"quantsupport.xll", "quantsupport.intellisense.xml", COMPANION, "QuantSupport.Ribbon64.xll",
                                                      "INSTALL.txt", "QuantSupport-LICENSE.txt",
                                                      "ExcelDna.IntelliSense-LICENSE.txt", "ExcelDna-LICENSE.txt"})
            self.assertEqual(archive.read("quantsupport.xll"), self.xll)
            self.assertEqual(archive.read("quantsupport.intellisense.xml"), self.xml)
            self.assertEqual(archive.read(COMPANION), self.companion)
            self.assertEqual(archive.read("QuantSupport.Ribbon64.xll"), self.ribbon)
            instructions = archive.read("INSTALL.txt").decode()
            for name in ("quantsupport.xll", "quantsupport.intellisense.xml", COMPANION, "QuantSupport.Ribbon64.xll"):
                self.assertIn(name, instructions)
            self.assertEqual(archive.read("ExcelDna.IntelliSense-LICENSE.txt"),
                             (EXCEL / "third-party/ExcelDna.IntelliSense-LICENSE.txt").read_bytes())
        self.assertEqual((self.dist / COMPANION).read_bytes(), self.companion)

    def test_changed_companion_is_rejected_before_replacing_an_existing_bundle(self):
        companion = self.dist / "changed.xll"
        companion.write_bytes(self.companion + b"changed")
        existing = self.dist / ARCHIVE
        existing.write_bytes(b"previous bundle")
        result = self.package("-IntelliSensePath", companion)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum mismatch", result.stdout + result.stderr)
        self.assertEqual(existing.read_bytes(), b"previous bundle")
        self.assertFalse((self.dist / COMPANION).exists())

    def test_missing_matching_metadata_prevents_packaging(self):
        (self.dist / "quantsupport.intellisense.xml").unlink()
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing build output", result.stdout + result.stderr)
        self.assertFalse((self.dist / ARCHIVE).exists())

    def test_missing_ribbon_prevents_replacing_an_existing_bundle(self):
        (self.dist / "QuantSupport.Ribbon64.xll").unlink()
        (self.dist / ARCHIVE).write_bytes(b"previous complete bundle")
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("QuantSupport.Ribbon64.xll", result.stdout + result.stderr)
        self.assertEqual((self.dist / ARCHIVE).read_bytes(), b"previous complete bundle")


if __name__ == "__main__":
    unittest.main()
