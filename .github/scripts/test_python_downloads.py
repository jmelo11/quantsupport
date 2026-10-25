"""Release selection, hosted assets and a standalone Python package page."""

import json
from pathlib import Path
import re
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import python_downloads as downloads


TARGETS = (
    "win_amd64", "win_arm64", "macosx_11_0_arm64",
    "manylinux_2_28_x86_64", "manylinux_2_28_aarch64",
)


def release(tag, portable=True):
    names = []
    for target in TARGETS:
        for python in downloads.PYTHONS:
            abi = "cp" + python.replace(".", "")
            stem = f"quantsupport-{tag[1:]}-{abi}-{abi}-{target}"
            names.append(stem + ".whl")
            if portable:
                names.append(stem + "-portable.zip")
    return {"tagName": tag, "assets": [
        {"name": name, "size": 42, "state": "uploaded"} for name in names]}


class PythonDownloadTests(unittest.TestCase):
    def setUp(self):
        self.releases = {
            "v0.1.9": release("v0.1.9", portable=False),
            "v0.1.10": release("v0.1.10"),
            "v0.1.11": release("v0.1.11"),
        }
        self.downloaded = []
        self.missing_download = None

    def command(self, *args):
        if args[:3] == ("gh", "release", "view"):
            return json.dumps(self.releases[args[3]])
        if args[:3] == ("gh", "release", "download"):
            self.downloaded.append(args[3])
            destination = Path(args[args.index("--dir") + 1])
            for asset in self.releases[args[3]]["assets"]:
                if asset["name"] != self.missing_download:
                    (destination / asset["name"]).write_bytes(b"fixture")
            return ""
        self.assertEqual(args[:3], ("gh", "release", "list"))
        self.assertIn("--exclude-drafts", args)
        self.assertIn("--exclude-pre-releases", args)
        return json.dumps([{"tagName": tag} for tag in self.releases])

    def select(self, tag="v0.1.10"):
        with patch.object(downloads, "command", side_effect=self.command):
            return downloads.select("owner/repo", tag)

    def test_all_20_combinations_have_wheel_and_folder_links(self):
        selected = self.select()
        entries = selected[0][1]
        self.assertEqual(len(entries), 20)
        for os, arch in downloads.PLATFORMS:
            for python in downloads.PYTHONS:
                self.assertTrue(entries[os, arch, python]["portable"])
        html = downloads.render("owner/repo", selected[:1])
        self.assertEqual(html.count(">Folder ZIP</a>"), 20)
        self.assertEqual(html.count(">Wheel</a>"), 20)
        self.assertIn("win_arm64-portable.zip", html)
        self.assertIn("macosx_11_0_arm64.whl", html)
        self.assertIn("manylinux_2_28_aarch64.whl", html)

    def test_selection_stays_on_deployed_release_and_orders_versions_numerically(self):
        self.assertEqual([tag for tag, _ in self.select()], ["v0.1.10", "v0.1.9"])
        self.assertEqual([tag for tag, _ in self.select("v0.1.11")], ["v0.1.11", "v0.1.10"])

    def test_legacy_release_shows_only_uploaded_wheels(self):
        prior = self.releases["v0.1.9"]
        prior["assets"] = prior["assets"][:1]
        html = downloads.render("owner/repo", self.select())
        previous = html.split('<h2 id="v0.1.9">')[1]
        self.assertNotIn("Folder ZIP</a>", previous)
        self.assertEqual(previous.count(">Wheel</a>"), 1)
        self.assertIn("Unavailable", previous)

    def test_incomplete_selected_release_fails_before_page_is_written(self):
        self.releases["v0.1.10"]["assets"][1]["state"] = "new"
        with TemporaryDirectory() as temporary:
            page = Path(temporary) / "downloads.html"
            source = "<html>Existing package page</html>"
            page.write_text(source)
            with patch.object(downloads, "command", side_effect=self.command):
                with self.assertRaisesRegex(RuntimeError, "missing Python wheels or folder ZIPs"):
                    downloads.stage("owner/repo", "v0.1.10", page, Path(temporary) / "packages")
            self.assertEqual(page.read_text(), source)
            self.assertEqual(self.downloaded, [])

    def test_empty_asset_cannot_produce_a_download_link(self):
        self.releases["v0.1.10"]["assets"][0]["size"] = 0
        with self.assertRaisesRegex(RuntimeError, "missing Python wheels or folder ZIPs"):
            self.select()

    def test_no_supported_predecessor_is_allowed(self):
        self.releases["v0.1.9"]["assets"] = []
        self.assertEqual(len(self.select()), 1)

    def test_unrelated_versions_abis_and_platforms_are_not_offered(self):
        names = [
            "quantsupport-0.1.10-cp310-cp310-win_amd64.whl",
            "quantsupport-0.1.10-cp314-cp314t-win_amd64.whl",
            "quantsupport-0.1.10-cp311-cp311-linux_x86_64.whl",
            "quantsupport-0.1.10-cp311-cp311-macosx_10_15_x86_64.whl",
            "quantsupport-0.1.10-cp311-cp311-macosx_10_15_x86_64-portable.zip",
            "quantsupport-0.1.11-cp311-cp311-win_amd64.whl",
        ]
        fixture = {"tagName": "v0.1.10", "assets": [
            {"name": name, "state": "uploaded", "size": 42} for name in names]}
        self.assertEqual(downloads.packages(fixture), {})

    def test_standalone_page_links_to_hosted_assets_and_previous_release(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            page = root / "python" / "downloads.html"
            destination = root / "downloads" / "python"
            with patch.object(downloads, "command", side_effect=self.command):
                downloads.stage("owner/repo", "v0.1.10", page, destination)
            html = page.read_text()
            self.assertIn("<!doctype html>", html)
            self.assertIn("../downloads/python/v0.1.10/", html)
            self.assertIn("https://github.com/owner/repo/releases/download/v0.1.9/", html)
            self.assertNotIn("https://github.com/owner/repo/releases/download/v0.1.10/", html)
            self.assertNotIn("/v0.1.11/", html)
            self.assertEqual(self.downloaded, ["v0.1.10"])
            self.assertEqual(len(list((destination / "v0.1.10").iterdir())), 40)
            for path in (destination / "v0.1.10").iterdir():
                self.assertEqual(path.read_bytes(), b"fixture")
            local_links = re.findall(r'href="(\.\./downloads/[^"]+)"', html)
            self.assertEqual(len(local_links), 40)
            for link in local_links:
                self.assertTrue((page.parent / link).is_file(), link)
            self.assertFalse((destination / "v0.1.9").exists())

    def test_missing_download_does_not_publish_a_page_or_partial_assets(self):
        self.missing_download = self.releases["v0.1.10"]["assets"][0]["name"]
        with TemporaryDirectory() as temporary:
            page = Path(temporary) / "python" / "downloads.html"
            destination = Path(temporary) / "downloads" / "python"
            with patch.object(downloads, "command", side_effect=self.command):
                with self.assertRaisesRegex(RuntimeError, "download missing or empty"):
                    downloads.stage("owner/repo", "v0.1.10", page, destination)
            self.assertFalse(page.exists())
            self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
