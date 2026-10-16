"""A small JS scanner shared by the UI text tests (copy of the one used for the conversion). """
"""A small JS scanner: finds string literals ('..', "..", `..` with ${} nesting) while skipping comments. Regular expression literals that
contain quotes are not recognised; the sources of this project do not have them. tokens: dict(kind 'str'|'tpl', start, end, body)."""


def scan(text):
    tokens = []
    n = len(text)

    def string(i, q):
        j = i + 1
        while j < n and text[j] != q:
            if text[j] == "\\":
                j += 1
            if text[j] == "\n":
                break
            j += 1
        tokens.append(dict(kind="str", start=i, end=j + 1, body=text[i + 1:j]))
        return j + 1

    def template(i):
        j = i + 1
        while j < n and text[j] != "`":
            if text[j] == "\\":
                j += 2
                continue
            if text[j] == "$" and text[j + 1:j + 2] == "{":
                j = code(j + 2, True)
                continue
            j += 1
        tokens.append(dict(kind="tpl", start=i, end=j + 1, body=text[i + 1:j]))
        return j + 1

    def code(i, in_expr):
        depth = 1 if in_expr else 0
        while i < n:
            c = text[i]
            if c in "'\"":
                i = string(i, c)
                continue
            if c == "`":
                i = template(i)
                continue
            if c == "/" and text[i + 1:i + 2] == "/":
                i = text.find("\n", i)
                i = n if i < 0 else i
                continue
            if c == "/" and text[i + 1:i + 2] == "*":
                i = text.find("*/", i + 2)
                i = n if i < 0 else i + 2
                continue
            if in_expr:
                if c == "{":
                    depth += 1
                elif c == "}":
                    depth -= 1
                    if depth == 0:
                        return i + 1
            i += 1
        return i

    code(0, False)
    tokens.sort(key=lambda t: t["start"])
    return tokens
