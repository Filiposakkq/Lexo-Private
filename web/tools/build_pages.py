"""Assemble the sub-pages from the shared header and footer.

Each page source in web/pages/<name>.html starts with a front matter block:
    title: ... / desc: ... / sub: ... / ---css--- ... / ---body--- ... / ---script--- ...
and becomes web/<name>.html. Run it after changing web/pages/_head.html, _foot.html or a page.
Usage: python web/tools/build_pages.py
"""
import pathlib
import re

WEB = pathlib.Path(__file__).resolve().parent.parent
PAGES = WEB / "pages"


def main():
    head = (PAGES / "_head.html").read_text(encoding="utf-8")
    foot = (PAGES / "_foot.html").read_text(encoding="utf-8")
    for src in sorted(PAGES.glob("[!_]*.html")):
        text = src.read_text(encoding="utf-8")
        meta, rest = text.split("---css---", 1)
        css, rest = rest.split("---body---", 1)
        body, script = (rest.split("---script---", 1) + [""])[:2]
        fields = dict(re.findall(r"^(\w+):\s*(.*)$", meta, re.M))
        page = head.replace("{{TITLE}}", fields["title"]).replace("{{DESC}}", fields["desc"]).replace("{{SUB}}", fields["sub"]).replace("{{CSS}}", css.strip())
        page = page.replace('<a href="%s.html">' % src.stem, '<a href="%s.html" class="on">' % src.stem)
        page += body.strip() + "\n" + foot.replace("{{SCRIPT}}", ("<script>\n" + script.strip() + "\n</script>") if script.strip() else "")
        (WEB / src.name).write_text(page, encoding="utf-8")
        print("wrote", src.name)


if __name__ == "__main__":
    main()
