import { get } from './api.js';

/* The helpers of the game-window preview (the picture of the scene with the text box over it), ported from the classic interface: the text of a line the way the
   game draws it, the font of the game as a FontFace, the width of every character from the font's own table, and a cache of everything one line needs. */

/* The text of a line as the player sees it: {i} {b} {u} {color=…} are drawn, line breaks break, [variables] are five X, the other tags are invisible. */
export function gameText(text) {
  const root = document.createElement('span');
  const stack = [root];
  const top = () => stack[stack.length - 1];
  const open = (tag, color) => { const el = document.createElement(tag); if (color) el.style.color = color; el.dataset.tag = tag; top().append(el); stack.push(el); };
  const close = (tag) => { for (let i = stack.length - 1; i > 0; i--) if (stack[i].dataset.tag === tag) { stack.length = i; return; } };
  const re = /\{\{|\[\[|\{(\/?)(i|b|u|s)\}|\{color=([^}]*)\}|\{\/color\}|\{p(?:=[^}]*)?\}|\{[^}]*\}|\[[^\]]*\]|\\n|\\"|\\\\|\\./g;
  const src = String(text ?? '');
  let last = 0, m;
  while ((m = re.exec(src))) {
    if (m.index > last) top().append(src.slice(last, m.index));
    const tok = m[0];
    if (tok === '{{') top().append('{');
    else if (tok === '[[') top().append('[');
    else if (m[2]) { if (m[1]) close(m[2]); else open(m[2]); }
    else if (m[3] !== undefined) { const c = m[3].trim(); open('span', /^#?[0-9a-fA-F]{3,8}$/.test(c) ? (c[0] === '#' ? c : '#' + c) : /^[a-z]+$/i.test(c) ? c : ''); }
    else if (tok === '{/color}') close('span');
    else if (tok === '\\n' || tok.startsWith('{p')) top().append(document.createElement('br'));
    else if (tok === '\\"') top().append('"');
    else if (tok === '\\\\') top().append('\\');
    else if (tok[0] === '[') top().append('XXXXX');
    else if (tok[0] === '\\') top().append(tok[1]);
    last = re.lastIndex;
  }
  if (last < src.length) top().append(src.slice(last));
  return root;
}

/* Every character goes into a box as wide as the font's own advance (hmtx), which is what Ren'Py uses. Words stay unbreakable, spaces break. */
export function placeChars(el, m, px) {
  if (!m) return;
  const w = (ch) => { const a = m.adv[ch.codePointAt(0)]; return a === undefined ? null : (a * px) / m.upm; };
  const box = (cls, width, text) => { const s = document.createElement('span'); s.className = cls; if (width !== undefined) s.style.width = width + 'px'; if (text !== undefined) s.textContent = text; return s; };
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT), nodes = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n);
  for (const node of nodes) {
    const frag = document.createDocumentFragment();
    for (const part of node.nodeValue.split(/( +)/)) {
      if (!part) continue;
      if (part[0] === ' ') { for (let k = 0; k < part.length; k++) { frag.append(box('gch', w(' ') ?? px * 0.25)); frag.append(document.createElement('wbr')); } continue; }
      const word = box('gw');
      for (const ch of part) { const cw = w(ch); word.append(cw === null ? ch : box('gch', cw, ch)); }
      frag.append(word);
    }
    node.replaceWith(frag);
  }
}

const cache = { scene: new Map(), fonts: {}, fontRes: {}, metrics: {}, metRes: {}, images: new Map(), gui: null };
export const fontOf = (f) => (f && cache.fontRes[f.file]) || '';
export const metricsOf = (f) => (f && cache.metRes[f.file]) || null;

/* A font file of the game made available to the page as a FontFace; its family name, or '' when it cannot be had. */
async function loadGameFont(f) {
  if (!f || !f.url) return '';
  cache.fonts[f.file] ||= (async () => {
    try { const face = new FontFace('game-' + f.file, `url("${f.url}")`); await face.load(); document.fonts.add(face); return 'game-' + f.file; } catch { return ''; }
  })().then((r) => { cache.fontRes[f.file] = r; return r; });
  return cache.fonts[f.file];
}
async function loadGameMetrics(f) {
  if (!f || !f.file) return null;
  cache.metrics[f.file] ||= get('game/fontmetrics', { k: f.file }).catch(() => null).then((r) => { cache.metRes[f.file] = r; return r; });
  return cache.metrics[f.file];
}

/* The window values of the game (size of the text box and of the text, from its gui.rpy), or the Ren'Py defaults when they cannot be read. */
export async function loadGui(fallbackSource) {
  if (cache.gui) return cache.gui;
  try { const st = await get('settings'); cache.gui = { ...st.game_gui, max_lines: st.validation.textbox.max_lines }; } catch { return { text_size: 33, dialogue_width: 1116, textbox_height: 278, max_lines: 4, source: fallbackSource }; }
  return cache.gui;
}

/* Everything the preview of one line needs (the scene, the picture decoded, the fonts and their widths), fetched ahead of drawing. Cached per line. */
export async function prefetchScene(file, line) {
  const key = file + '\t' + line;
  if (!cache.scene.has(key)) {
    cache.scene.set(key, get('game/scene', { file, line }).catch((e) => ({ available: false, reason: e.message })));
    if (cache.scene.size > 80) cache.scene.delete(cache.scene.keys().next().value);
  }
  const sc = await cache.scene.get(key);
  const jobs = [loadGameFont(sc.fonts_text), loadGameFont(sc.fonts_name), loadGameMetrics(sc.fonts_text), loadGameMetrics(sc.fonts_name)];
  if (sc.url) {
    if (!cache.images.has(sc.url)) { const im = new Image(); im.src = sc.url; cache.images.set(sc.url, im.decode().catch(() => {})); if (cache.images.size > 12) cache.images.delete(cache.images.keys().next().value); }
    jobs.push(cache.images.get(sc.url));
  }
  await Promise.all(jobs);
  return sc;
}
/* After something was written the pictures stay valid, but the scene of a line may change (a script was prepared again): forget them. */
export function forgetScenes() { cache.scene.clear(); }
