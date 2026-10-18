// Makes the skewed symbol of the logo (as in the brand sheet: a rounded quadrilateral with a perspective, the A stays upright) and puts it into the logos.
// The corners were measured on the brand sheet; the shape is drawn in the 1024 x 1024 viewBox of the icon (the picture lies inside 64..960).
// Usage: node make-skewed.mjs   (run in this folder)
import { readFileSync, writeFileSync } from "node:fs";

// corners (TL, TR, BR, BL) measured on the picture, with the radius of every corner
const raw = [
  { x: 197, y: 128, r: 90 },
  { x: 782, y: 32, r: 140 },
  { x: 736, y: 768, r: 125 },
  { x: 20, y: 654, r: 105 },
];
const [minX, maxX] = [20, 782];
const s = 896 / (maxX - minX);
const height = (768 - 32) * s;
const offY = 64 + (896 - height) / 2;
const P = raw.map((c) => ({ x: 64 + (c.x - minX) * s, y: offY + (c.y - 32) * s, r: c.r * s }));

const unit = (a, b) => {
  const dx = b.x - a.x, dy = b.y - a.y, d = Math.hypot(dx, dy);
  return { x: dx / d, y: dy / d };
};
const f = (n) => Math.round(n * 10) / 10;
// a rounded polygon: along every edge, a curve (quadratic, the corner is the control point) at every corner
let d = "";
for (let i = 0; i < P.length; i++) {
  const prev = P[(i + P.length - 1) % P.length], cur = P[i], next = P[(i + 1) % P.length];
  const u1 = unit(cur, prev), u2 = unit(cur, next);
  const a = { x: cur.x + u1.x * cur.r, y: cur.y + u1.y * cur.r };
  const b = { x: cur.x + u2.x * cur.r, y: cur.y + u2.y * cur.r };
  d += (i === 0 ? "M" : "L") + f(a.x) + " " + f(a.y) + " Q" + f(cur.x) + " " + f(cur.y) + " " + f(b.x) + " " + f(b.y) + " ";
}
d += "Z";

const A = '<path d="M230 760 L414 300 Q430 260 466 260 H558 Q594 260 610 300 L794 760 H652 L512 414 L372 760 Z" fill="#FFFFFF"/>';
const FOOT = '<path d="M454 638 H603 L652 760 H508 Z" fill="#FFFFFF" fill-opacity=".62"/>';
const FOOT_MONO = '<path d="M454 638 H603 L652 760 H508 Z" fill="#0F0F14" fill-opacity=".4"/>';
const grad = '<linearGradient id="gs" x1="0" y1="1" x2="1" y2="0"><stop offset="0" stop-color="#FF315C"/><stop offset="1" stop-color="#FF7043"/></linearGradient>';
const symbol = `<path d="${d}" fill="url(#gs)"/>\n${A}\n${FOOT}`;
const symbolMono = `<path d="${d}" fill="#EAEAEA"/>\n<path d="M230 760 L414 300 Q430 260 466 260 H558 Q594 260 610 300 L794 760 H652 L512 414 L372 760 Z" fill="#0F0F14"/>\n${FOOT_MONO}`;

// the symbol alone
writeFileSync(
  "symbol-only-1024.svg",
  `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">\n<defs>\n  ${grad}\n</defs>\n${symbol}\n</svg>\n`,
);

// the logos: the rounded square of the icon is replaced by the skewed shape
function patchLogo(file, mono) {
  let s = readFileSync(file, "utf8").replace(/\r\n/g, "\n");
  const start = s.indexOf('<rect x="64"');
  const end = s.indexOf("</g>", start);
  if (start < 0 || end < 0) throw new Error(file + ": the symbol was not found");
  s = s.slice(0, start) + (mono ? symbolMono : symbol) + "\n" + s.slice(end);
  if (!mono && !s.includes('id="gs"')) s = s.replace("</defs>", grad + "</defs>");
  writeFileSync(file, s);
  console.log("patched", file);
}
patchLogo("primary-logo.svg", false);
patchLogo("compact-logo-360x120.svg", false);
patchLogo("monochrome-logo-360x120.svg", true);
console.log("path:", d.length, "chars");
