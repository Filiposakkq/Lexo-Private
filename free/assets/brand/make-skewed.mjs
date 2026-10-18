// Draws the symbol of the logo as on the brand sheet, from measurements of the sheet, and puts it into the SVG files of this folder:
//  - the skewed symbol (a rounded quadrilateral with a perspective; the A stays upright) in symbol-only-1024.svg and in the three logos,
//  - the upright icon (app-icon-1024.svg) with the same A and foot.
// All coordinates below are pixels of a crop of the brand sheet; the A is a wide, low letter with a foot (a lighter wedge on the right).
// Usage: node make-skewed.mjs   (run in this folder)
import { readFileSync, writeFileSync } from "node:fs";

const f = (n) => Math.round(n * 10) / 10;
const unit = (a, b) => {
  const dx = b.x - a.x, dy = b.y - a.y, d = Math.hypot(dx, dy);
  return { x: dx / d, y: dy / d };
};
/** A polygon with a radius at every corner (0 = sharp): the corner is the control point of a quadratic curve. */
function rounded(P) {
  let d = "";
  for (let i = 0; i < P.length; i++) {
    const prev = P[(i + P.length - 1) % P.length], cur = P[i], next = P[(i + 1) % P.length];
    if (!cur.r) { d += (i === 0 ? "M" : "L") + f(cur.x) + " " + f(cur.y) + " "; continue; }
    const u1 = unit(cur, prev), u2 = unit(cur, next);
    const a = { x: cur.x + u1.x * cur.r, y: cur.y + u1.y * cur.r };
    const b = { x: cur.x + u2.x * cur.r, y: cur.y + u2.y * cur.r };
    d += (i === 0 ? "M" : "L") + f(a.x) + " " + f(a.y) + " Q" + f(cur.x) + " " + f(cur.y) + " " + f(b.x) + " " + f(b.y) + " ";
  }
  return d + "Z";
}

// ---- the sheet (crop coordinates) --------------------------------------------------------------------------------------------------------------
const SHAPE = [{ x: 197, y: 128, r: 90 }, { x: 782, y: 32, r: 140 }, { x: 736, y: 768, r: 125 }, { x: 20, y: 654, r: 105 }]; // TL, TR, BR, BL of the skewed symbol
const A = [ // the letter: left foot, the two rounded top corners, right foot, inner right foot, inner apex, inner left foot
  { x: 125, y: 608, r: 0 }, { x: 366, y: 210, r: 30 }, { x: 478, y: 210, r: 30 }, { x: 675, y: 608, r: 0 }, { x: 540, y: 608, r: 0 }, { x: 412, y: 350, r: 0 }, { x: 275, y: 608, r: 0 },
];
const FOOT = [{ x: 362, y: 503 }, { x: 488, y: 503 }, { x: 540, y: 608 }, { x: 435, y: 608 }]; // the lighter wedge: its right edge lies on the inner edge of the right leg

// ---- the skewed symbol: crop -> the 1024 viewBox (the picture lies inside 64..960) -----------------------------------------------------------
const [minX, maxX] = [20, 782];
const s = 896 / (maxX - minX);
const offY = 64 + (896 - (768 - 32) * s) / 2;
const mapSkew = (c) => ({ ...c, x: 64 + (c.x - minX) * s, y: offY + (c.y - 32) * s, r: (c.r || 0) * s });
// ---- the upright icon: the A is 667 wide and 534 high, centred on the square ---------------------------------------------------------------------
const sx = 667 / 550, sy = 534 / 398;
const mapIcon = (c) => ({ ...c, x: 512 + (c.x - 400) * sx, y: 517 + (c.y - 409) * sy, r: (c.r || 0) * Math.min(sx, sy) });

const pathOf = (pts, map) => rounded(pts.map((p) => map({ r: 0, ...p })));
const grad = '<linearGradient id="gs" x1="0" y1="1" x2="1" y2="0"><stop offset="0" stop-color="#FF315C"/><stop offset="1" stop-color="#FF7043"/></linearGradient>';
const gradIcon = '<linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#FF315C"/><stop offset="1" stop-color="#FF7043"/></linearGradient>';

function symbol(kind, map, container) {
  const a = `<path d="${pathOf(A, map)}" fill="${kind === "mono" ? "#0F0F14" : "#FFFFFF"}"/>`;
  const foot = `<path d="${pathOf(FOOT, map)}" fill="${kind === "mono" ? "#0F0F14" : "#FFFFFF"}" fill-opacity="${kind === "mono" ? ".4" : ".62"}"/>`;
  return `${container}\n${a}\n${foot}`;
}
const skewContainer = (fill) => `<path d="${rounded(SHAPE.map(mapSkew))}" fill="${fill}"/>`;
const iconContainer = '<rect x="64" y="64" width="896" height="896" rx="210" fill="url(#g)"/>';

// the symbol alone and the upright icon
writeFileSync("symbol-only-1024.svg", `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">\n<defs>\n  ${grad}\n</defs>\n${symbol("color", mapSkew, skewContainer("url(#gs)"))}\n</svg>\n`);
writeFileSync("app-icon-1024.svg", `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">\n<defs>\n  ${gradIcon}\n</defs>\n${symbol("color", mapIcon, iconContainer)}\n</svg>\n`);

// the logos: the content of the group that holds the symbol is replaced
function patchLogo(file, kind) {
  let t = readFileSync(file, "utf8").replace(/\r\n/g, "\n");
  const inner = symbol(kind, mapSkew, skewContainer(kind === "mono" ? "#EAEAEA" : "url(#gs)"));
  const re = /(<g transform="[^"]*">)([\s\S]*?)(<\/g>)/;
  if (!re.test(t)) throw new Error(file + ": no group with the symbol");
  t = t.replace(re, (_, open, _old, close) => `${open}\n${inner}\n${close}`);
  if (kind !== "mono" && !t.includes('id="gs"')) t = t.replace("</defs>", grad + "</defs>");
  writeFileSync(file, t);
  console.log("patched", file);
}
patchLogo("primary-logo.svg", "color");
patchLogo("compact-logo-360x120.svg", "color");
patchLogo("monochrome-logo-360x120.svg", "mono");
