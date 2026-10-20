import { reactive } from 'vue';

/* How the Lines screen is laid out, kept for the whole application: the layout (automatic, detail below, detail on the right, scene and film strip), the focus mode (only the list and
   the detail, with a floating bar) and the quick modes (focus + film strip + only the lines still to translate, or still to check). The layout is remembered between runs; the focus
   and the quick mode are not (the application starts the usual way). */
export const LAYOUTS = ['auto', 'rows', 'cols', 'film'];
function stored() { try { const v = localStorage.getItem('aside.layout'); return LAYOUTS.includes(v) ? v : 'auto'; } catch { return 'auto'; } }
export const ui = reactive({ layout: stored(), focus: false, quick: '', left: {} });

export function setLayout(l) {
  ui.layout = l;
  try { localStorage.setItem('aside.layout', l); } catch { /* only for this page */ }
  if (l === 'film') { try { if (!localStorage.getItem('aside.detailTab') || localStorage.getItem('aside.detailTab') === 'scene') localStorage.setItem('aside.detailTab', 'check'); } catch { /* none */ } }
}
export function setFocus(on) { ui.focus = !!on; if (!on) ui.quick = ''; }
export function enterQuick(kind) {
  setLayout('film');
  try { localStorage.setItem('aside.detailTab', kind === 'proof' ? 'proof' : 'check'); } catch { /* none */ }
  ui.quick = kind; ui.focus = true;
}
export function leaveQuick() { ui.quick = ''; ui.focus = false; }
