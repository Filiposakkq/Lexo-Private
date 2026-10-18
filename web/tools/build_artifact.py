"""Build a preview copy of the site (for a claude.ai artifact) into a folder.

index.html becomes a fragment without the document wrapper, with site.css and the images inlined;
the other pages and the assets they need are copied next to it as they are.
Usage: python web/tools/build_artifact.py OUT_DIR
"""
import base64
import pathlib
import re
import shutil
import sys

WEB = pathlib.Path(__file__).resolve().parent.parent / "public"
MIME = {".webp": "image/webp", ".png": "image/png", ".svg": "image/svg+xml"}


def data_uri(rel):
    path = WEB / rel
    return "data:%s;base64,%s" % (MIME[path.suffix], base64.b64encode(path.read_bytes()).decode())


def main(out):
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    html = (WEB / "index.html").read_text(encoding="utf-8")
    head = re.search(r"<head>(.*?)</head>", html, re.S).group(1)
    m = re.search(r"<body([^>]*)>(.*?)</body>", html, re.S)
    body = m.group(2)
    # the artifact wraps the page in its own <body>, so attributes of ours are set by a script instead
    attrs = re.findall(r"([\w-]+)", m.group(1))
    if attrs:
        body = "<script>" + "".join('document.body.setAttribute("%s","");' % a for a in attrs) + "</script>" + body
    head = "\n".join(
        line for line in head.splitlines()
        if not re.search(r'<meta (charset|name="viewport"|name="description"|name="theme-color"|property=)|rel="(icon|apple-touch-icon)"', line)
    )
    css = (WEB / "assets" / "site.css").read_text(encoding="utf-8")
    head = head.replace('<link rel="stylesheet" href="assets/site.css">', "<style>\n" + css + "</style>")
    page = head.strip() + "\n" + body
    page = re.sub(r"(assets/img/[\w.-]+\.(?:webp|png))", lambda m: data_uri(m.group(1)), page)
    (out / "index.html").write_text(page, encoding="utf-8")
    for f in WEB.glob("*.html"):
        if f.name not in ("index.html", "404.html"):
            shutil.copy(f, out / f.name)
    (out / "assets").mkdir(exist_ok=True)
    for name in ("site.css", "site.js", "data.js", "favicon.svg"):
        shutil.copy(WEB / "assets" / name, out / "assets" / name)
    shutil.copytree(WEB / "assets" / "badge", out / "assets" / "badge", dirs_exist_ok=True)
    print("wrote", out, (out / "index.html").stat().st_size // 1024, "KB")


if __name__ == "__main__":
    main(sys.argv[1])
