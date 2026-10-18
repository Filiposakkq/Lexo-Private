"""Build a preview copy of the site (for a claude.ai artifact) into a folder.

index.html becomes a fragment without the document wrapper, with site.css and the images inlined;
the other pages, site.css, site.js, favicon.svg and the badges are copied next to it as they are.
Usage: python web/tools/build_artifact.py OUT_DIR
"""
import base64
import pathlib
import re
import shutil
import sys

WEB = pathlib.Path(__file__).resolve().parent.parent
MIME = {".webp": "image/webp", ".png": "image/png", ".svg": "image/svg+xml"}


def data_uri(rel):
    path = WEB / rel
    return "data:%s;base64,%s" % (MIME[path.suffix], base64.b64encode(path.read_bytes()).decode())


def main(out):
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    html = (WEB / "index.html").read_text(encoding="utf-8")
    head = re.search(r"<head>(.*?)</head>", html, re.S).group(1)
    body = re.search(r"<body>(.*?)</body>", html, re.S).group(1)
    head = "\n".join(
        line for line in head.splitlines()
        if not re.search(r'<meta (charset|name="viewport"|name="description"|name="theme-color"|property=)|rel="(icon|apple-touch-icon)"', line)
    )
    css = (WEB / "site.css").read_text(encoding="utf-8")
    head = head.replace('<link rel="stylesheet" href="site.css">', "<style>\n" + css + "</style>")
    page = head.strip() + "\n" + body
    page = re.sub(r"(img/[\w.-]+\.(?:webp|png))", lambda m: data_uri(m.group(1)), page)
    (out / "index.html").write_text(page, encoding="utf-8")
    for name in ("docs.html", "releases.html", "compare.html", "blog.html", "developers.html", "beta.html", "showcase.html", "roadmap.html", "site.css", "site.js", "favicon.svg"):
        shutil.copy(WEB / name, out / name)
    shutil.copytree(WEB / "badge", out / "badge", dirs_exist_ok=True)
    print("wrote", out, (out / "index.html").stat().st_size // 1024, "KB")


if __name__ == "__main__":
    main(sys.argv[1])
