"""Offline tests for the static site intended for reaterm.pro-genai.com.

No third-party dependencies, paid APIs or production deployment are required.
Run from the repository root:
    python -m unittest discover -s docs/website -p "test_*.py" -v
"""

from __future__ import annotations

from functools import partial
from html.parser import HTMLParser
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import re
import threading
import unittest
from urllib.parse import unquote, urlsplit
from urllib.request import urlopen
from xml.etree import ElementTree


SITE = Path(__file__).resolve().parent
DOMAIN = "reaterm.pro-genai.com"


class SiteHTMLParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.elements: list[tuple[str, dict[str, str]]] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.elements.append((tag, {key: value or "" for key, value in attrs}))

    def find(self, tag: str) -> list[dict[str, str]]:
        return [attributes for item, attributes in self.elements if item == tag]


def parse_page(filename: str = "index.html") -> SiteHTMLParser:
    parser = SiteHTMLParser()
    parser.feed((SITE / filename).read_text(encoding="utf-8"))
    parser.close()
    return parser


class QuietHTTPRequestHandler(SimpleHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass


class StaticWebsiteTests(unittest.TestCase):
    def test_required_files_are_self_contained(self) -> None:
        for name in [
            "index.html",
            "styles.css",
            "app.js",
            "favicon.svg",
            "assets/reaterm-cover.svg",
            "404.html",
            "robots.txt",
            "sitemap.xml",
            "CNAME",
            "README.md",
        ]:
            with self.subTest(file=name):
                self.assertTrue((SITE / name).is_file(), name)
                self.assertGreater((SITE / name).stat().st_size, 0)

    def test_all_relative_images_scripts_styles_and_links_resolve(self) -> None:
        for filename in ["index.html", "404.html"]:
            page = parse_page(filename)
            for tag, attrs in page.elements:
                for key in ("href", "src"):
                    url = attrs.get(key)
                    if not url or url.startswith("#") or url.startswith(("mailto:", "tel:")):
                        continue
                    parsed = urlsplit(url)
                    if parsed.scheme in ("https", "http"):
                        self.assertEqual(parsed.scheme, "https", (tag, key, url))
                        continue
                    self.assertFalse(parsed.scheme or parsed.netloc, (tag, key, url))
                    self.assertFalse(parsed.path.startswith("/"), (tag, key, url))
                    path = unquote(parsed.path)
                    resolved = (SITE / path).resolve()
                    self.assertTrue(resolved.is_relative_to(SITE.resolve()), (filename, url))
                    self.assertTrue(resolved.is_file(), (filename, tag, key, url))

    def test_semantic_landmarks_and_accessibility(self) -> None:
        page = parse_page()
        self.assertTrue(any(tag == "html" and attrs.get("lang") == "en" for tag, attrs in page.elements))
        self.assertEqual(len(page.find("main")), 1)
        self.assertEqual(len(page.find("header")), 1)
        self.assertEqual(len(page.find("footer")), 1)
        self.assertTrue(any(link.get("href") == "#content" for link in page.find("a")))
        ids = [attrs["id"] for _, attrs in page.elements if "id" in attrs]
        self.assertEqual(len(ids), len(set(ids)), "Duplicate element IDs")
        for img in page.find("img"):
            self.assertIn("alt", img)
        for link in page.find("a"):
            href = link.get("href", "")
            if href.startswith("#"):
                self.assertIn(href[1:], ids, f"Broken hash link: {href}")

    def test_tabs_are_connected_to_panels(self) -> None:
        page = parse_page()
        tabs = [attrs for _, attrs in page.elements if attrs.get("role") == "tab"]
        panels = [attrs for _, attrs in page.elements if attrs.get("role") == "tabpanel"]
        self.assertEqual(len(tabs), 3)
        self.assertEqual(len(panels), 3)
        self.assertEqual(sum(tab.get("aria-selected") == "true" for tab in tabs), 1)
        panel_by_id = {panel.get("id"): panel for panel in panels}
        for tab in tabs:
            panel_id = tab.get("aria-controls")
            self.assertIn(panel_id, panel_by_id)
            self.assertEqual(panel_by_id[panel_id].get("aria-labelledby"), tab.get("id"))
        self.assertTrue(any(attrs.get("role") == "status" for _, attrs in page.elements))

    def test_links_cover_rust_python_typescript_and_registries(self) -> None:
        page = parse_page()
        links = {attrs.get("href") for attrs in page.find("a")}
        for url in [
            "https://github.com/Pro-GenAI/ReaTerm/tree/main/src/rust",
            "https://github.com/Pro-GenAI/ReaTerm/tree/main/src/python",
            "https://github.com/Pro-GenAI/ReaTerm/tree/main/src/typescript",
            "https://pypi.org/project/reaterm/",
            "https://www.npmjs.com/package/reaterm",
        ]:
            self.assertIn(url, links)
        self.assertIn("Registry availability is not guaranteed", (SITE / "index.html").read_text(encoding="utf-8"))

    def test_all_time_download_badges_on_website_and_readmes(self) -> None:
        page = parse_page()
        badges = {
            img["alt"]: img["src"]
            for img in page.find("img")
            if img.get("alt") in {"PyPI total downloads", "npm total downloads"}
        }
        self.assertEqual(set(badges), {"PyPI total downloads", "npm total downloads"})
        self.assertIn("/personalized-badge/reaterm", badges["PyPI total downloads"])
        self.assertIn("period=total", badges["PyPI total downloads"])
        self.assertIn("units=NONE", badges["PyPI total downloads"])
        self.assertIn("/npm/dt/reaterm.svg", badges["npm total downloads"])

        links = {anchor.get("href") for anchor in page.find("a")}
        self.assertIn("https://pepy.tech/projects/reaterm", links)
        self.assertIn("https://www.npmjs.com/package/reaterm", links)

        root = SITE.parents[1]
        for path in [
            root / "README.md",
            root / "src/python/README.md",
            root / "src/typescript/README.md",
            SITE / "README.md",
        ]:
            with self.subTest(readme=str(path.relative_to(root))):
                content = path.read_text(encoding="utf-8")
                self.assertIn("/personalized-badge/reaterm?period=total", content)
                self.assertIn("/npm/dt/reaterm.svg", content)
                self.assertIn('alt="PyPI total downloads"', content)
                self.assertIn('alt="npm total downloads"', content)

    def test_provider_tool_docs_and_one_line_examples(self) -> None:
        page = parse_page()
        ids = {attrs.get("id") for _, attrs in page.elements}
        self.assertIn("ai-tools", ids)
        links = {attrs.get("href") for attrs in page.find("a")}
        self.assertIn("#ai-tools", links)
        self.assertIn(
            "https://github.com/Pro-GenAI/ReaTerm/blob/main/docs/TOOL_INTEGRATIONS.md",
            links,
        )
        html = (SITE / "index.html").read_text(encoding="utf-8")
        for snippet in (
            'import reaterm; print(reaterm.run("ls -al"))',
            'import * as reaterm from "reaterm"',
            "reaterm_tool_openai",
            "reaterm_tool_openai_responses",
            "reaterm_tool_anthropic",
            "execute_reaterm_tool",
            "call.function.arguments",
            "block.input",
        ):
            with self.subTest(snippet=snippet):
                self.assertIn(snippet, html)
        self.assertIn("tool calls", html)
        self.assertIn("can disclose readable files", html)

    def test_branding_domain_and_index_metadata(self) -> None:
        page = parse_page()
        self.assertEqual((SITE / "CNAME").read_text(encoding="utf-8").strip(), DOMAIN)
        canonical = [link.get("href") for link in page.find("link") if link.get("rel") == "canonical"]
        self.assertEqual(canonical, ["https://" + DOMAIN + "/"])
        self.assertTrue(any(m.get("name") == "description" for m in page.find("meta")))
        self.assertEqual(len(page.find("title")), 1)
        self.assertIn("read-only", (SITE / "index.html").read_text(encoding="utf-8").lower())

    def test_svg_and_sitemap_xml_are_well_formed(self) -> None:
        for file in ["favicon.svg", "assets/reaterm-cover.svg", "sitemap.xml"]:
            with self.subTest(file=file):
                root = ElementTree.parse(SITE / file).getroot()
                self.assertIn(root.tag.rsplit("}", 1)[-1], {"svg", "urlset"})
        sitemap = (SITE / "sitemap.xml").read_text(encoding="utf-8")
        self.assertIn("https://" + DOMAIN + "/", sitemap)
        robots = (SITE / "robots.txt").read_text(encoding="utf-8")
        self.assertIn("Sitemap: https://" + DOMAIN + "/sitemap.xml", robots)

    def test_no_third_party_runtime_scripts_or_styles(self) -> None:
        page = parse_page()
        scripts = [attrs.get("src") for attrs in page.find("script")]
        self.assertEqual(scripts, ["./app.js"])
        styles = [attrs.get("href") for attrs in page.find("link") if attrs.get("rel") == "stylesheet"]
        self.assertEqual(styles, ["./styles.css"])
        css = (SITE / "styles.css").read_text(encoding="utf-8")
        self.assertNotIn("@import", css)
        self.assertNotRegex(css, r"url\s*\(\s*['\"]?https?://")
        js = (SITE / "app.js").read_text(encoding="utf-8")
        self.assertIn('navigator.clipboard', js)
        self.assertIn('aria-selected', js)
        self.assertNotIn("fetch(", js)

    def test_marketing_readme_and_website_development_guide(self) -> None:
        readme = (SITE / "README.md").read_text(encoding="utf-8")
        self.assertIn(DOMAIN, readme)
        self.assertIn("read-only", readme.lower())
        self.assertNotIn("python -m http.server", readme)
        self.assertNotIn("make -C docs/website", readme)

        guide = (SITE.parent / "WEBSITE_DEVELOPMENT.md").read_text(encoding="utf-8")
        for phrase in (
            "python -m http.server",
            "cd docs/website",
            "docs/website",
            DOMAIN,
            "not automatically",
            "unittest discover -s docs/website",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, guide)

    def test_local_python_http_server_serves_site(self) -> None:
        handler = partial(QuietHTTPRequestHandler, directory=str(SITE))
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            base = "http://127.0.0.1:" + str(server.server_port)
            for url, expected_type, needle in [
                ("/", "text/html", b"Explore everything."),
                ("/styles.css", "text/css", b".hero"),
                ("/app.js", "javascript", b"aria-selected"),
                ("/favicon.svg", "image/svg+xml", b"<svg"),
                ("/assets/reaterm-cover.svg", "image/svg+xml", b"ReaTerm"),
            ]:
                with self.subTest(path=url):
                    with urlopen(base + url, timeout=4) as response:
                        self.assertEqual(response.status, 200)
                        self.assertIn(expected_type, response.headers.get("Content-Type", ""))
                        self.assertIn(needle, response.read())
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=4)


if __name__ == "__main__":
    unittest.main()
