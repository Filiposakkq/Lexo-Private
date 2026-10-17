import { locale, tn, t as T, tx } from './05-i18n.js';
import { $, fmtN, h, plural } from './00-core.js';
import { clearCtx, get, post } from './10-api.js';
import { S, speakerFull, speakerLabel, speakerMode, speakerName } from './11-state.js';
import { COMPANION, layoutMenu, paintFocusBar, pref, setPref, speakerBadge, speakerColor, speakerModeSelect } from './12-chrome.js';
import { confirmDialog, dialog, fail, status, toast } from './13-feedback.js';
import { ENTRY_LABEL, KIND_LABEL, SEV_LABEL, bar, buildTable, chip, empty, fmtText, fname, gameText, loading, makeSplitter, menuButton, proofChip } from './14-renderers.js';
import { activeChips, filterGroup, pillRow } from './15-filterbar.js';
import { go, render } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
import { announceDraft, announceFocus, copy, openEditor } from './21-files.js';
import { diffSpans, fixDialog } from './23-fixes.js';
import { mtDialog } from './24-machine-translation.js';
import { replaceDialog } from './25-replace.js';
import { ruleApproveDialog } from './26-approve-rules.js';
import { missedTextsDialog } from './36-missed-texts.js';
import { ignoreDialog, unignore } from './28-ignore.js';
/* ================================================================ ENTRIES */
const EN_COLS = () => [
  { k: 'file', w: 360, label: T('File') }, { k: 'line', w: 66, label: T('Line'), sortable: true, title: T('Line in the translation file') },
  { k: 'ref', w: 150, label: T('Script'), title: T("The original line in the game script (game/…:number) as Ren'Py recorded it") },
  { k: 'type', w: 84, label: T('Type'), title: T('Dialogue, or Choice (text from “translate strings” – mostly a menu choice or interface text)') },
  { k: 'status', w: 120, label: T('State') }, { k: 'proof', w: 150, label: T('Proofreading') },
  { k: 'who', w: 90, label: T('Code'), title: T('Speaker abbreviation (and image attributes)') }, { k: 'whoname', w: 170, label: T('Name'), title: T('Full name from the Glossary') }, { k: 'source', w: 420, label: T('Original') }, { k: 'target', w: 420, label: T('Translation') }, { k: 'issues', w: 90, label: T('Check'), sortable: true },
];
function entQuery(extra = {}) {
  const e = pref('entries', {});
  return { file: e.file || '', status: e.status || 'todo', unit: e.unit || '', kind: e.kind || '', issues: e.issues || '', q: e.q || '', sort: e.sort || 'line', limit: 200, ...extra };
}
export async function viewEntries() {
  if (S.goOpts) {
    const o = S.goOpts;
    if (o.file !== undefined) setPref('entries.file', o.file);
    setPref('entries.status', o.status || 'all');
    if (o.q !== undefined) setPref('entries.q', o.q);
    setPref('entries.kind', ''); setPref('entries.issues', '');
    S.ent.pendingLine = o.line || 0;
    S.ent.autoSelect = !!o.select;
    S.goOpts = null;
  }
  const files = S.files || (S.files = (await get('files')).files);
  const e = pref('entries', {});
  const layoutPref = pref('entries.layout', 'auto');
  const film = !COMPANION && layoutPref === 'film';
  const sideBySide = !COMPANION && !film && (layoutPref === 'cols' || (layoutPref === 'auto' && innerWidth >= 1500));
  const root = h('div', { class: 'split ' + (COMPANION ? 'companion' : film ? 'film' : sideBySide ? 'cols ecols' : 'rows'), style: { '--detail-h': pref('entries.detailh', 380) + 'px', '--detail-w': pref('entries.detailw', 620) + 'px' } });
  const main = h('div', { class: 'pane' });
  const detail = h('div', { class: 'detail', id: 'entryDetail' });
  const counter = h('span', { class: 'muted' });
  const search = h('input', { type: 'search', class: 'search', id: 'search', placeholder: T('Search in the text… (Ctrl+F)'), value: e.q || '',
    oninput: (ev) => { setPref('entries.q', ev.target.value); debounceLoad(); } });
  const sel = (key, def, opts, title) => h('select', { title, value: e[key] || def, onchange: (ev) => { setPref('entries.' + key, ev.target.value); load(true); } }, opts.map(([v, l]) => h('option', { value: v }, l)));
  const fileSel = h('select', { title: T('File'), onchange: (ev) => { setPref('entries.file', ev.target.value); load(true); } }, h('option', { value: '' }, T('All files')), files.map((f) => h('option', { value: f.name }, f.name)));
  fileSel.value = e.file || '';
  fileSel.style.maxWidth = '340px';
  const STATUS_OPTS = [['all', T('All')], ['todo', T('To translate')], ['translated', T('Translated ones')], ['approved', T('Approved ones')], ['review', T('For proofreading')], ['fix', T('To fix')], ['stale', T('Changed after proofreading')], ['proofed', T('Proofread ones')]];
  const OPTS = {
    unit: [['', T('Dialogues and choices')], ['dialogue', T('Dialogues only')], ['string', T('Choices only (strings)')]],
    kind: [['', T('All kinds')], ['english', T('In English')], ['same', T('Identical, maybe OK')], ['symbols', T('Symbols only')], ['empty', T('Empty')]],
    issues: [['', T('All')], ['any', T('With a problem')], ['error', T('With an error')], ['warning', T('With a warning')], ['overflow', T('Does not fit the game window')]],
    sort: [['line', T('Line')], ['file', T('File')], ['kind', T('Kind')], ['issues', T('Problems')]],
  };
  const FILTER_NAMES = { unit: T('Type'), kind: T('Kind'), issues: T('Check') };
  const selects = {
    unit: sel('unit', '', OPTS.unit, T('Line type')), kind: sel('kind', '', OPTS.kind, T('Kind of untranslated line')),
    issues: sel('issues', '', OPTS.issues, T('Filter by check')), sort: sel('sort', 'line', OPTS.sort, T('Sorting')),
  };
  const moreBox = h('div', { class: 'frow fmore' }, filterGroup(T('Line type'), selects.unit), filterGroup(T('Kind of untranslated'), selects.kind), filterGroup(T('Check'), selects.issues),
    filterGroup(T('Sort by'), selects.sort), filterGroup(T('Speaker'), speakerModeSelect(() => render())));
  const pillsBox = h('div', { class: 'frow pills', role: 'group', 'aria-label': T('State of the lines') }), chipsBox = h('div', { class: 'frow fchips' });
  const extraCount = () => ['unit', 'kind', 'issues'].filter((k) => pref('entries.' + k, '')).length;
  const moreOpen = () => { const v = pref('entries.more', null); return v === null ? extraCount() > 0 : !!v; };
  const moreBtn = h('button', { class: 'btn small', title: T('Line type, kind, check, sorting, speaker display'), onclick: () => { setPref('entries.more', !moreOpen()); paintFilters(); } });
  function resetFilters() {
    for (const k of ['file', 'unit', 'kind', 'issues', 'q']) setPref('entries.' + k, '');
    setPref('entries.status', 'all');
    search.value = ''; fileSel.value = ''; selects.unit.value = ''; selects.kind.value = ''; selects.issues.value = '';
    load(true);
  }
  function paintFilters() {
    const st = pref('entries.status', 'todo');
    pillsBox.replaceChildren(...pillRow(STATUS_OPTS, st, S.ent.counts, (k) => { setPref('entries.status', k); load(true); }));
    const active = [];
    const lab = (list, v) => (list.find(([k]) => k === v) || [v, v])[1];
    const file = pref('entries.file', '');
    if (file) active.push([T('File: ') + file.split(/[\\/]/).pop(), () => { setPref('entries.file', ''); fileSel.value = ''; load(true); }]);
    if (st !== 'all') active.push([T('State: ') + lab(STATUS_OPTS, st), () => { setPref('entries.status', 'all'); load(true); }]);
    for (const k of ['unit', 'kind', 'issues']) if (pref('entries.' + k, '')) active.push([`${FILTER_NAMES[k]}: ${lab(OPTS[k], pref('entries.' + k, ''))}`, () => { setPref('entries.' + k, ''); selects[k].value = ''; load(true); }]);
    if (pref('entries.q', '')) active.push([T('Search: “{q}”', { q: pref('entries.q', '') }), () => { setPref('entries.q', ''); search.value = ''; load(true); }]);
    chipsBox.replaceChildren(...activeChips(active, resetFilters));
    const open = moreOpen();
    moreBtn.textContent = `${open ? '▾' : '▸'} ${T('More filters')}${extraCount() ? ` (${extraCount()})` : ''}`;
    moreBox.style.display = open ? '' : 'none';
    paintFocusBar();
  }
  const pickInfo = h('span', { class: 'pickinfo' });
  main.append(h('div', { class: 'pane-head' }, h('div', { class: 'filterbar' },
    h('div', { class: 'frow' }, search, filterGroup(T('File'), fileSel), moreBtn,
      menuButton(T('Tools'), [
        { label: T('Find and replace…'), hint: T('in the translations'), run: () => replaceDialog(async () => { S.files = null; await refreshHeader(); load(true); }, S.fl.picked.size ? [...S.fl.picked] : (pref('entries.file', '') ? [pref('entries.file', '')] : [])) },
        { label: T('Approve by rule…'), hint: T('names, interjections'), run: () => ruleApproveDialog(async () => { S.files = null; await refreshHeader(); load(true); }, S.fl.picked.size ? [...S.fl.picked] : (pref('entries.file', '') ? [pref('entries.file', '')] : [])) },
        { label: T('Texts the generator missed…'), hint: T('show text, input prompts, titles'), run: () => missedTextsDialog(async () => { S.files = null; await refreshHeader(); load(true); }) },
        { label: T('Fix…'), hint: T('speaker, spaces, typos'), run: () => fixDialog({ file: pref('entries.file', '') }, async () => { S.files = null; await refreshHeader(); load(true); }) },
        { label: T('Translate with a machine…'), hint: T('selected lines, scene, file'), run: () => openMtDialog() },
      ], { title: T('Bulk edits of translations') }),
      pickInfo, h('span', { class: 'spacer' }), counter,
      layoutMenu()),
    pillsBox, moreBox, chipsBox)));
  paintFilters();
  const wrap = h('div', { class: 'pane-body' });
  main.append(wrap);
  root.append(main, sideBySide ? makeSplitter(root, 'h', { key: 'entries.detailw', def: 620, min: 320, maxFrac: 0.72 }) : makeSplitter(root, 'v', { key: 'entries.detailh', def: 380, min: 140, maxFrac: 0.7 }), detail);
  let timer;
  const debounceLoad = () => { clearTimeout(timer); timer = setTimeout(() => load(true), 250); };
  let tbody;

  function skeleton() {
    const showFile = !pref('entries.file', '');
    const mode = speakerMode();
    const cols = EN_COLS().filter((c) => (showFile || c.k !== 'file') && !(c.k === 'who' && mode === 'name') && !(c.k === 'whoname' && mode === 'code'));
    const t = buildTable({ key: 'entries' + (showFile ? '' : '1'), cols, className: 'entries' });
    tbody = t.tbody;
    const more = h('div', { class: 'more' });
    wrap.replaceChildren(h('div', { class: 'tablewrap' }, t.table), more);
    S.ent.more = more; S.ent.showFile = showFile;
  }
  const pickKey = (it) => it.file + '\t' + it.line;
  const picked = () => (S.ent.picked ||= new Set());
  function paintPicks() {
    [...tbody.children].forEach((tr) => { const it = S.ent.items[+tr.dataset.i]; tr.classList.toggle('picked', !!it && picked().has(pickKey(it))); });
    const n = picked().size;
    pickInfo.replaceChildren(...(n ? [tn(n, 'Selected {n} line', 'Selected {n} lines') + ' ', h('button', { class: 'btn small ghost', onclick: () => { picked().clear(); paintPicks(); } }, T('Clear the selection')),
      h('button', { class: 'btn small', onclick: () => openMtDialog() }, T('Translate with a machine…'))] : []));
  }
  function rowClick(ev, i, it) {
    if (ev.ctrlKey || ev.metaKey) { const k = pickKey(it); picked().has(k) ? picked().delete(k) : picked().add(k); paintPicks(); return; }
    if (ev.shiftKey && S.ent.sel >= 0) {
      const [a, b] = [Math.min(S.ent.sel, i), Math.max(S.ent.sel, i)];
      for (let j = a; j <= b; j++) picked().add(pickKey(S.ent.items[j]));
      paintPicks(); return;
    }
    if (picked().size) { picked().clear(); paintPicks(); }
    select(i);
  }
  /* Machine translation of the picked lines, the lines around the open one, the file in the filter, or everything. */
  function openMtDialog() {
    const cur = S.ent.items[S.ent.sel];
    mtDialog({ picked: [...picked()].map((k) => { const [file, line] = k.split('\t'); return { file, line: +line }; }),
      anchor: cur ? { file: cur.file, line: cur.line } : null, file: pref('entries.file', '') }, async () => { picked().clear(); S.files = null; await refreshHeader(); load(true); });
  }
  function rowFor(it, i) {
    const hasErr = it.issues.some((x) => x.severity === 'error');
    const sev = hasErr ? 'error' : it.issues.length ? 'warning' : '';
    return h('tr', { class: `status-${it.status} ${hasErr ? 'has-error' : ''} ${it.issues.some((x) => x.code === 'textbox-overflow') ? 'ovf' : ''} ${S.ent.sel === i ? 'sel' : ''} ${picked().has(pickKey(it)) ? 'picked' : ''}`, dataset: { i }, onclick: (ev) => rowClick(ev, i, it), ondblclick: () => openEditor(it.file, it.line) },
      S.ent.showFile ? h('td', null, fname(it.file)) : null,
      h('td', { class: 'num' }, it.line), h('td', { class: 'mono muted', style: { fontSize: '12px' } }, it.ref ? it.ref.replace(/^game\//, '') : '–'),
      h('td', null, it.unit === 'string' ? h('span', { class: 'chip choice', title: T('Text from “translate strings” – mostly a menu choice or interface text') }, T('Choice')) : h('span', { class: 'muted' }, T('Dialogue'))),
      h('td', null, chip(it.status, ENTRY_LABEL[it.status]), it.kind && it.status !== 'translated' ? h('div', { class: 'muted', style: { fontSize: '11.5px' } }, KIND_LABEL[it.kind]) : null),
      h('td', null, it.status === 'todo' ? h('span', { class: 'muted' }, '–') : (proofChip(it.proof) || h('span', { class: 'muted' }, T('Not proofread')))),
      speakerMode() !== 'name' ? h('td', { class: 'who' }, speakerBadge(it.who), it.who ? ' ' : null, it.who) : null,
      speakerMode() !== 'code' ? h('td', { class: 'whoname' }, speakerName(it.who) || h('span', { class: 'muted' }, it.who ? '–' : '')) : null,
      h('td', { class: 'txt entry-source' }, fmtText(it.source ?? '')), h('td', { class: 'txt entry-target' }, fmtText(it.target)),
      h('td', null, sev ? chip(sev, `${it.issues.length}×`) : null));
  }
  /* Detail window: there is no list, only the one line it follows (loading a whole list for every change was slow and racy). */
  async function followLine(file, line) {
    let d;
    clearCtx();                                         // the main window may have changed neighbouring lines
    prefetchScene(file, line).catch(() => {});          // starts at once, in parallel with the detail
    try { d = await get('entry', { file, line }); } catch (err) { S.ent.items = [{ file, line }]; S.ent.detail = null; return drawDetail(null, err.message); }
    const it = { file, line, key: d.key, status: d.status, proof: d.proof, kind: d.kind, unit: d.unit, who: d.who, ref: d.ref, source: d.source, target: d.target,
      issues: d.issues.filter((x) => x.severity !== 'info').map((x) => ({ severity: x.severity, code: x.code })) };
    S.ent.items = [it]; S.ent.sel = -1; S.ent.total = 1; S.ent.start = 0;
    tbody.replaceChildren(rowFor(it, 0));
    await select(0);
  }
  async function load(reset) {
    if (COMPANION) { const cur = S.ent.items[0]; return cur ? followLine(cur.file, cur.line) : drawDetail(null); }
    if (S.ent.loading) return;
    S.ent.loading = true;
    try {
      if (reset) { S.ent.items = []; S.ent.sel = -1; S.ent.start = 0; skeleton(); tbody.replaceChildren(); S.ent.more.replaceChildren(loading()); }
      const around = reset && S.ent.pendingLine ? S.ent.pendingLine : 0;
      const q = entQuery(reset ? { offset: 0, around } : { offset: S.ent.start + S.ent.items.length });
      if (around) q.status = 'all';
      const r = await get('entries', q);
      if (reset) S.ent.start = r.offset;
      const base = S.ent.items.length;
      S.ent.items.push(...r.items);
      S.ent.total = r.total; S.ent.counts = r.counts; paintFilters();
      r.items.forEach((it, k) => tbody.append(rowFor(it, base + k)));
      counter.textContent = tn(r.total, '{n} line', '{n} lines', { n: fmtN(r.total) });
      const shown = S.ent.start + S.ent.items.length;
      S.ent.more.replaceChildren(shown < r.total ? h('button', { class: 'btn', onclick: () => load(false) }, T('Load {n} more (shown {shown} of {total})', { n: Math.min(200, r.total - shown), shown: fmtN(shown), total: fmtN(r.total) })) : null);
      if (!r.total) S.ent.more.replaceChildren(empty(T('Nothing found'), pref('entries.status', 'todo') === 'todo' ? T('There is nothing left to translate here – or change the filters.') : T('Try changing the filters or the search text.')));
      if (reset && around) { const i = S.ent.items.findIndex((x) => x.line === around); S.ent.pendingLine = 0; if (i >= 0) select(i); }
      else if (reset && S.ent.items.length) { if (S.ent.autoSelect) { S.ent.autoSelect = false; select(0); } else drawDetail(null); }
      else if (reset) drawDetail(null);
    } catch (err) { fail(err); S.ent.more?.replaceChildren(h('div', { class: 'notice bad' }, err.message)); }
    finally { S.ent.loading = false; }
  }
  async function select(i) {
    if (i < 0 || i >= S.ent.items.length) return;
    const open = $('#editTarget');
    if (open?.dataset.dirty === '1' && !(await confirmDialog(T('Unsaved translation'), T('You changed the text of this line and did not save it. Discard the changes?'), T('Discard the changes')))) return;
    tbody.querySelector('tr.sel')?.classList.remove('sel');
    S.ent.sel = i;
    if (S.quick === 'tr') S.ent.focusEdit = true;
    const tr = tbody.children[i];
    tr?.classList.add('sel');
    tr?.scrollIntoView({ block: 'nearest' });
    const it = S.ent.items[i];
    if (!COMPANION) {
      announceFocus(it);
      (S.prefs.ui ||= {}).last = { file: it.file, line: it.line };                    // in memory at once, written after a pause
      // the preferences may have been replaced (reloaded from the server) during the pause: write the position only if it still exists
      clearTimeout(S.lastT); S.lastT = setTimeout(() => { const last = S.prefs.ui?.last; if (last) setPref('ui.last', last); }, 6000);
    }
    const tab = pref('ui.detailTab', 'scene');
    const pre = (COMPANION ? (S.compFull || tab === 'scene') : (film || (tab === 'scene' && !(S.companionOpen && pref('ui.slimMain', true))))) ? prefetchScene(it.file, it.line).catch(() => {}) : null;
    try { S.ent.detail = await get('entry', { file: it.file, line: it.line }); } catch (err) { S.ent.detail = null; drawDetail(null, err.message); return; }
    if (pre) await pre;
    if (S.ent.sel === i) { const top = detail.scrollTop; drawDetail(S.ent.detail); detail.scrollTop = top; requestAnimationFrame(() => { if (S.ent.sel === i) detail.scrollTop = top; }); }
    if (film) for (const k of [i + 1, S.quick === 'proof' ? nextToReview(i) : i + 2]) { const x = S.ent.items[k]; if (x && k !== i) prefetchScene(x.file, x.line).catch(() => {}); }
    if (i >= S.ent.items.length - 15 && S.ent.start + S.ent.items.length < S.ent.total) load(false);
  }
  function drawDetail(d, err) {
    if (!d) return detail.replaceChildren(h('h4', null, T('Line detail')), err ? h('div', { class: 'notice bad' }, err) : h('p', { class: 'muted' }, tx('Select a line. The arrow keys ↑ ↓ move through the lines, {a} approves, {e} opens the editor, {q} shows all shortcuts.', { a: h('kbd', null, 'A'), e: h('kbd', null, 'E'), q: h('kbd', null, '?') })));
    const canApprove = d.status !== 'translated' && d.kind !== 'empty';
    const ed = translationEditor(d);
    detail.replaceChildren(
      COMPANION ? companionBar() : null,
      h('div', { class: 'dhead' },
        h('span', { class: 'path', title: d.ref ? T('original script: {ref}', { ref: d.ref }) : '' }, `${d.file}:${d.line}`),
        d.unit === 'string' ? chip('choice', T('Choice')) : chip('none', T('Dialogue')),
        chip(d.status, ENTRY_LABEL[d.status] + (d.kind && d.status !== 'translated' ? ` · ${KIND_LABEL[d.kind]}` : '')),
        d.status !== 'todo' ? (proofChip(d.proof) || chip('none', T('Not proofread'))) : null,
        h('span', { class: 'spacer' }),
        canApprove ? h('button', { class: 'btn small primary', onclick: () => toggleApprove() }, d.status === 'approved' ? T('Unapprove (A)') : T('Approve (A)')) : null,
        h('button', { class: 'btn small', title: T('Opens the file at this line in an external editor'), onclick: () => openEditor(d.file, d.line) }, T('Editor (E)')),
        menuButton(T('More'), [
          ...(canApprove ? [{ label: T('Approve all identical…'), hint: 'Shift+A', run: () => globalApprove() }] : []),
          ...(!COMPANION && S.companionOpen ? [{ label: pref('ui.slimMain', true) ? T('Show the tabs in this window too') : T('Tabs only in the second window'), run: () => { setPref('ui.slimMain', !pref('ui.slimMain', true)); drawDetail(S.ent.detail); } }] : []),
          { label: T('Copy the path'), run: () => copy(`${d.path}:${d.line}`) },
        ], { title: T('More actions with the line') })),
      ...layoutParts(d, ed, h('div', { class: 'pair' },
        h('div', null, h('div', { class: 'row', style: { justifyContent: 'space-between' } }, h('h4', { style: { margin: 0 } }, T('Original')),
          h('span', { class: 'row' }, h('button', { class: 'btn small ghost', title: T('Copies the original text to the clipboard'), onclick: () => copy(d.source ?? '') }, T('⧉ Original')),
            h('button', { class: 'btn small ghost', title: T('Copies the original and the translation one under the other'), onclick: () => copy(`${d.source ?? ''}\n${$('#editTarget') ? $('#editTarget').value : d.target}`) }, T('⧉ Both')))),
          h('div', { class: 'textbox', style: { marginTop: '6px' } }, fmtText(d.source ?? T('(source not found)')))),
        ed), () => detailTabs(d, ed)));
  }
  /* With a detail window open the main window keeps the translating (original + editor) and hands the tabs (scene, check, proofreading…) to it;
     the detail window can show the editor as well. */
  function goLine(file, line) {
    const i = S.ent.items.findIndex((x) => x.file === file && x.line === line);
    if (i >= 0) select(i); else go('entries', { file, line, status: 'all', select: true });
  }
  /* Scenes of a file (from the `scene` statements of the game script), fetched once per file and kept until something is written. */
  function scenesFor(file) {
    S.scenesCache ||= {}; S.scenesDone ||= {};
    if (!S.scenesCache[file]) S.scenesCache[file] = get('scenes', { file, line: 0 }).then((r) => { S.scenesDone[file] = r; return r; }).catch(() => { const r = { available: false, items: [] }; S.scenesDone[file] = r; return r; });
    return S.scenesCache[file];
  }
  async function fileRowsReady() { if (!S.files) S.files = (await get('files')).files; S.filesOld = S.files; return S.files; }
  /* Repaint the strips in place (e.g. after P: the neighbouring card gets its tick, the file counters move) without redrawing the whole detail. */
  /* Counters move at once from what we know; the real numbers (overview, files, scenes) are fetched only after a short pause. */
  function bumpLocal(file, line, dDone, dProof) {
    const row = (S.files || S.filesOld || []).find((x) => x.name === file);
    if (row) { row.done = Math.max(0, row.done + dDone); row.todo = Math.max(0, row.todo - dDone); row.corr = Math.max(0, row.corr + dProof); }
    const sc = S.scenesDone?.[file];
    if (sc?.items?.length) { let k = 0; sc.items.forEach((it, i) => { if (it.line <= line) k = i; }); const it = sc.items[k]; it.done = Math.max(0, it.done + dDone); it.proofed = Math.max(0, it.proofed + dProof); }
    refreshFileStrip();
  }
  function lazyRefresh() {
    clearTimeout(S.lazyT);
    S.lazyT = setTimeout(async () => {
      const file = S.ent.detail?.file;
      S.files = null; S.scenesCache = {};
      try { await refreshHeader(); if (file && S.view === 'entries') { await Promise.all([fileRowsReady(), scenesFor(file)]); refreshFileStrip(); } } catch { /* the next pause tries again */ }
    }, 2500);
  }
  function refreshFileStrip() { const el = $('#entryDetail .filestrip'), d = S.ent.detail; if (el && d) el.replaceWith(fileStrip(d, true)); }
  function refreshStrips() { const d = S.ent.detail; if (!d) return; $('#entryDetail .filmstrip')?.replaceWith(filmStrip(d)); refreshFileStrip(); }
  async function jumpNext(file, line, kind) {
    try {
      const r = await get('entry/next', { file, line, kind });
      if (!r.line) return toast(T('There is no such line left in this file.'), 'ok', 2200);
      if (r.wrapped) toast(T('The search continues from the start of the file.'), 'ok', 1600);
      goLine(file, r.line);
    } catch (e) { fail(e); }
  }
  /* Progress of the file the line belongs to (translation and proofreading apart) and jumps: next scene, next untranslated / unchecked / too long. */
  function fileStrip(d, retried = false) {
    const box = h('div', { class: 'filestrip' });
    const row = (S.files || S.filesOld || []).find((x) => x.name === d.file);
    const sc = S.scenesDone?.[d.file];
    const pct = (a, b) => (b ? Math.round((a / b) * 100) : 0);
    const bars = row ? h('div', { class: 'fsbars' },
      h('div', { class: 'fsb' }, h('span', null, T('Translated {n} %', { n: pct(row.done, row.total) })), bar(pct(row.done, row.total)), h('span', { class: 'muted' }, T('{n} left to translate', { n: fmtN(row.todo) }))),
      h('div', { class: 'fsb' }, h('span', null, T('Proofread {n} %', { n: pct(row.corr, row.total) })), bar(pct(row.corr, row.total), 'alt'), h('span', { class: 'muted' }, T('{n} left to check', { n: fmtN(Math.max(0, row.total - row.corr)) })))) : null;
    const items = sc?.available ? sc.items : [];
    let cur = -1; items.forEach((it, i) => { if (it.line <= d.line) cur = i; });
    const goScene = (i) => { if (items[i]) goLine(d.file, items[i].line); };
    const nav = h('div', { class: 'fsnav' },
      items.length ? h('button', { class: 'btn small', disabled: cur <= 0, title: T('First line of the previous scene'), onclick: () => goScene(cur - 1) }, T('◀ Scene')) : null,
      items.length ? menuButton(T('Scene {n} / {total}', { n: cur + 1, total: items.length }), items.map((it, i) => ({ label: `${i + 1}. ${it.name || '—'}`, hint: `${it.done}/${it.count} · ✓ ${it.proofed}`, run: () => goScene(i) })), { title: T('List of the scenes of the file: translated / lines · proofread') }) : null,
      items.length ? h('button', { class: 'btn small', disabled: cur < 0 || cur >= items.length - 1, title: T('First line of the next scene'), onclick: () => goScene(cur + 1) }, T('Scene ▶')) : null,
      h('span', { class: 'spacer' }),
      h('button', { class: 'btn small', title: T('Next untranslated line in the file'), onclick: () => jumpNext(d.file, d.line, 'todo') }, T('Next untranslated')),
      h('button', { class: 'btn small', title: T('Next translated but unchecked line'), onclick: () => jumpNext(d.file, d.line, 'review') }, T('Next unchecked')),
      h('button', { class: 'btn small', title: T('Next line that does not fit the game window'), onclick: () => jumpNext(d.file, d.line, 'over') }, T('Next one that does not fit')));
    box.append(h('div', { class: 'fsname' }, fname(d.file)), bars, nav);
    if (!retried && (!row || !sc)) { fileRowsReady().then(() => scenesFor(d.file)).then(() => { if (box.isConnected) box.replaceWith(fileStrip(d, true)); }); }
    return box;
  }
  /* The lines around the current one as a horizontal strip of cards (speaker badge, number, text), the current one centred. */
  function filmStrip(d) {
    const strip = h('div', { class: 'filmstrip', role: 'list', 'aria-label': T('Lines of the scene') });
    const radius = pref('ui.sceneRadius', 6);
    const center = () => { const c = strip.querySelector('.cur'); if (c) strip.scrollLeft = c.offsetLeft - (strip.clientWidth - c.offsetWidth) / 2; };
    const fill = (r) => {
      strip.replaceChildren(...r.items.map((it) => h('button', { class: `fcard st-${it.status}${it.current ? ' cur' : ''}${it.over ? ' over' : ''}`, role: 'listitem', title: T('Line {n}', { n: it.line }), onclick: () => goLine(d.file, it.line) },
        h('div', { class: 'fhead' }, speakerBadge(it.who), h('span', { class: 'fnum' }, it.line),
          it.over ? h('span', { class: 'fmark over', title: T('The text does not fit the game window') }, '⤓') : null,
          it.proof === 'ok' ? h('span', { class: 'fmark ok', title: T('Proofread') }, '✓') : it.proof === 'fix' ? h('span', { class: 'fmark fix', title: T('To fix') }, '⚑') : null),
        h('div', { class: 'ftxt' }, it.status === 'todo' ? h('span', { class: 'muted' }, fmtText(it.source ?? '')) : fmtText(it.target)))));
      center();
    };
    const peek = S.ctxDone?.get(ctxKey(d.file, d.line, radius));
    if (peek) fill(peek); else { strip.append(loading()); contextFor(d.file, d.line, radius).then(fill).catch((e) => strip.replaceChildren(h('div', { class: 'notice bad' }, e.message))); }
    requestAnimationFrame(center);
    return strip;
  }
  function layoutParts(d, ed, pair, tabsFn) {
    if (COMPANION) {
      S.compDraft = (t) => { if (!S.compEditor) { ed.ta.value = t; ed.ta.dispatchEvent(new Event('draft')); } };
      const editor = S.compEditor ? pair : null;
      return (S.compFull ? [inPane(gamePreview(d, ed.ta)), editor] : [editor, tabsFn()]).filter(Boolean);
    }
    if (film) return [inPane(gamePreview(d, ed.ta)), pair, fileStrip(d), filmStrip(d), ...(S.quick === 'proof' ? [proofPane(d)] : S.quick ? [] : [tabsFn()])];
    if (S.companionOpen && pref('ui.slimMain', true)) {
      return [pair, h('div', { class: 'slimnote muted' }, T('The scene, check, proofreading and other tabs are in the second window. '),
        h('button', { class: 'btn small ghost', onclick: () => { setPref('ui.slimMain', false); drawDetail(S.ent.detail); } }, T('Show them here too')))];
    }
    return [pair, tabsFn()];
  }
  /* Detail window: follows the main window; the scene can take the whole window (the editor stays under it). */
  function companionBar() {
    const setFull = (v) => {
      S.compFull = v;
      const ta = $('#editTarget'), keep = ta && ta.dataset.dirty === '1' ? ta.value : null;
      drawDetail(S.ent.detail);
      if (keep !== null) { const n = $('#editTarget'); n.value = keep; n.dispatchEvent(new Event('input')); }
    };
    return h('div', { class: 'followbar' },
      h('label', { class: 'check', title: T('The window shows the line I click in the main window') }, h('input', { type: 'checkbox', checked: S.follow !== false, onchange: (e) => { S.follow = e.target.checked; S.followTarget = ''; if (S.follow) S.followApply?.(S.lastFocus || {}); } }), T(' Follow the main window')),
      h('span', { class: 'spacer' }),
      h('label', { class: 'check', title: T('Translate in this window too (otherwise translating is done in the main one and this window shows the scene and the checks)') }, h('input', { type: 'checkbox', checked: !!S.compEditor, onchange: (e) => { S.compEditor = e.target.checked; drawDetail(S.ent.detail); } }), T(' Editor here too')),
      h('div', { class: 'seg' }, h('button', { class: S.compFull ? '' : 'on', onclick: () => setFull(false) }, T('Scene and detail')), h('button', { class: S.compFull ? 'on' : '', onclick: () => setFull(true) }, T('Scene over the whole window'))));
  }
  /* Secondary panels of the line detail live in tabs: only the chosen one is built (and fetches its data). */
  const inPane = (el) => { if (!el.dataset.keep) el.open = true; el.classList.add('inpane'); return el; };
  const issuesPane = (d) => (
        d.issues.length ? h('div', null, d.issues.map((x) => h('div', { class: 'issue ' + x.severity + (x.ignored ? ' ignored' : '') }, x.ignored ? chip('none', T('ignored')) : chip(x.severity, SEV_LABEL[x.severity]), ' ', x.message,
        x.ignored ? h('span', { class: 'muted' }, x.ignore_note ? ` – ${x.ignore_note}` : '') : null,
        h('div', null, x.ignored ? h('button', { class: 'btn small ghost', onclick: () => unignore([{ file: d.file, line: d.line, code: x.code }], () => { S.ent.api.reload(); }) }, T('Stop ignoring'))
          : h('button', { class: 'btn small ghost', title: T('Hides this finding on this line (can be undone)'), onclick: () => ignoreDialog(d.issues.filter((y) => !y.ignored).map((y) => ({ file: d.file, line: d.line, code: y.code, severity: y.severity, message: y.message })).sort((a, b) => (a.code === x.code ? -1 : b.code === x.code ? 1 : 0)), () => { S.ent.api.reload(); }) }, T('Ignore…'))),
        x.fixable ? h('div', null, h('button', { class: 'btn small primary', onclick: () => fixDialog({ items: [{ file: d.file, line: d.line }], kinds: [x.kind || 'speaker'] }, () => { load(true); }) }, x.kind === 'speaker' || !x.kind ? T('Fix from the original') : T('Fix…'))) : null,
        x.ratio ? h('div', { class: 'muted' }, T('Length: original {a}, translation {b} characters (ratio {r}). Decide yourself whether it is intended.', { a: x.source_len, b: x.target_len, r: new Intl.NumberFormat(locale(), { maximumFractionDigits: 2 }).format(x.ratio) })) : null))) : h('div', { class: 'muted' }, T('The line has no findings.')));
  const proofPane = (d) => (
        d.status !== 'todo' ? h('div', { class: 'proofbox' },
        h('div', { class: 'row' }, h('h4', { style: { margin: 0 } }, T('Proofreading')), proofChip(d.proof) || h('span', { class: 'muted' }, T('Not proofread yet')),
          d.proof_date && d.proof === 'ok' ? h('span', { class: 'muted' }, d.proof_date) : null),
        h('div', { class: 'actions' },
          h('button', { class: 'btn small ' + (d.proof === 'ok' ? '' : 'primary'), onclick: () => toggleProof() }, d.proof === 'ok' ? T('Remove the mark (P)') : T('✓ Proofread (P)')),
          h('button', { class: 'btn small', onclick: () => flagFix() }, d.proof === 'fix' ? T('Clear “to fix” (F)') : T('⚑ To fix (F)')),
          h('button', { class: 'btn small', title: T('Marks all translated lines of this file from the start up to this line as proofread. Lines marked “to fix” do not change. It can be undone (Ctrl+Z).'), onclick: () => proofUpto('ok') }, T('✓ Proofread everything up to here (Shift+P)')),
          h('button', { class: 'btn small', title: T('Removes the “Proofread” mark from all lines of this file from the start up to this line. Lines marked “to fix” stay. It can be undone (Ctrl+Z).'), onclick: () => proofUpto('') }, T('↺ Remove the proofread mark up to here (Shift+U)'))),
        h('textarea', { id: 'proofNote', rows: 2, placeholder: T('Note for the proofreading (what to fix)…'), value: d.proof_note || '', style: { width: '100%' },
          onchange: (ev) => saveNote(ev.target.value) })) : h('div', { class: 'muted' }, T('The line is not translated yet – proofreading is done after translating.')));
  const scriptPane = (d) => h('div', null,
      h('div', { class: 'ctx' }, T('Block: '), h('b', { class: 'mono' }, d.block), d.who ? [T('  ·  speaker: '), h('b', { class: 'mono' }, speakerFull(d.who))] : null, d.src_line ? '  ·  ' + T('original on line {n}', { n: d.src_line }) : null),
    scriptContext(d));
  function detailTabs(d, ed) {
    const nIssues = d.issues.filter((x) => !x.ignored).length;
    const needsEditor = new Set(['memory', 'history']);
    const defs0 = [
      ['check', nIssues ? T('Check ({n})', { n: nIssues }) : T('Check'), () => issuesPane(d)],
      ['proof', T('Proofreading'), () => proofPane(d)],
      ...(d.unit === 'string' && d.ref ? [['menu', T('Menu'), () => inPane(menuBox(d))]] : []),
      ['scene', T('Scene'), () => h('div', null, inPane(gamePreview(d, ed.ta)), inPane(sceneBox(d)))],
      ['memory', T('Memory'), () => inPane(memoryPanel(d, ed.ta, ed.insert))],
      ['history', T('History'), () => inPane(historyPanel(d, ed.ta))],
      ['script', T('Script'), () => scriptPane(d)],
    ];
    const defs = (COMPANION && !S.compEditor ? defs0.filter(([k]) => !needsEditor.has(k)) : defs0).filter(([k]) => !(film && k === 'scene'));
    let cur = pref('ui.detailTab', nIssues ? 'check' : 'scene');
    if (d.unit === 'string' && cur === 'scene' && defs.some(([k]) => k === 'menu')) cur = 'menu';       // a choice is read together with its menu, not with a scene
    if (!defs.some(([k]) => k === cur)) cur = defs[0][0];
    const bar = h('div', { class: 'dtabs', role: 'tablist' }), pane = h('div', { class: 'dpane' });
    function show(k) {
      cur = k; setPref('ui.detailTab', k);
      bar.replaceChildren(...defs.map(([key, label]) => h('button', { class: 'dtab' + (key === k ? ' on' : '') + (key === 'check' && nIssues ? ' warn' : ''), role: 'tab', 'aria-selected': String(key === k), onclick: () => show(key) }, label)));
      pane.replaceChildren(defs.find(([key]) => key === k)[2]());
    }
    show(cur);
    return h('div', { class: 'dtabswrap' }, bar, pane);
  }
  function translationEditor(d) {
    const ta = h('textarea', { id: 'editTarget', class: 'ed editbox', rows: 3, spellcheck: 'true', lang: 'cs', 'aria-label': T('Translation') });
    ta.value = d.target;
    const msg = h('div', { class: 'editmsg' });
    const save = h('button', { class: 'btn small primary', disabled: true, title: T('Writes the text to the .rpy file (a backup is made before writing)') }, T('Save to the file (Ctrl+Enter)'));
    let timer, seq = 0;
    const dirty = () => ta.value !== d.target;
    const sync = () => { if (dirty()) ta.dataset.dirty = '1'; else delete ta.dataset.dirty; save.disabled = !dirty(); };
    async function check() {
      const my = ++seq;
      if (!dirty()) { msg.replaceChildren(); return; }
      try {
        const r = await post('entry/check', { file: d.file, line: d.line, text: ta.value });
        if (my !== seq) return;
        msg.replaceChildren(h('h4', null, T('Check of the edited text')),
          r.literal_error ? h('div', { class: 'issue error' }, chip('error', T('Cannot save')), ' ', r.literal_error) : null,
          ...r.issues.map((x) => h('div', { class: 'issue ' + x.severity }, chip(x.severity, SEV_LABEL[x.severity]), ' ', x.message)),
          !r.literal_error && !r.issues.length ? h('div', { class: 'muted' }, T('No findings.')) : null);
        save.disabled = !dirty() || !!r.literal_error;
      } catch { /* the live check is only a hint */ }
    }
    async function doSave(advance) {
      if (!dirty() || save.disabled) return;
      save.disabled = true;
      try { await post('entry/save', { file: d.file, line: d.line, text: ta.value }); }
      catch (err) { save.disabled = false; return fail(err); }
      delete ta.dataset.dirty;
      if (d.status === 'todo') bumpLocal(d.file, d.line, 1, 0);
      lazyRefresh();
      const i = S.ent.sel, it = S.ent.items[i];
      try {
        const nd = await get('entry', { file: d.file, line: d.line });
        if (it) { Object.assign(it, { target: nd.target, status: nd.status, proof: nd.proof, kind: nd.kind, issues: nd.issues.filter((x) => x.severity !== 'info').map((x) => ({ severity: x.severity, code: x.code })) }); refreshRow(i); }
        toast(T('Saved to the file ✓'), 'ok', 1800);
        if (advance) {
          let k = i + 1;
          while (k < S.ent.items.length && S.ent.items[k].status !== 'todo') k++;
          S.ent.focusEdit = true;
          select(k < S.ent.items.length ? k : Math.min(i + 1, S.ent.items.length - 1));
        } else select(i);
      } catch (err) { fail(err); }
    }
    ta.addEventListener('input', () => { sync(); clearTimeout(timer); timer = setTimeout(check, 300); if (S.companionOpen && !COMPANION) announceDraft(d, ta.value); });
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); doSave(true); }
      else if (e.key === 'Escape') { ta.value = d.target; sync(); check(); ta.blur(); }
    });
    save.onclick = () => doSave(false);
    /* snippets: insert at the cursor, or wrap the selection for paired tags */
    const insert = (text) => { const a = ta.selectionStart, b = ta.selectionEnd; ta.setRangeText(text, a, b, 'end'); ta.focus(); ta.dispatchEvent(new Event('input')); };
    const wrap = (open, close) => { const a = ta.selectionStart, b = ta.selectionEnd; ta.setRangeText(open + ta.value.slice(a, b) + close, a, b, 'end'); if (a === b) { ta.selectionStart = ta.selectionEnd = a + open.length; } ta.focus(); ta.dispatchEvent(new Event('input')); };
    const snip = (label, fn, title) => h('button', { class: 'btn small snip', title: title || '', onclick: (e) => { e.preventDefault(); fn(); } }, label);
    const tagBar = h('div', { class: 'snips' }, h('span', { class: 'muted' }, T('Insert:')),
      snip('{i}…', () => wrap('{i}', '{/i}'), T('Italic – wraps the selected text')), snip('{b}…', () => wrap('{b}', '{/b}'), T('Bold')), snip('{u}…', () => wrap('{u}', '{/u}'), T('Underlined')),
      snip('{w}', () => insert('{w}'), T('Pause, waits for a click')), snip('{p}', () => insert('{p}'), T('A new paragraph with a pause')), snip('{nw}', () => insert('{nw}'), T('No waiting for a click')),
      snip('\\n', () => insert('\\n'), T('Line break (written as \\n in the game text)')), snip('…', () => insert('…'), T('Three dots as one character')), snip('„ “', () => wrap('„', '“'), T('Czech quotation marks – wraps the selected text')));
    const tokenRe = /\{[^}]*\}|\[[^\]]*\]/g;
    const missing = h('div', { class: 'snips' });
    const terms = h('div', { class: 'snips' });
    const paintMissing = () => {
      const have = {}; (ta.value.match(tokenRe) || []).forEach((t) => { have[t] = (have[t] || 0) + 1; });
      const need = {}; ((d.source || '').match(tokenRe) || []).forEach((t) => { need[t] = (need[t] || 0) + 1; });
      const miss = Object.keys(need).filter((t) => !t.startsWith('{#') && (have[t] || 0) < need[t]);
      missing.replaceChildren(...(miss.length ? [h('span', { class: 'muted' }, T('Missing from the original:')), ...miss.map((t) => snip(t, () => insert(t), T('Inserts a tag or variable from the original at the cursor'))),
        miss.length > 1 ? snip(T('all'), () => insert(miss.join('')), T('Inserts all missing ones at the cursor')) : null] : []));
    };
    const paintTerms = () => {
      terms.replaceChildren(...((d.terms || []).length ? [h('span', { class: 'muted' }, T('Glossary:')), ...d.terms.map((t) => {
        const used = ta.value.toLowerCase().includes(t.cz.toLowerCase().slice(0, Math.max(3, t.cz.length - 2)));
        return snip(`${used ? '✓ ' : ''}${t.en} → ${t.cz}`, () => insert(t.cz), T('Inserts the translated term from the Glossary at the cursor'));
      })] : []));
    };
    ta.addEventListener('input', () => { paintMissing(); paintTerms(); });
    paintMissing(); paintTerms();
    const reset = h('button', { class: 'btn small', onclick: () => { ta.value = d.target; sync(); check(); } }, T('Restore the text'));
    const copyTgt = h('button', { class: 'btn small ghost', title: T('Copies the text of the translation field to the clipboard'), onclick: () => copy(ta.value) }, T('⧉ Translation'));
    const copySrc = h('button', { class: 'btn small', title: T('Puts the original text into the field (handy as a base with the right tags already)'), onclick: () => { ta.value = d.source ?? ''; sync(); check(); ta.focus(); } }, T('Original into the field'));
    const undoLast = h('button', { class: 'btn small ghost', title: T('Restores the last change to an .rpy file made by this app (a text edit or a speaker fix) from the backup'), onclick: async () => {
      try { const r = await post('fix/undo'); toast(T('Undone: {what}', { what: r.label || T('last change') }), 'ok'); S.files = null; refreshHeader(); load(true); } catch (err) { fail(err); }
    } }, T('↶ Undo the last file change'));
    /* Optional machine translation: a suggestion only, shown below, inserted when you click. Visible only when set up in Nastavení. */
    const mtBox = h('div', { class: 'mtbox' });
    const mtBtn = h('button', { class: 'btn small', style: { display: 'none' }, title: T('Sends the source text of this line to the configured service and shows a suggestion. Nothing is written by itself.') }, T('Machine suggestion'));
    mtBtn.onclick = async () => {
      mtBtn.disabled = true; mtBox.replaceChildren(loading(T('Asking the service…')));
      try {
        const r = await post('entry/mt', { file: d.file, line: d.line });
        mtBox.replaceChildren(h('h4', null, T('Machine suggestion ({provider})', { provider: r.provider })), h('div', { class: 'textbox' }, fmtText(r.suggestion)),
          ...r.warnings.map((w) => h('div', { class: 'issue warning' }, chip('warning', T('Careful')), ' ', w)),
          ...r.issues.map((x) => h('div', { class: 'issue ' + x.severity }, chip(x.severity, SEV_LABEL[x.severity]), ' ', x.message)),
          h('div', { class: 'actions' }, h('button', { class: 'btn small primary', onclick: () => { ta.value = r.suggestion; ta.dispatchEvent(new Event('input')); ta.focus(); } }, T('Use the suggestion')),
            h('button', { class: 'btn small', onclick: () => insert(r.suggestion) }, T('Insert')), h('button', { class: 'btn small ghost', onclick: () => mtBox.replaceChildren() }, T('Close'))),
          h('div', { class: 'muted editnote' }, T('A machine suggestion is often literal and lacks the case, gender or tone of the character – always edit it.')));
      } catch (e) { mtBox.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
      mtBtn.disabled = false;
    };
    (async () => { try { if (S.mtOn === undefined) { const st = await get('settings'); S.mtOn = !!(st.mt?.provider && st.mt.consent && (st.mt.has_key || st.mt.base_url)); } mtBtn.style.display = S.mtOn ? '' : 'none'; } catch { /* the button just stays hidden */ } })();
    if (S.ent.focusEdit) { S.ent.focusEdit = false; setTimeout(() => { ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length); }, 0); }
    const node = h('div', { class: 'editor' }, h('h4', null, T('Translation '), h('span', { class: 'muted', style: { textTransform: 'none', letterSpacing: 0 } }, T('– edited directly in the file'))), ta,
      tagBar, missing, terms,
      h('div', { class: 'actions' }, save, reset, mtBtn, menuButton(T('More'), [{ label: T('Original into the field'), hint: T('as a base'), run: () => copySrc.click() }, { label: T('⧉ Copy the translation'), run: () => copyTgt.click() }, { label: T('↶ Undo the last file change'), run: () => undoLast.click() }], { title: T('More actions') })), mtBox,
      h('div', { class: 'muted editnote' }, T('Write a new line as \\n, a quote as \\". Leave the tags {i}…{/i} and [variables] as in the original.')), msg);
    node.ta = ta; node.insert = insert;
    return node;
  }
  /* "Translation memory": the same / a similar original that was already translated. The match is made on the English original,
     so Czech endings do not matter for finding it – but they do for reusing it, hence the reminder. */
  function memoryPanel(d, ta, insert) {
    const box = h('details', { class: 'memory' });
    const sum = h('summary', null, T('Translation memory '), h('span', { class: 'muted' }, T('loading…')));
    box.append(sum);
    const use = (text) => { ta.value = text; ta.dispatchEvent(new Event('input')); ta.focus(); };
    const card = (title, item, extra) => h('div', { class: 'memitem' },
      h('div', { class: 'memhead' }, title, extra || null),
      h('div', { class: 'txt memtext' }, fmtText(item.target)),
      h('div', { class: 'actions' },
        h('button', { class: 'btn small', title: T('Replaces the whole text in the field'), onclick: () => use(item.target) }, T('Apply')),
        h('button', { class: 'btn small', title: T('Inserts the text at the cursor'), onclick: () => insert(item.target) }, T('Insert')),
        item.file ? h('button', { class: 'btn small ghost', onclick: () => go('entries', { file: item.file, line: item.line, status: 'all' }) }, T('Show the line')) : null));
    get('entry/memory', { file: d.file, line: d.line }).then((m) => {
      const n = m.exact.length + m.similar.length;
      sum.replaceChildren(T('Translation memory '), h('span', { class: 'muted' }, n ? `(${n})` : T('(nothing similar)')));
      if (!n) return box.append(h('div', { class: 'muted' }, T('Neither this nor a similar original is translated yet.')));
      box.append(h('div', { class: 'muted', style: { fontSize: '12px', margin: '4px 0' } }, T('Mind the case, gender and number: the text is inserted as it was translated before – adapt it to the new sentence.')));
      m.exact.forEach((g) => box.append(card(h('span', null, chip('proof', T('same original')), ` ×${g.count}`, g.lines.length ? h('span', { class: 'muted' }, `  (${[...new Set(g.lines.map((l) => l.who || '–'))].join(', ')})`) : null), { ...g, file: g.lines[0]?.file, line: g.lines[0]?.line }, g.vars_differ ? chip('warning', T('⚠ different variables')) : null)));
      m.similar.forEach((x) => { const df = diffSpans(d.source || '', x.source); box.append(card(h('span', null, chip('info', T('{n} % similar', { n: Math.round(x.ratio * 100) }))), x, h('span', null, ' ', x.vars_differ ? chip('warning', T('⚠ different variables')) : null, h('div', { class: 'muted memsrc' }, T('Original there: '), df.after)))); });
      if (n && pref('ui.memoryOpen', true)) box.open = true;
    }).catch(() => sum.replaceChildren(T('Translation memory '), h('span', { class: 'muted' }, T('(unavailable)'))));
    box.addEventListener('toggle', () => setPref('ui.memoryOpen', box.open));
    return box;
  }
  /* Every text this line has had (edits in the app, fixes, replacements, undo and changes made elsewhere that were noticed at a refresh).
     Restoring puts the old text into the box; it is only written when you save. */
  function historyPanel(d, ta) {
    const box = h('details', { class: 'texthistory' });
    const sum = h('summary', null, T('Text history '), h('span', { class: 'muted' }, T('loading…')));
    box.append(sum);
    get('entry/history', { file: d.file, line: d.line }).then((r) => {
      sum.replaceChildren(T('Text history '), h('span', { class: 'muted' }, r.count ? '(' + tn(r.count, '{n} change', '{n} changes') + ')' : T('(no recorded change)')));
      if (!r.versions.length) return box.append(h('div', { class: 'muted' }, T('No change has been recorded for this line yet. Every edit in the app is recorded, and also a change made outside it that the app detects when the files are refreshed (F5).')));
      for (const v of r.versions) {
        const df = diffSpans(d.target, v.text);
        box.append(h('div', { class: 'memitem' },
          h('div', { class: 'memhead' }, v.current ? chip('done', T('current')) : null, h('span', { class: 'muted' }, [v.time ? v.time.replace('T', ' ') : '', v.via].filter(Boolean).join(' · '))),
          h('div', { class: 'txt memtext' }, v.current ? fmtText(v.text) : df.after),
          v.current ? null : h('div', { class: 'actions' },
            h('button', { class: 'btn small', title: T('Puts this version into the field. It is written only after saving.'), onclick: () => { ta.value = v.text; ta.dispatchEvent(new Event('input')); ta.focus(); } }, T('Restore into the field')),
            h('button', { class: 'btn small ghost', onclick: () => copy(v.text) }, T('⧉ Copy')))));
      }
    }).catch(() => sum.replaceChildren(T('Text history '), h('span', { class: 'muted' }, T('(unavailable)'))));
    return box;
  }
  /* The line as the player sees it: the picture of the scene (found in the game's archives by following the game script upwards
     to the last `scene` / `show`) with the dialogue box drawn over it, sized by the game's own gui.rpy values. Without the game
     folder it is the box alone. The real font is not used, so wrapping is an estimate. */
  function gamePreview(d, ta) {
    const box = h('details', { class: 'gamepreview', dataset: { keep: '1' } });
    const sum = h('summary', null, T('Scene preview '), h('span', { class: 'muted' }, ''));
    const outer = h('div', { class: 'gwin' }), info = h('div', { class: 'muted ginfo' });
    if (S.gwinH) { outer.style.height = S.gwinH + 'px'; outer.style.width = S.gwinW + 'px'; }     // keep the size of the previous picture until this one is drawn (no jump)
    const mode = h('select', { title: T('Which text to show in the game window'), onchange: () => draw() },
      h('option', { value: 'target' }, T('Translation')), h('option', { value: 'source' }, T('Original')));
    box.append(sum, h('div', { class: 'toolbar', style: { margin: '6px 0' } }, mode), outer, info,
      h('div', { class: 'muted', style: { fontSize: '11.5px', marginTop: '4px' } }, T("The preview is an estimate from the game script, not a screenshot. For games with layered images (characters or backgrounds built from several parts, animations, conditions) it can be wrong or incomplete, and it may not be a good picture of the scene. The font of the text can be a substitute, and the frames of the text box and the name box (their pictures and colours) may look different in the game.")));
    box.open = !!pref('ui.gamepreview', true);
    box.addEventListener('toggle', () => { setPref('ui.gamepreview', box.open); if (box.open) draw(); });
    let gui = S.gui, scene = null, imgOk = true;
    const fallbackName = speakerLabel(d.who) || '';
    async function load() {
      if (!gui) { try { const st = await get('settings'); gui = S.gui = { ...st.game_gui, max_lines: st.validation.textbox.max_lines }; } catch { gui = { text_size: 33, dialogue_width: 1116, textbox_height: 278, max_lines: 4, source: T('default values') }; } }
      if (!scene) scene = await prefetchScene(d.file, d.line);
    }
    const badLayers = new Set();
    const fontsReady = () => [scene.fonts_text, scene.fonts_name].every((f) => !f || !f.url || (S.fontRes && f.file in S.fontRes && S.metRes && f.file in S.metRes));
    function ready() { gui = gui || S.gui; scene = scene || S.sceneDone?.get(d.file + '\t' + d.line); return !!(gui && scene && fontsReady()); }
    /* When the prefetch already has everything, the picture is drawn in the same task as the rest of the detail (no await in between), so the
       view is complete before the browser paints and the scroll position can be kept. */
    async function draw() {
      if (!box.open) return;
      if (!ready()) await load();
      paint();
    }
    function paint() {
      const g = { ...gui, ...(scene.gui || {}) };
      const fam = (scene.fonts_text && S.fontRes?.[scene.fonts_text.file]) || '', nfam = (scene.fonts_name && S.fontRes?.[scene.fonts_name.file]) || '';
      const mText = fam ? S.metRes?.[scene.fonts_text.file] || null : null, mName = nfam ? S.metRes?.[scene.fonts_name.file] || null : null;
      const sp = scene.speaker;                      // the game's own Character(...) name and colour, not the app's speaker names
      const speaker = sp ? sp.name : fallbackName;
      const W = g.screen_width || 1920, H = g.screen_height || 1080;
      const text = mode.value === 'source' ? (d.source ?? '') : ta.value;
      const stage = h('div', { class: 'gstage', style: { width: W + 'px', height: H + 'px', fontSize: g.text_size + 'px', ...(fam ? { fontFamily: `"${fam}", sans-serif` } : {}) } });
      if (scene.flat) stage.style.background = scene.flat;
      else if (scene.url && imgOk) stage.append(h('img', { class: 'gbg', src: scene.url, alt: '', onerror: () => { imgOk = false; draw(); } }));
      for (const l of scene.layers || []) {           // pictures `show`n over the background, at their natural size, standing on the bottom edge
        if (badLayers.has(l.url)) continue;
        stage.append(h('img', { class: 'glay', src: l.url, alt: '', style: { left: l.x * 100 + '%', transform: `translateX(-${l.x * 100}%)` }, onerror: () => { badLayers.add(l.url); draw(); } }));
      }
      const tb = h('div', { class: 'gtextbox', style: { height: g.textbox_height + 'px' } });
      const nm = speaker ? h('div', { class: 'gname', style: { left: g.name_xpos + 'px', top: g.name_ypos + 'px', fontSize: g.name_text_size + 'px', ...(nfam ? { fontFamily: `"${nfam}", sans-serif` } : {}), ...(sp?.color ? { color: sp.color } : {}) } }, speaker) : null;
      const dlg = h('div', { class: 'gbox', style: { left: g.dialogue_xpos + 'px', top: g.dialogue_ypos + 'px', width: g.dialogue_width + 'px' } });
      dlg.append(gameText(text));
      placeChars(dlg, mText, g.text_size);
      if (nm) placeChars(nm, mName, g.name_text_size);
      tb.append(...(nm ? [nm] : []), dlg);
      stage.append(tb);
      outer.replaceChildren(stage);
      const maxH = COMPANION ? (S.compFull ? innerHeight - 250 : innerHeight * 0.55) : film ? innerHeight * 0.52 : innerHeight * 0.45;
      const fit = Math.max(200, maxH) / H;
      const k = box.isConnected && box.clientWidth ? Math.min(1, (box.clientWidth - 2) / W, fit) : (S.gwinK ?? Math.min(1, fit));       // not attached yet: the previous scale
      if (box.isConnected && box.clientWidth) S.gwinK = k;
      stage.style.transform = `scale(${k})`;
      outer.style.height = Math.ceil(H * k) + 'px'; outer.style.width = Math.ceil(W * k) + 'px';
      S.gwinH = Math.ceil(H * k); S.gwinW = Math.ceil(W * k);
      const lines = Math.max(1, Math.round(dlg.offsetHeight / (g.text_size * 1.3)));
      const over = lines > g.max_lines || g.dialogue_ypos + dlg.offsetHeight > g.textbox_height;
      sum.lastChild.textContent = '(≈ ' + tn(lines, '{n} line', '{n} lines') + (over ? ' – ' + T('too much!') : '') + ')';
      outer.classList.toggle('over', over);
      let where;
      if (!scene.available) where = scene.reason || T('The scene picture is not available.');
      else if (scene.flat) where = T('Background: {image} ({script}).', { image: scene.image, script: scene.script });
      else if (scene.url && imgOk) where = T('Picture: {image} (according to {script}) – an estimate from the game script.', { image: scene.image, script: scene.script });
      else if (scene.found && !scene.url && scene.layers?.length) where = T('No background found; only the pictures shown on the screen are drawn (according to {script}) – an estimate from the game script.', { script: scene.script });
      else where = scene.reason || (scene.found ? T('The picture could not be loaded.') : T('The scene picture could not be found.'));
      const fontNote = !scene.available ? '' : fam ? ' ' + T('Game font: {file}.', { file: scene.fonts_text.file }) : T(' The game font could not be loaded, a substitute is shown.');
      const shown = (scene.layers || []).filter((l) => !badLayers.has(l.url));
      const overNote = shown.length ? ' ' + T('Shown over it: {names}.', { names: shown.map((l) => l.image).join(', ') }) : '';
      const layerNote = scene.layered ? ' ' + T('Layered images are not supported yet: characters made of layers are not drawn here. Support is planned.') : '';
      info.textContent = `${where}${overNote}${layerNote}${fontNote} ${T('Window {w}×{h}, font {size} px ({source}); limit {lines}.', { w: g.screen_width, h: g.screen_height, size: g.text_size, source: g.source, lines: tn(g.max_lines, '{n} line', '{n} lines') })}`;
    }
    ta.addEventListener('input', () => { clearTimeout(box._t); box._t = setTimeout(draw, 150); });
    ta.addEventListener('draft', () => draw());
    draw();
    requestAnimationFrame(() => draw());       // again once attached, with the real width
    const onResize = () => { if (!box.isConnected) removeEventListener('resize', onResize); else { clearTimeout(box._r); box._r = setTimeout(draw, 80); } };
    addEventListener('resize', onResize);
    return box;
  }
  async function loadGameMetrics(f) {
    if (!f || !f.file) return null;
    S.metricCache ||= {};
    S.metricCache[f.file] ||= get('game/fontmetrics', { k: f.file }).catch(() => null).then((r) => { (S.metRes ||= {})[f.file] = r; return r; });
    return S.metricCache[f.file];
  }
  /* Every character goes into a box as wide as the font's own advance (hmtx), which is what Ren'Py uses. A browser may take the advance from the
     font's CFF charstring instead, and some patched fonts get a gap after each accented letter that way. Words stay unbreakable, spaces break. */
  function placeChars(el, m, px) {
    if (!m) return;
    const w = (ch) => { const a = m.adv[ch.codePointAt(0)]; return a === undefined ? null : (a * px) / m.upm; };
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT), nodes = [];
    for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n);
    for (const node of nodes) {
      const frag = document.createDocumentFragment();
      for (const part of node.nodeValue.split(/( +)/)) {
        if (!part) continue;
        if (part[0] === ' ') { for (let k = 0; k < part.length; k++) frag.append(h('span', { class: 'gch', style: { width: (w(' ') ?? px * 0.25) + 'px' } }), document.createElement('wbr')); continue; }
        const word = h('span', { class: 'gw' });
        for (const ch of part) { const cw = w(ch); word.append(cw === null ? ch : h('span', { class: 'gch', style: { width: cw + 'px' } }, ch)); }
        frag.append(word);
      }
      node.replaceWith(frag);
    }
  }
  /* Everything the scene preview of one line needs (scene info, the picture decoded, the fonts and their widths), fetched ahead of drawing so
     that choosing a new line swaps the whole view at once instead of showing it piece by piece. Cached per line. */
  const ctxKey = (file, line, radius) => `${file}\t${line}\t${radius}`;
  function contextFor(file, line, radius) {
    S.ctxCache ||= new Map();
    const key = ctxKey(file, line, radius);
    if (!S.ctxCache.has(key)) {
      S.ctxCache.set(key, get('entry/scene', { file, line, radius }).then((r) => { (S.ctxDone ||= new Map()).set(key, r); return r; }));
      if (S.ctxCache.size > 60) S.ctxCache.delete(S.ctxCache.keys().next().value);
    }
    return S.ctxCache.get(key).catch((e) => { S.ctxCache.delete(key); throw e; });
  }
  async function prefetchScene(file, line) {
    S.sceneCache ||= new Map();
    const key = file + '\t' + line;
    if (!S.sceneCache.has(key)) {
      S.sceneCache.set(key, get('game/scene', { file, line }).catch((e) => ({ available: false, reason: e.message })));
      if (S.sceneCache.size > 80) S.sceneCache.delete(S.sceneCache.keys().next().value);
    }
    const sc = await S.sceneCache.get(key);
    const jobs = [loadGameFont(sc.fonts_text), loadGameFont(sc.fonts_name), loadGameMetrics(sc.fonts_text), loadGameMetrics(sc.fonts_name),
      contextFor(file, line, pref('ui.sceneRadius', 6)).catch(() => {})];
    if (pref('entries.layout', 'auto') === 'film') {
      if (!S.filesOld) jobs.push(fileRowsReady().catch(() => {}));
      if (!S.scenesDone?.[file]) jobs.push(scenesFor(file).catch(() => {}));
    }
    if (sc.url) {
      S.imgCache ||= new Map();
      if (!S.imgCache.has(sc.url)) { const im = new Image(); im.src = sc.url; S.imgCache.set(sc.url, im.decode().catch(() => {})); if (S.imgCache.size > 12) S.imgCache.delete(S.imgCache.keys().next().value); }
      jobs.push(S.imgCache.get(sc.url));
    }
    await Promise.all(jobs);
    S.sceneDone ||= new Map();
    S.sceneDone.set(key, sc);
    if (S.sceneDone.size > 80) S.sceneDone.delete(S.sceneDone.keys().next().value);
    return sc;
  }
  /* A font file of the game (found in its folder or archives) made available to the page as a FontFace; its family name, or '' when it cannot be had. */
  async function loadGameFont(f) {
    if (!f || !f.url) return '';
    S.fontCache ||= {};
    S.fontCache[f.file] ||= (async () => {
      try { const face = new FontFace('game-' + f.file, `url("${f.url}")`); await face.load(); document.fonts.add(face); return 'game-' + f.file; } catch { return ''; }
    })().then((r) => { (S.fontRes ||= {})[f.file] = r; return r; });
    return S.fontCache[f.file];
  }
  function scriptContext(d) {
    if (!d.ref) return null;
    const box = h('div', { class: 'scriptctx' });
    const show = async () => {
      box.replaceChildren(loading());
      try {
        const r = await get('entry/context', { file: d.file, line: d.line });
        if (!r.available) return box.replaceChildren(h('div', { class: 'muted' }, r.reason));
        box.replaceChildren(h('div', { class: 'muted mono', style: { fontSize: '12px' } }, T('Game script: '), r.ref),
          h('pre', { class: 'ctxcode' }, r.lines.map((x) => h('div', { class: x.hit ? 'hit' : '' }, `${String(x.n).padStart(5)}  ${x.text}`))));
      } catch (e) { box.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    };
    if (d.unit === 'string') show();      // a choice / UI string has no dialogue around it: show where the game uses it
    else box.append(h('button', { class: 'btn small ghost', onclick: show }, T('Show the context in the game script')));
    return box;
  }
  /* The menu a choice belongs to (read from the game script): the question or the lines said before it, all choices with their translations,
     and the first lines after each choice, because what a choice means is what follows it. Click a line to jump to it. */
  function menuBox(d) {
    const box = h('div', { class: 'menubox' });
    box.append(loading());
    const textOf = (e) => (e.status === 'todo' ? h('span', { class: 'muted' }, T('untranslated')) : fmtText(e.target));
    const sayRow = (x, cls) => x.entry
      ? h('div', { class: 'mline ' + cls, onclick: () => goLine(x.entry.file, x.entry.line) },
        h('div', { class: 'mwho muted' }, speakerLabel(x.entry.who) || '—'), h('div', { class: 'mtxt' }, textOf(x.entry)), h('div', { class: 'msrc muted' }, fmtText(x.entry.source ?? '')))
      : h('div', { class: 'mline ' + cls + ' muted mono' }, x.script);
    get('entry/menu', { file: d.file, line: d.line }).then((r) => {
      if (!r.available) return box.replaceChildren(h('div', { class: 'muted' }, r.reason));
      const parts = [];
      if (r.question.length) parts.push(h('h4', null, T('Question')), ...r.question.map((x) => sayRow(x, 'q')));
      else if (r.before.length) parts.push(h('h4', null, T('Said before the menu')), ...r.before.map((x) => sayRow(x, 'q')));
      parts.push(h('h4', null, T('Choices')));
      for (const o of r.options) {
        parts.push(h('div', { class: 'mopt' + (o.current ? ' cur' : '') },
          h('div', { class: 'mline', onclick: o.entry ? () => goLine(o.entry.file, o.entry.line) : null },
            h('div', { class: 'mwho' }, o.cond ? h('span', { class: 'chip', title: o.cond }, T('if {cond}', { cond: o.cond })) : ''),
            h('div', { class: 'mtxt' }, o.entry ? textOf(o.entry) : h('span', { class: 'muted' }, T('(not found in the translation)'))),
            h('div', { class: 'msrc muted' }, fmtText(o.text))),
          ...o.after.map((x) => sayRow(x, 'after'))));
      }
      box.replaceChildren(h('div', { class: 'muted mono', style: { fontSize: '12px' } }, T('Game script: '), r.script), ...parts);
    }).catch((e) => box.replaceChildren(h('div', { class: 'notice bad' }, e.message)));
    return box;
  }
  /* The scene around the line: original next to translation, the current line marked; click a row to jump to it. */
  function sceneBox(d) {
    const box = h('details', { class: 'scene', dataset: { keep: '1' } });
    box.open = !!pref('ui.sceneOpen', true);
    const sum = h('summary', null, T('Scene around the line '), h('span', { class: 'muted' }, T('(original next to the translation)')));
    const body = h('div');
    const radius = h('select', { title: T('How many surrounding lines to show'), onchange: (ev) => { setPref('ui.sceneRadius', +ev.target.value); fill(); } },
      [3, 6, 10, 15].map((n) => h('option', { value: n, selected: n === pref('ui.sceneRadius', 6) }, T('±{n} lines', { n }))));
    const modeSeg = h('div', { class: 'seg' });
    const paintSeg = () => modeSeg.replaceChildren(...[['table', T('Table')], ['bubbles', T('Bubbles')]].map(([k, l]) => h('button', { class: pref('ui.sceneMode', 'table') === k ? 'on' : '', onclick: () => { setPref('ui.sceneMode', k); paintSeg(); fill(); } }, l)));
    paintSeg();
    const bubbles = (r) => h('div', { class: 'bubbles' }, r.items.map((it) => h('div', { class: 'bubble' + (it.current ? ' cur' : ''), style: { '--sc': speakerColor(it.who) }, onclick: () => goLine(d.file, it.line) },
      h('div', { class: 'bwho' }, speakerBadge(it.who), h('b', null, speakerName(it.who) || S.chars?.[(it.who || '').trim().split(/\s+/)[0]]?.name || it.who || '—'), h('span', { class: 'muted' }, ' ' + it.line)),
      h('div', { class: 'btxt' }, it.status === 'todo' ? h('span', { class: 'muted' }, T('untranslated')) : fmtText(it.target)),
      h('div', { class: 'bsrc muted' }, fmtText(it.source ?? '')))));
    const mtScene = h('button', { class: 'btn small', title: T('Translates the untranslated lines around this one (±15) by machine. You review the suggestions before writing.'), onclick: () => mtDialog({ picked: [], anchor: { file: d.file, line: d.line }, file: d.file, scope: 'scene' }, async () => { S.files = null; await refreshHeader(); select(S.ent.sel); }) }, T('Translate the scene with a machine…'));
    box.append(sum, h('div', { class: 'toolbar', style: { margin: '6px 0' } }, radius, modeSeg, mtScene), body);
    async function fill() {
      if (!box.open) return;
      const peek = S.ctxDone?.get(ctxKey(d.file, d.line, pref('ui.sceneRadius', 6)));
      if (!peek) body.replaceChildren(loading());
      try {
        const r = peek || await contextFor(d.file, d.line, pref('ui.sceneRadius', 6));
        if (pref('ui.sceneMode', 'table') === 'bubbles') return body.replaceChildren(bubbles(r));
        const rows = r.items.map((it) => h('tr', { class: it.current ? 'cur' : '', onclick: () => { const i = S.ent.items.findIndex((x) => x.file === d.file && x.line === it.line); if (i >= 0) select(i); else go('entries', { file: d.file, line: it.line, status: 'all' }); } },
          h('td', { class: 'num muted' }, it.line), h('td', { class: 'who' }, speakerLabel(it.who) || ''), h('td', { class: 'txt' }, fmtText(it.source ?? '')),
          h('td', { class: 'txt' }, it.status === 'todo' ? h('span', { class: 'muted' }, T('untranslated')) : fmtText(it.target))));
        body.replaceChildren(h('div', { class: 'tablewrap' }, h('table', { class: 'data scene-table' }, h('colgroup', null, [60, 100, 'auto', 'auto'].map((w) => h('col', { style: w === 'auto' ? {} : { width: w + 'px' } }))),
          h('thead', null, h('tr', null, [T('Line'), T('Speaker'), T('Original'), T('Translation')].map((x) => h('th', null, x)))), h('tbody', null, rows))));
      } catch (e) { body.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    }
    box.addEventListener('toggle', () => { setPref('ui.sceneOpen', box.open); fill(); });
    fill();
    return box;
  }
  const ctxLine = (x) => h('div', { class: 'ctx' }, `${x.line}  `, x.who ? h('span', { class: 'mono' }, speakerLabel(x.who) + ' ') : null, fmtText(x.target));

  async function toggleApprove() {
    const d = S.ent.detail; if (!d) return;
    const on = d.status !== 'approved';
    try {
      await post('approve/apply', { selections: [{ file: d.file, key: d.key }], on });
      toast(on ? T('Line approved. Undo: Ctrl+Z') : T('Approval removed.'));
      const it = S.ent.items[S.ent.sel];
      it.status = on ? 'approved' : 'todo';
      tbody.replaceChild(rowFor(it, S.ent.sel), tbody.children[S.ent.sel]);
      S.files = null; await refreshHeader();
      if (on && pref('entries.status', 'todo') === 'todo') select(Math.min(S.ent.sel + 1, S.ent.items.length - 1));
      else select(S.ent.sel);
    } catch (err) { fail(err); }
  }
  function repaintRows() {
    if (!tbody) return;
    tbody.replaceChildren(...S.ent.items.map((it, i) => rowFor(it, i)));
    if (S.ent.detail) drawDetail(S.ent.detail);
  }
  function refreshRow(i) { tbody.replaceChild(rowFor(S.ent.items[i], i), tbody.children[i]); }
  async function setProof(state, note, d = S.ent.detail, idx = S.ent.sel) {
    if (!d || d.status === 'todo') return false;
    try {
      const r = await post('proof/set', { items: [{ file: d.file, key: d.key }], state, note });
      if (r.skipped.length) { toast(r.skipped[0][1], 'bad'); return false; }
      if (S.ent.items[idx]) { S.ent.items[idx].proof = state; refreshRow(idx); }
      bumpLocal(d.file, d.line, 0, (state === 'ok' ? 1 : 0) - (d.proof === 'ok' ? 1 : 0)); lazyRefresh();
      return true;
    } catch (err) { fail(err); return false; }
  }
  function nextToReview(from) {
    for (let k = from + 1; k < S.ent.items.length; k++) { const x = S.ent.items[k]; if (x.status !== 'todo' && !['ok', 'fix'].includes(x.proof)) return k; }
    return Math.min(from + 1, S.ent.items.length - 1);
  }
  async function toggleProof() {
    const d = S.ent.detail; if (!d) return;
    const on = d.proof !== 'ok';
    if (on && d.status !== 'todo') {                        // go on to the next line at once; the write finishes behind it
      const cur = S.ent.sel, before = d.proof;
      S.ent.items[cur].proof = 'ok'; refreshRow(cur);
      await select(nextToReview(cur));                       // the next line is shown first, then the mark is written (the server handles one request at a time)
      if (await setProof('ok', undefined, d, cur)) { toast(T('Proofread ✓  (Ctrl+Z undoes)'), 'ok', 1800); refreshStrips(); }
      else if (S.ent.items[cur]) { S.ent.items[cur].proof = before; refreshRow(cur); }
      return;
    }
    if (await setProof(on ? 'ok' : '')) {
      toast(on ? T('Proofread ✓  (Ctrl+Z undoes)') : T('Mark removed.'), 'ok', 1800);
      select(on ? nextToReview(S.ent.sel) : S.ent.sel);
    }
  }
  async function flagFix() {
    const d = S.ent.detail; if (!d) return;
    const on = d.proof !== 'fix';
    if (await setProof(on ? 'fix' : '')) {
      setPref('ui.detailTab', 'proof');
      await select(S.ent.sel);
      if (on) { const n = $('#proofNote'); n?.focus(); toast(T('Flagged for a fix – write a note.'), 'ok', 2500); }
    }
  }
  async function saveNote(text) {
    const d = S.ent.detail; if (!d) return;
    const state = d.proof === 'fix' ? 'fix' : d.proof === 'ok' ? 'ok' : (text.trim() ? 'fix' : '');
    if (!state) return;
    if (await setProof(state, text)) { toast(T('Note saved ✓'), 'ok', 1500); await select(S.ent.sel); }
  }
  async function proofUpto(state = 'ok') {
    const d = S.ent.detail; if (!d) return;
    const undo = state === '';
    if (undo && !await confirmDialog(T('Remove the proofread mark up to here?'), T('In all lines of the file {file} from the start to line {line} the “proofread” mark is removed. Lines marked “to fix” stay. It can be undone with Ctrl+Z.', { file: d.file.split(/[\\/]/).pop(), line: d.line }), T('Remove the proofread mark'))) return;
    try {
      const r = await post('proof/upto', { file: d.file, line: d.line, state });
      S.ent.items.forEach((x, i) => {
        if (x.file !== d.file || x.line > d.line || x.status === 'todo') return;
        if (!undo && ['', 'stale'].includes(x.proof)) { x.proof = 'ok'; refreshRow(i); }
        if (undo && ['ok', 'stale'].includes(x.proof)) { x.proof = ''; refreshRow(i); }
      });
      S.files = null; await refreshHeader(); await select(S.ent.sel);
      toast(r.changed ? (undo ? tn(r.changed, 'Proofread mark removed on {n} line  (Ctrl+Z undoes)', 'Proofread mark removed on {n} lines  (Ctrl+Z undoes)') : tn(r.changed, 'Proofread {n} line ✓  (Ctrl+Z undoes)', 'Proofread {n} lines ✓  (Ctrl+Z undoes)')) : (undo ? T('There was nothing to remove.') : T('There was nothing to mark.')), 'ok', 3000);
    } catch (err) { fail(err); }
  }
  async function globalApprove() {
    const d = S.ent.detail; if (!d || d.status === 'translated') return;
    await globalApproveDialog(d, async () => { S.files = null; await refreshHeader(); load(true); });
  }
  S.ent.api = { select, toggleApprove, toggleProof, flagFix, proofUpto, unproofUpto: () => proofUpto(''), globalApprove, load, tbody: () => tbody, reload: () => load(true), follow: followLine, redraw: () => drawDetail(S.ent.detail) };
  root.afterMount = () => {
    if (!COMPANION) { load(true); drawDetail(null); return; }
    skeleton();
    const f = S.ent.pendingLine ? { file: pref('entries.file', ''), line: S.ent.pendingLine } : S.lastFocus;
    if (f?.file && f.line) followLine(f.file, f.line); else drawDetail(null);
  };
  return root;
}
async function globalApproveDialog(d, done) {
  let scope = 'file', sameSpeaker = true, data = null, armed = false;
  const body = h('div');
  const foot = h('button', { class: 'btn primary', disabled: true });
  const info = h('span', { class: 'grow' });
  const sel = new Set();
  async function loadPreview() {
    body.replaceChildren(loading());
    try { data = await post('approve/preview', { file: d.file, line: d.line, scope, same_speaker: sameSpeaker }); }
    catch (e) { body.replaceChildren(h('div', { class: 'notice bad' }, e.message)); return; }
    sel.clear(); data.items.forEach((it, i) => { if (it.selected) sel.add(i); });
    armed = false; paint();
  }
  function paint() {
    const files = new Set([...sel].map((i) => data.items[i].file));
    const segBtn = (k, l) => h('button', { class: scope === k ? 'on' : '', onclick: () => { scope = k; loadPreview(); } }, l);
    const rows = data.items.map((it, i) => h('tr', null,
      h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(i), disabled: !it.selectable, onchange: (e) => { e.target.checked ? sel.add(i) : sel.delete(i); armed = false; paint2(); } })),
      h('td', null, fname(it.file)), h('td', { class: 'num' }, it.line), h('td', { class: 'who' }, speakerLabel(it.who) || '—'),
      h('td', null, fmtText(it.text)), h('td', null, it.is_reference ? chip('rev', T('selected line')) : null, it.approved ? chip('approved', T('approved')) : null, h('span', { class: 'muted' }, it.notes.join('; ')))));
    const t = h('table', { class: 'data' }, h('colgroup', null, [40, 420, 70, 90, 260, 300].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', T('File'), T('Line'), T('Speaker'), T('Text'), T('Note')].map((x, i) => h('th', { class: i === 0 ? 'center' : '' }, x)))), h('tbody', null, rows));
    t.style.width = '1240px';
    body.replaceChildren(
      h('p', { class: 'muted' }, T('Only lines whose original and translation are the same as those of the selected line are approved. Lines with another speaker or looking like untranslated English are unticked by default. Every approved line is written separately (by block), so it can be undone with Ctrl+Z.')),
      h('div', { class: 'toolbar' }, h('div', { class: 'seg' }, segBtn('occurrence', T('This line only')), segBtn('file', T('This whole file')), segBtn('project', T('The whole project'))),
        h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: sameSpeaker, onchange: (e) => { sameSpeaker = e.target.checked; loadPreview(); } }), T('Preselect the same speaker only'))),
      h('div', { class: 'ctx' }, T('Selected text: '), h('b', null, fmtText(data.reference.text))),
      h('div', { class: 'tablewrap', style: { maxHeight: '48vh', marginTop: '8px' } }, t));
    paint2(files);
  }
  function paint2(files) {
    files = files || new Set([...sel].map((i) => data.items[i].file));
    const n = sel.size;
    const broad = n > 1;
    foot.disabled = !n;
    foot.textContent = armed ? T('Confirm: approve {n} lines in {f} files', { n, f: files.size }) : T('Approve selected ({n})', { n });
    foot.classList.toggle('danger', armed);
    info.textContent = tn(files.size, 'Found {total} identical · selected {n} in {f} file', 'Found {total} identical · selected {n} in {f} files', { total: data.counts.total, n, f: files.size }) + (broad ? T(' · a bulk change must be confirmed twice') : '');
    foot.onclick = async () => {
      if (broad && !armed) { armed = true; paint2(files); setTimeout(() => { armed = false; paint2(); }, 8000); return false; }
      try {
        const res = await post('approve/apply', { selections: [...sel].map((i) => ({ file: data.items[i].file, key: data.items[i].key })), on: true });
        toast(tn(res.changed, 'Approved {n} line', 'Approved {n} lines') + (res.skipped.length ? ', ' + T('skipped {n}', { n: res.skipped.length }) : '') + T('. Undo: Ctrl+Z'), res.skipped.length ? 'bad' : 'ok');
        close(); done?.();
      } catch (e) { fail(e); }
      return false;
    };
  }
  S.dlgDirty = true;
  const close = dialog({ title: T('Approve identical lines'), body, buttons: [{ label: T('Cancel') }, { node: h('span', { style: { display: 'contents' } }, info, foot) }] });
  loadPreview();
}

