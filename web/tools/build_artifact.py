"""Build one self-contained page from web/index.html (for a preview link such as a claude.ai artifact).

Images are inlined as data: URIs; the document wrapper, the favicon and the social tags are dropped.
Usage: python web/tools/build_artifact.py OUT.html
"""
import base64
import pathlib
import re
import sys

WEB = pathlib.Path(__file__).resolve().parent.parent
MIME = {".webp": "image/webp", ".png": "image/png", ".svg": "image/svg+xml"}


def data_uri(rel):
    path = WEB / rel
    return "data:%s;base64,%s" % (MIME[path.suffix], base64.b64encode(path.read_bytes()).decode())


def main(out):
    html = (WEB / "index.html").read_text(encoding="utf-8")
    head = re.search(r"<head>(.*?)</head>", html, re.S).group(1)
    body = re.search(r"<body>(.*?)</body>", html, re.S).group(1)
    head = "\n".join(
        line for line in head.splitlines()
        if not re.search(r'<meta (charset|name="viewport"|name="description"|name="theme-color"|property=)|rel="(icon|apple-touch-icon)"', line)
    )
    page = head.strip() + "\n" + body
    page = re.sub(r'"(img/[\w.-]+\.(?:webp|png))"', lambda m: '"%s"' % data_uri(m.group(1)), page)
    pathlib.Path(out).write_text(page, encoding="utf-8")
    print("wrote", out, len(page) // 1024, "KB")


if __name__ == "__main__":
    main(sys.argv[1])
