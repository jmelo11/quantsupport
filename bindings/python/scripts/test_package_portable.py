"""Dependency resolution must respect the advertised minimum operating system."""

import unittest

from package_portable import compatible_platforms


class PortablePlatformTests(unittest.TestCase):
    def platforms(self, platform):
        return compatible_platforms(f"quantsupport-0.1.12-cp311-cp311-{platform}.whl")

    def test_macos_arm_excludes_newer_accelerate_wheels(self):
        platforms = self.platforms("macosx_11_0_arm64")
        self.assertIn("macosx_11_0_arm64", platforms)
        self.assertIn("macosx_10_9_universal2", platforms)
        self.assertNotIn("macosx_14_0_arm64", platforms)
        self.assertNotIn("macosx_10_15_arm64", platforms)
        self.assertNotIn("macosx_11_0_x86_64", platforms)

    def test_macos_intel_includes_older_deployment_targets(self):
        platforms = self.platforms("macosx_10_15_x86_64")
        self.assertIn("macosx_10_9_x86_64", platforms)
        self.assertIn("macosx_10_15_universal2", platforms)
        self.assertNotIn("macosx_11_0_x86_64", platforms)

    def test_linux_uses_glibc_floor_and_legacy_aliases(self):
        for arch in ("x86_64", "aarch64"):
            platforms = self.platforms(f"manylinux_2_28_{arch}")
            self.assertIn(f"manylinux_2_17_{arch}", platforms)
            self.assertIn(f"manylinux2014_{arch}", platforms)
            self.assertNotIn(f"manylinux_2_31_{arch}", platforms)
            self.assertNotIn(f"musllinux_1_2_{arch}", platforms)
        self.assertNotIn("manylinux1_aarch64", self.platforms("manylinux_2_28_aarch64"))

    def test_windows_keeps_the_interpreter_architecture(self):
        self.assertEqual(self.platforms("win_arm64"), ["win_arm64"])
        self.assertEqual(self.platforms("win_amd64"), ["win_amd64"])


if __name__ == "__main__":
    unittest.main()
