# Aside brand assets

The vector sources of the logo (colours `#FF315C` to `#FF7043`, ink `#0F0F14`, light `#EAEAEA`). The small light wedge at the foot of the "A" is white at 62 % opacity (a dark one at 40 % in the monochrome logo); it touches the right leg.

| File | Use |
|---|---|
| `app-icon-1024.svg` | the icon of the application (the rounded square with the A) |
| `primary-logo.svg` | the logo with the name and the line "Translation Manager for Ren'Py" (dark backgrounds) |
| `compact-logo-360x120.svg` | the same, smaller (headers, e-mail) |
| `monochrome-logo-360x120.svg` | one-colour version |

The text in the logos uses the fonts Inter / Manrope when they are installed; for printing or the website convert the text to outlines first.

The icons of the application (`rust/crates/lexo-app/icons/`, `rust/resources/assets/icon.*`, `rust/resources/ui-classic/aside.png`) are PNG renders of `app-icon-1024.svg` cropped to the rounded square (the viewBox has a margin of 64 units); `icon.ico` holds the sizes 16 to 256.

## The skewed symbol

The logos (not the icon of the application, which is the upright rounded square) use the skewed symbol of the brand sheet: a rounded quadrilateral with a perspective, the A stays upright. `symbol-only-1024.svg` is the symbol alone. The shape was measured on the brand sheet and is drawn by `make-skewed.mjs` (run it in this folder to change the corners or the radii; it also puts the shape into the three logos).
