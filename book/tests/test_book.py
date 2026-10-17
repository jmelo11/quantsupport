"""Contracts for the conceptual Book, independent of binding generators."""

from pathlib import Path
import re
import unittest
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "book" / "src"
FENCE = re.compile(r"^```([^\n]*)\n(.*?)^```[ \t]*$", re.M | re.S)
LINK = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")


def prose(text):
    return FENCE.sub("", text)


def link_prose(text):
    text = re.sub(r"\\\\\[.*?\\\\\]", "", prose(text), flags=re.S)
    return re.sub(r"`[^`]*`", "", text)


def anchors(text):
    result = set()
    counts = {}
    for heading in re.findall(r"^#{1,6}\s+(.+)$", prose(text), re.M):
        explicit = re.search(r"\{#([^}]+)\}", heading)
        if explicit:
            result.add(explicit.group(1))
            continue
        heading = heading.replace("`", "").lower()
        slug = re.sub(r"[^\w\s-]", "", heading)
        slug = re.sub(r"\s+", "-", slug)
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        result.add(f"{slug}-{count}" if count else slug)
    return result


class BookStructureTests(unittest.TestCase):
    def test_summary_covers_every_chapter_once(self):
        links = [
            target for target in LINK.findall((SOURCE / "SUMMARY.md").read_text())
            if target.endswith(".md")
        ]
        self.assertEqual(len(links), len(set(links)))
        self.assertGreaterEqual(len(links), 26)
        self.assertLessEqual(len(links), 30)
        self.assertEqual(
            set(links),
            {str(path.relative_to(SOURCE)) for path in SOURCE.rglob("*.md")
             if path.name != "SUMMARY.md"},
        )

    def test_local_links_and_anchors_exist(self):
        for path in [
            ROOT / "README.md", ROOT / "book" / "README.md",
            *SOURCE.rglob("*.md"),
        ]:
            for target in LINK.findall(link_prose(path.read_text())):
                with self.subTest(path=path.relative_to(ROOT), target=target):
                    parsed = urlsplit(target)
                    if parsed.scheme or parsed.netloc:
                        continue
                    destination = (
                        path.parent / unquote(parsed.path)
                    ).resolve() if parsed.path else path
                    self.assertTrue(destination.exists(), str(destination))
                    if parsed.fragment and destination.suffix == ".md":
                        self.assertIn(unquote(parsed.fragment),
                                      anchors(destination.read_text()))

    def test_interfaces_are_substantive_and_independent(self):
        for relative in (
            "getting-started/rust-api.md", "python/getting-started.md",
            "excel/workbook.md",
        ):
            with self.subTest(chapter=relative):
                text = (SOURCE / relative).read_text()
                self.assertGreater(len(prose(text).split()), 350)
                self.assertNotIn("](../api/", text)
        self.assertIn("releases/latest/download/quantsupport.xll",
                      (SOURCE / "excel/installation.md").read_text())

    def test_runnable_rust_has_a_crate_harness(self):
        runnable = 0
        for path in SOURCE.rglob("*.md"):
            text = path.read_text()
            for fence in FENCE.finditer(text):
                language, content = fence.groups()
                if language.split(",")[0] != "rust":
                    continue
                with self.subTest(path=path.relative_to(SOURCE)):
                    if "ignore" in language.split(","):
                        introduction = text[:fence.start()].rsplit("```", 1)[-1]
                        self.assertIn("fragment", introduction.lower())
                    else:
                        runnable += 1
                        self.assertIn("# extern crate quantsupport;", content)
        self.assertGreaterEqual(runnable, 4)

    def test_worksheet_context_argument_positions(self):
        text = (SOURCE / "excel/workbook.md").read_text()
        formulas = re.findall(r"QS\.CONTEXT\.CREATE\(([^)\n]+)\)", text)
        self.assertEqual(len(formulas), 1)
        arguments = formulas[0].split(",")
        self.assertEqual(arguments[12:14], ['"USD"', '"SOFR"'])
        self.assertEqual(arguments[3:12], [""] * 9)

    def test_reference_catalog_is_not_a_book_preprocessor(self):
        configuration = (ROOT / "book.toml").read_text()
        self.assertNotIn("[preprocessor.bindings]", configuration)
        self.assertFalse((ROOT / "book" / "bindings.py").exists())

    def test_retired_chapter_redirects_reach_current_chapters(self):
        configuration = (ROOT / "book.toml").read_text()
        section = configuration.split("[output.html.redirect]", 1)[1].split("\n[", 1)[0]
        redirects = re.findall(r'^"([^"]+)" = "([^"]+)"$', section, re.M)
        self.assertEqual(len(redirects), 16)
        for old, target in redirects:
            with self.subTest(old=old):
                destination = (SOURCE / Path(old).parent / target).with_suffix(".md")
                self.assertTrue(destination.resolve().exists())

    def test_inline_math_delimiters_survive_markdown(self):
        for path in SOURCE.rglob("*.md"):
            with self.subTest(path=path.relative_to(SOURCE)):
                self.assertNotRegex(prose(path.read_text()), r"(?<!\\)\\[()]")
                self.assertNotRegex(prose(path.read_text()), r"(?<!\\)\\[!,:;%{}]")

    def test_math_does_not_turn_into_markdown_markup(self):
        for path in SOURCE.rglob("*.md"):
            for formula in re.findall(r"\\\\\[\n(.*?)\n\\\\\]", prose(path.read_text()), re.S):
                with self.subTest(path=path.relative_to(SOURCE)):
                    self.assertNotRegex(formula, r"\[[^\]]+\]\([^)]*\)")
                    self.assertNotRegex(formula, r"(?<!\\)\*")


if __name__ == "__main__":
    unittest.main()
