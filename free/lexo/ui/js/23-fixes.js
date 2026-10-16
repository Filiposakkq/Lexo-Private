import { $, h, plural } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, fname } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
/* Fix speaker / attributes / trailing code from the original (the only operation that edits the .rpy files). */
/* Shows what changed between two texts: common start / end stay plain, the middle is marked. */
export function diffSpans(before, after) {
  const n = Math.min(before.length, after.length);
  let i = 0;
  while (i < n && before[i] === after[i]) i++;
  let j = 0;
  while (j < n - i && before[before.length - 1 - j] === after[after.length - 1 - j]) j++;
  const part = (str, cls) => h('span', null, str.slice(0, i), h('mark', { class: cls }, str.slice(i, str.length - j) || '∅'), str.slice(str.length - j));
  return { before: part(before, 'diff-del'), after: part(after, 'diff-add') };
}

/* Safe fixes: speaker / attributes from the original, double spaces, typo hints. Every line is ticked or unticked
   by the user; doubtful ones start unticked. Variables and tags are never offered (their order may legitimately differ). */
const FIX_KIND_LABEL = { speaker: 'Mluvčí / atributy', spaces: 'Dvojité mezery', typo: 'Překlepy' };
const FIX_KIND_HELP = {
  speaker: 'Mluvčí, atributy a kód za textem se zkopírují z originálu (řádek s #). Text překladu se nemění.',
  spaces: 'Několik mezer za sebou mezi slovy se změní na jednu (jen když je originál nemá taky).',
  typo: 'Návrhy podle slovníku z tvého vlastního překladu. Krátká a nejistá slova jsou odškrtnutá, protože může jít o falešný poplach (byt / být). Zaškrtni jen to, co opravdu chceš.',
};
export async function fixDialog({ items = [], file = '', kinds = ['speaker', 'spaces', 'typo'] }, done) {
  let data;
  try { data = await post('fix/preview', { items, file, kinds }); } catch (e) { return fail(e); }
  if (!data.total) return toast('Není co opravovat.', 'ok');
  const key = (it) => `${it.file}|${it.line}|${it.kind}`;
  const sel = new Set(data.items.filter((it) => it.default && !it.skip).map(key));
  let only = kinds.length === 1 ? kinds[0] : '';
  const head = h('div'), listBox = h('div'), info = h('span', { class: 'grow' });
  const go = h('button', { class: 'btn primary', disabled: true });
  const visible = () => data.items.filter((it) => !only || it.kind === only).slice(0, 600);
  function paint() {
    const seg = h('div', { class: 'seg' }, h('button', { class: only === '' ? 'on' : '', onclick: () => { only = ''; paint(); } }, `Vše (${data.total})`),
      ...Object.keys(data.counts).filter((k) => data.counts[k]).map((k) => h('button', { class: only === k ? 'on' : '', onclick: () => { only = k; paint(); } }, `${FIX_KIND_LABEL[k]} (${data.counts[k]})`)));
    const vis = visible();
    head.replaceChildren(
      only ? h('p', null, FIX_KIND_HELP[only]) : h('p', { class: 'muted' }, 'Vyber, co se má opravit. Před zápisem se vytvoří záloha souboru ve složce ', h('b', null, '_progress_data/backups'), '; změnu jde vrátit tlačítkem „Vrátit poslední změnu souboru“. Soubory musí být uložené v editoru.'),
      h('div', { class: 'toolbar' }, seg, h('span', { class: 'spacer' }),
        h('button', { class: 'btn small', onclick: () => { vis.forEach((it) => { if (!it.skip) sel.add(key(it)); }); paint(); } }, 'Vybrat vše'),
        h('button', { class: 'btn small', onclick: () => { vis.forEach((it) => sel.delete(key(it))); paint(); } }, 'Nic'),
        h('button', { class: 'btn small', title: 'Jen to, co je bezpečné (nejisté návrhy zůstanou odškrtnuté)', onclick: () => { vis.forEach((it) => { if (it.default && !it.skip) sel.add(key(it)); else sel.delete(key(it)); }); paint(); } }, 'Jen doporučené')));
    const rows = vis.map((it) => {
      const d = it.code ? null : diffSpans(it.before, it.after);
      return h('tr', null,
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(key(it)), disabled: !!it.skip, onchange: (e) => { e.target.checked ? sel.add(key(it)) : sel.delete(key(it)); paint2(); } })),
        h('td', null, fname(it.file), h('span', { class: 'mono muted' }, `řádek ${it.line}`), only ? null : h('div', null, chip('none', FIX_KIND_LABEL[it.kind]))),
        h('td', { class: it.code ? 'mono txt' : 'txt' }, d ? d.before : it.before),
        h('td', { class: it.code ? 'mono txt' : 'txt' }, it.skip ? h('span', { class: 'muted' }, it.skip) : (d ? d.after : it.after), !it.code && it.note ? h('div', { class: 'muted', style: { fontSize: '12px' } }, it.note) : null));
    });
    const t = h('table', { class: 'data' }, h('colgroup', null, [44, 190, 340, 340].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', 'Kde', 'Teď', 'Po opravě'].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '914px';
    listBox.replaceChildren(vis.length < data.items.filter((it) => !only || it.kind === only).length ? h('p', { class: 'muted' }, 'Zobrazeno prvních 600 řádků.') : null, h('div', { class: 'tablewrap', style: { maxHeight: '46vh' } }, t));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    go.disabled = !n;
    go.textContent = `Opravit ${n} ${plural(n, 'řádek', 'řádky', 'řádků')}`;
    info.textContent = `Vybráno ${n} z ${data.fixable} použitelných`;
  }
  go.onclick = async () => {
    const selections = data.items.filter((it) => sel.has(key(it))).map((it) => ({ file: it.file, line: it.line, kind: it.kind }));
    try {
      const r = await post('fix/apply', { selections });
      toast(`Opraveno ${r.fixed} ${plural(r.fixed, 'řádek', 'řádky', 'řádků')}` + (r.skipped.length ? `, přeskočeno ${r.skipped.length}` : '') + '. Zpět: „Vrátit poslední změnu souboru“.', r.skipped.length ? 'bad' : 'ok', 8000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  const body = h('div', null, head, listBox);
  paint();
  dialog({ title: 'Opravit', body, buttons: [{ label: 'Zrušit' }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
}

