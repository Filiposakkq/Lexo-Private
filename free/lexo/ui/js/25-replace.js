import { $, h, plural } from './00-core.js';
import { post, request } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { empty, fname, loading } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
import { diffSpans } from './23-fixes.js';
/* Find and replace in the translated texts of all or chosen files, with a preview you tick line by line. */
export async function replaceDialog(done, preselected = []) {
  const allFiles = (S.files || []).map((x) => x.name);
  const chosen = new Set(preselected.filter((n) => allFiles.includes(n)));
  const st = { mode: 'plain', scope: chosen.size ? 'chosen' : 'all', armed: false };
  let data = null, seq = 0, timer;
  const sel = new Set();
  const find = h('input', { type: 'text', placeholder: 'Co hledat', style: { flex: 1 } });
  const repl = h('input', { type: 'text', placeholder: 'Čím nahradit (může být prázdné)', style: { flex: 1 } });
  const caseBox = h('input', { type: 'checkbox' }), protBox = h('input', { type: 'checkbox', checked: true });
  const MODES = [['plain', 'Text'], ['word', 'Celé slovo'], ['stem', 'Podle kmene (pokusné)'], ['regex', 'Regulární výraz']];
  const modeSeg = h('div', { class: 'seg' }), scopeBox = h('div'), results = h('div'), info = h('span', { class: 'grow' });
  const go = h('button', { class: 'btn primary', disabled: true }, 'Nahradit');
  const MODE_HELP = { plain: 'Najde text kdekoli (i uvnitř slov).', word: 'Najde jen celé slovo.', stem: 'Najde i jiné tvary slova podle kmene (dikmince → dikminci) a ponechá jejich koncovku. Funguje dobře jen u slov se stejným skloňováním – vždy zkontroluj náhled.', regex: 'Regulární výraz Pythonu; v náhradě lze použít \\1, \\2…' };
  const hint = h('div', { class: 'muted' });
  function paintMode() {
    modeSeg.replaceChildren(...MODES.map(([k, l]) => h('button', { class: st.mode === k ? 'on' : '', onclick: () => { st.mode = k; paintMode(); schedule(); } }, l)));
    hint.textContent = MODE_HELP[st.mode];
  }
  function paintScope() {
    const segBtn = (k, l) => h('button', { class: st.scope === k ? 'on' : '', onclick: () => { st.scope = k; paintScope(); schedule(); } }, l);
    const list = h('div', { class: 'filepick' });
    const q = h('input', { type: 'search', class: 'search', placeholder: 'Hledat soubor…', style: { maxWidth: '240px' } });
    const count = h('span', { class: 'muted' }, `vybráno ${chosen.size}`);
    const fill = () => { const t = q.value.toLowerCase(); list.replaceChildren(...allFiles.filter((n) => !t || n.toLowerCase().includes(t)).map((n) => h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: chosen.has(n), onchange: (e) => { e.target.checked ? chosen.add(n) : chosen.delete(n); count.textContent = `vybráno ${chosen.size}`; schedule(); } }), ' ', n))); };
    q.oninput = fill;
    scopeBox.replaceChildren(h('div', { class: 'toolbar' }, h('b', null, 'Kde:'), h('div', { class: 'seg' }, segBtn('all', `Všechny soubory (${allFiles.length})`), segBtn('chosen', 'Jen vybrané soubory')),
      st.scope === 'chosen' ? [count, h('button', { class: 'btn small', onclick: () => { allFiles.forEach((n) => chosen.add(n)); fill(); count.textContent = `vybráno ${chosen.size}`; schedule(); } }, 'Vše'), h('button', { class: 'btn small', onclick: () => { chosen.clear(); fill(); count.textContent = 'vybráno 0'; schedule(); } }, 'Nic'), q] : null), st.scope === 'chosen' ? list : null);
    if (st.scope === 'chosen') fill();
  }
  const request = () => ({ find: find.value, replace: repl.value, mode: st.mode, case: caseBox.checked, protect: protBox.checked, files: st.scope === 'chosen' ? [...chosen] : [] });
  function schedule() { clearTimeout(timer); timer = setTimeout(preview, 350); }
  async function preview() {
    const my = ++seq;
    if (!find.value) { data = null; sel.clear(); results.replaceChildren(empty('Zadej hledaný text', 'Náhled se ukáže hned, nic se nezapisuje, dokud nepotvrdíš.')); paint2(); return; }
    if (st.scope === 'chosen' && !chosen.size) { data = null; results.replaceChildren(empty('Vyber soubory', 'Zaškrtni aspoň jeden soubor, nebo přepni na „Všechny soubory“.')); paint2(); return; }
    results.replaceChildren(loading());
    let r;
    try { r = await post('replace/preview', request()); } catch (e) { if (my === seq) { data = null; sel.clear(); results.replaceChildren(h('div', { class: 'notice bad' }, e.message)); paint2(); } return; }
    if (my !== seq) return;
    data = r; sel.clear(); r.items.forEach((it, i) => { if (!it.skip) sel.add(i); }); st.armed = false; paintResults();
  }
  function paintResults() {
    if (!data.items.length) { results.replaceChildren(empty('Nic nenalezeno', 'V přeložených řádcích hledaný text není (nepřeložené řádky se nehledají).')); return paint2(); }
    const shown = data.items.slice(0, 500);
    const rows = shown.map((it, i) => { const d = diffSpans(it.before, it.after); return h('tr', null,
      h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(i), disabled: !!it.skip, onchange: (e) => { e.target.checked ? sel.add(i) : sel.delete(i); st.armed = false; paint2(); } })),
      h('td', null, fname(it.file), h('span', { class: 'mono muted' }, `řádek ${it.line}`)), h('td', { class: 'txt' }, d.before),
      h('td', { class: 'txt' }, it.skip ? h('span', { class: 'muted' }, it.skip) : d.after), h('td', { class: 'num' }, it.count)); });
    const t = h('table', { class: 'data' }, h('colgroup', null, [44, 200, 330, 330, 50].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', 'Kde', 'Teď', 'Po nahrazení', '×'].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '954px';
    results.replaceChildren(h('div', { class: 'toolbar' }, h('span', { class: 'muted' }, `${data.total} ${plural(data.total, 'řádek', 'řádky', 'řádků')}, ${data.matches} ${plural(data.matches, 'výskyt', 'výskyty', 'výskytů')}` + (data.truncated ? ' (zobrazeno jen prvních 2000)' : '')), h('span', { class: 'spacer' }),
      h('button', { class: 'btn small', onclick: () => { shown.forEach((it, i) => { if (!it.skip) sel.add(i); }); paintResults(); } }, 'Vybrat vše'), h('button', { class: 'btn small', onclick: () => { sel.clear(); paintResults(); } }, 'Nic')),
      data.items.length > 500 ? h('p', { class: 'muted' }, 'Zobrazeno prvních 500; nahradí se jen zobrazené.') : null, h('div', { class: 'tablewrap', style: { maxHeight: '40vh' } }, t));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    go.disabled = !n;
    go.classList.toggle('danger', st.armed);
    go.textContent = st.armed ? `Potvrdit: nahradit v ${n} ${plural(n, 'řádku', 'řádcích', 'řádcích')}` : `Nahradit (${n})`;
    info.textContent = data ? `Vybráno ${n} z ${data.applicable}` + (n > 1 ? ' · hromadná změna se musí potvrdit dvakrát' : '') : '';
  }
  go.onclick = async () => {
    if (sel.size > 1 && !st.armed) { st.armed = true; paint2(); setTimeout(() => { st.armed = false; paint2(); }, 8000); return false; }
    try {
      const shown = data.items.slice(0, 500);
      const r = await post('replace/apply', { ...request(), selections: [...sel].map((i) => ({ file: shown[i].file, line: shown[i].line })) });
      toast(`Nahrazeno v ${r.changed} ${plural(r.changed, 'řádku', 'řádcích', 'řádcích')} (${r.files} ${plural(r.files, 'soubor', 'soubory', 'souborů')}). Zpět: „Vrátit poslední změnu souboru“.`, 'ok', 8000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  [find, repl].forEach((i) => i.addEventListener('input', schedule));
  [caseBox, protBox].forEach((i) => i.addEventListener('change', schedule));
  paintMode(); paintScope();
  const body = h('div', null,
    h('p', { class: 'muted' }, 'Hledá a nahrazuje v už přeloženém textu. Značky {i}, proměnné [jméno] a zápisy \\n se ve výchozím nastavení nikdy nemění. Před zápisem se vytvoří záloha; změnu jde vrátit jedním krokem.'),
    h('div', { class: 'row', style: { marginBottom: '8px' } }, find, repl),
    h('div', { class: 'toolbar' }, modeSeg, h('label', { class: 'check' }, caseBox, ' Rozlišovat velikost písmen'), h('label', { class: 'check' }, protBox, ' Nehýbat značky a proměnné')), hint, scopeBox, results);
  S.dlgDirty = true;
  dialog({ title: 'Najít a nahradit', body, buttons: [{ label: 'Zavřít' }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
  setTimeout(() => find.focus(), 0);
  preview();
}

