<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { get, post } from '../api.js';
import { register } from '../keys.js';
import { ui } from '../layout.js';
import LayoutMenu from '../components/LayoutMenu.vue';
import ResultsBar from '../components/ResultsBar.vue';
import { bumpScene, forgetFileData, menuOf, mtOn, peekMt, prefetchAll, prefetchStats, sceneRadius, scenesOf } from '../gamewindow.js';
import { requireFeature } from '../pro.js';
import LineDetail from '../components/LineDetail.vue';
import MtDialog from '../components/MtDialog.vue';
import { t, tn } from '../i18n.js';
import { baseName, fmtN, lineKind, lineState, loadFiles, loadNames, loadOverview, pillOf, project } from '../project.js';

/* Lines: every line of the translation with all its columns (file, line, script, type, state, proofreading, code, name, original, translation, check);
   the line you click opens on the right, where it is translated, approved and marked as proofread. */
const emit = defineEmits(['go', 'quick']);
const error = ref('');
const items = ref([]);
const total = ref(0);
const counts = ref(null);
const offset = ref(0);
const loading = ref(false);
const sel = ref(-1);
const detail = ref(null);
const asideEl = ref(null);
const picked = ref(new Set());
const moreOpen = ref(false);
const ld = ref(null);
const f = ref({ file: project.lines.file || '', status: project.lines.status || 'todo', q: project.lines.q || '', word: false, unit: '', kind: '', issues: '', sort: 'line' });
const speaker = ref('');
const mode = ref(storedMode());      // both | code | name: how the speaker is shown
let around = project.lines.line || 0;
let timer = null;

function storedMode() { try { return localStorage.getItem('aside.speaker') || 'both'; } catch { return 'both'; } }
watch(mode, (m) => { try { localStorage.setItem('aside.speaker', m); } catch { /* only remembered while the page lives */ } });

const files = computed(() => project.files || []);
const showFile = computed(() => !f.value.file);
const STATUS_OPTS = computed(() => [['all', t('All')], ['todo', t('To translate')], ['translated', t('Translated ones')], ['approved', t('Approved ones')], ['review', t('For proofreading')], ['fix', t('To fix')], ['stale', t('Changed after proofreading')], ['proofed', t('Proofread ones')]]);
const OPTS = computed(() => ({
  unit: [['', t('Dialogues and choices')], ['dialogue', t('Dialogues only')], ['string', t('Choices only (strings)')]],
  kind: [['', t('All kinds')], ['english', t('In English')], ['same', t('Identical, maybe OK')], ['symbols', t('Symbols only')], ['empty', t('Empty')]],
  issues: [['', t('All')], ['any', t('With a problem')], ['error', t('With an error')], ['warning', t('With a warning')], ['overflow', t('Does not fit the game window')]],
  sort: [['line', t('Line')], ['file', t('File')], ['kind', t('Kind')], ['issues', t('Problems')]],
  mode: [['both', t('Code and name')], ['code', t('Code only')], ['name', t('Name only')]],
}));
const extra = computed(() => ['unit', 'kind', 'issues'].filter((k) => f.value[k]).length);
const speakers = computed(() => Object.entries(project.names).sort((a, b) => a[1].localeCompare(b[1])));
const narrowed = computed(() => !!(f.value.q || f.value.file || f.value.unit || f.value.kind || f.value.issues));
const summary = computed(() => [f.value.q ? t('Search: “{q}”', { q: f.value.q }) : '', f.value.file ? t('File: {name}', { name: baseName(f.value.file) }) : ''].filter(Boolean).join(' · '));
const chips = computed(() => {
  const out = [];
  const lab = (list, v) => (list.find(([k]) => k === v) || [v, v])[1];
  if (f.value.file) out.push([t('File: ') + baseName(f.value.file), () => { f.value.file = ''; }]);
  if (f.value.status !== 'all') out.push([t('State: ') + lab(STATUS_OPTS.value, f.value.status), () => { f.value.status = 'all'; }]);
  for (const [k, name] of [['unit', t('Type')], ['kind', t('Kind')], ['issues', t('Check')]]) if (f.value[k]) out.push([`${name}: ${lab(OPTS.value[k], f.value[k])}`, () => { f.value[k] = ''; }]);
  if (f.value.q) out.push([t('Search: “{q}”', { q: f.value.q }) + (f.value.word ? ' · ' + t('Whole word') : ''), () => { f.value.q = ''; speaker.value = ''; }]);
  return out;
});
function reset() { f.value = { ...f.value, file: '', status: 'all', q: '', unit: '', kind: '', issues: '' }; speaker.value = ''; }
function pickSpeaker() { if (speaker.value) { f.value.q = speaker.value; f.value.word = true; } else if (speakers.value.some(([c]) => c === f.value.q)) f.value.q = ''; }

async function load(resetList) {
  if (loading.value) return;
  loading.value = true; error.value = '';
  try {
    if (resetList) { items.value = []; sel.value = -1; detail.value = null; offset.value = 0; picked.value = new Set(); }
    const query = { ...f.value, word: f.value.word ? '1' : '', limit: 200, offset: resetList ? 0 : offset.value + items.value.length, around: resetList ? around : 0 };
    if (resetList && around) query.status = 'all';
    const r = await get('entries', query);
    if (resetList) offset.value = r.offset;
    items.value.push(...r.items);
    total.value = r.total; counts.value = r.counts;
    ui.left = { todo: r.counts?.todo, review: r.counts?.review };
    if (resetList && !around && ui.layout === 'film' && items.value.length) select(0);          // a list that is not shown has nothing to click: the first line opens
    if (resetList && around) { const i = items.value.findIndex((x) => x.line === around); around = 0; if (i >= 0) await select(i); }
  } catch (e) { error.value = e.message; } finally { loading.value = false; }
}
watch(() => project.lines, (p) => { around = p.line || 0; f.value = { ...f.value, file: p.file || '', status: p.status || 'all', q: p.q || '' }; });
function change() { clearTimeout(timer); timer = setTimeout(() => load(true), 220); }
watch(f, change, { deep: true });

/* Is the Scene tab the one shown? (the last tab chosen is remembered; with none chosen the detail opens on the scene when the line has no findings) */
function wantsScene() { try { const k = localStorage.getItem('aside.detailTab') || ''; return k === 'scene' || k === ''; } catch { return true; } }
/* Choosing a line. Moving fast (a key held down, many clicks) must feel at least as quick as the classic list, so:
   - only the last choice counts: an answer for a line that is no longer the chosen one is thrown away (the row is highlighted at once, the detail follows the last row);
   - the line itself is fetched first (a few milliseconds); the heavier parts (the picture of the scene, the scenes of the file, the menu) are asked for only once the line has arrived and is still
     the chosen one, so a run of presses does not flood the server with requests for lines that are skipped;
   - the detail waits for those parts a quarter of a second when you stop, but only a moment while you are moving fast;
   - the position is told to the other window after a short pause, not for every line that is passed. */
let selSeq = 0, lastSelAt = 0, focusT = null;
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
async function select(i) {
  if (i < 0 || i >= items.value.length) return;
  if (ld.value && ld.value.dirty && !window.confirm(t('You changed the text of this line and did not save it. Discard the changes?'))) return;
  sel.value = i;
  const my = ++selSeq, at = performance.now(), fast = at - lastSelAt < 350;
  lastSelAt = at;
  const it = items.value[i];
  nextTick(keepRowVisible);
  try {
    const scroller = scrollerOf(); const top = scroller ? scroller.scrollTop : 0;      // the panel keeps its place when the next line opens (the new line has its tabs still loading, so the content is short for a moment)
    const d = await get('entry', { file: it.file, line: it.line });
    if (my !== selSeq) return;
    const ahead = [scenesOf(it.file)];                                                  // the strip of scenes and the Menu tab are there with the line
    if (d.unit !== 'string' && d.ref) ahead.push(menuOf(it.file, it.line));
    if (peekMt() === null) ahead.push(mtOn());
    if (wantsScene()) ahead.push(prefetchAll(it.file, it.line, sceneRadius()).catch(() => {}));
    // A line nobody opened yet can take seconds on the server (the first scene of a file, the index of the pictures): the detail waits a moment for what is quick, and opens without the rest.
    await Promise.race([Promise.all(ahead).catch(() => {}), pause(fast ? 40 : 250)]);
    if (my !== selSeq) return;
    detail.value = d;
    await nextTick();
    holdScroll(scroller, top);
    if (ui.quick === 'tr') ld.value?.focusEditor();             // quick translating: the editor has the focus
    try { localStorage.setItem('aside.last', JSON.stringify({ file: it.file, line: it.line })); } catch { /* only for the Continue button */ }
    clearTimeout(focusT);
    focusT = setTimeout(() => { post('focus', { file: it.file, line: it.line }).catch(() => {}); }, 150);
    const nx = items.value[i + 1];
    if (nx && wantsScene() && prefetchStats.ms < 400) setTimeout(() => { if (sel.value === i) prefetchAll(nx.file, nx.line, sceneRadius()).catch(() => {}); }, 400);      // the next line is ready when you get there
    if (i >= items.value.length - 15 && offset.value + items.value.length < total.value) load(false);
  } catch (e) { if (my === selSeq) { error.value = e.message; detail.value = null; } }
}
/* After a new line replaced the old one: put the panel back where it was, and keep doing so while the content grows (the tabs fetch their data), until the user scrolls. */
let holdOff = null;
/* What scrolls: the panel itself when it has its own scroll bar (wide window), otherwise the page (narrow window, where the panel sits under the list). */
function scrollerOf() { return asideEl.value; }
function holdScroll(el, top) {
  if (holdOff) holdOff();
  if (!el || !top) return;
  el.scrollTop = top;
  const inner = el === document.scrollingElement ? document.body : el.firstElementChild;
  if (!inner || typeof ResizeObserver === "undefined") return;
  const stop = () => { clearTimeout(tm); ro.disconnect(); for (const ev of ["wheel", "touchstart", "keydown"]) el.removeEventListener(ev, stop); holdOff = null; };
  const ro = new ResizeObserver(() => { if (el.scrollTop < top) el.scrollTop = top; });
  const tm = setTimeout(stop, 1500);
  ro.observe(inner);
  for (const ev of ["wheel", "touchstart", "keydown"]) el.addEventListener(ev, stop);
  holdOff = stop;
}
onBeforeUnmount(() => { if (holdOff) holdOff(); });
/* The selected row stays in view inside the list: only the list scrolls, never the page. */
function keepRowVisible() {
  const row = document.querySelector('.ln.sel'), box = row && row.closest('.tablewrap');
  if (!row || !box) return;
  const head = box.querySelector('thead')?.offsetHeight || 0;
  if (row.offsetTop - head < box.scrollTop) box.scrollTop = row.offsetTop - head;
  else if (row.offsetTop + row.offsetHeight > box.scrollTop + box.clientHeight) box.scrollTop = row.offsetTop + row.offsetHeight - box.clientHeight;
}
function mirror() {
  const d = detail.value, it = items.value[sel.value];
  if (!d || !it) return;
  Object.assign(it, { target: d.target, status: d.status, proof: d.proof, kind: d.kind, issues: (d.issues || []).filter((x) => x.severity !== 'info').map((x) => ({ severity: x.severity, code: x.code })) });
}
async function goto({ file, line }) {
  const i = items.value.findIndex((x) => x.file === file && x.line === line);
  if (i >= 0) { await select(i); return; }
  if (ld.value && ld.value.dirty && !window.confirm(t('You changed the text of this line and did not save it. Discard the changes?'))) return;
  around = line; f.value = { ...f.value, file, status: 'all', q: '' };
}
const nextWhere = (from, pred) => { for (let k = from + 1; k < items.value.length; k++) if (pred(items.value[k])) return k; return -1; };
/* Approve and proofread: the row and the counters change at once, the list goes on to the next line (it is already prefetched), and only then is the mark written, because the server
   does one thing at a time and the next line should not wait for a write. A failed write puts everything back. */
function bumpFile(file, line, dDone, dProof) {
  const row = (project.files || []).find((x) => x.name === file);
  if (row) { row.done = Math.max(0, row.done + dDone); row.todo = Math.max(0, row.todo - dDone); row.corr = Math.max(0, row.corr + dProof); }
  bumpScene(file, line, dDone, dProof);
}
async function doApprove() {
  const d = detail.value, i = sel.value, it = items.value[i];
  if (!d || !it || d.status === 'translated' || d.kind === 'empty') return;
  const on = d.status !== 'approved', before = it.status;
  it.status = on ? 'approved' : 'todo'; bumpFile(d.file, d.line, on ? 1 : -1, 0);
  let k = i;
  if (on) { k = nextWhere(i, (x) => x.status === 'todo'); if (k < 0) k = Math.min(i + 1, items.value.length - 1); }
  if (k !== i) await select(k);
  try {
    await post('approve/apply', { selections: [{ file: d.file, key: d.key }], on });
    if (k === i) { detail.value = await get('entry', { file: d.file, line: d.line }); mirror(); }
    lazyNumbers();
  } catch (e) { it.status = before; bumpFile(d.file, d.line, on ? -1 : 1, 0); error.value = e.message; }
}
async function doProof({ state }) {
  const d = detail.value, i = sel.value, it = items.value[i];
  if (!d || !it || d.status === 'todo') return;
  const before = it.proof, dProof = (state === 'ok' ? 1 : 0) - (before === 'ok' ? 1 : 0);
  it.proof = state; bumpFile(d.file, d.line, 0, dProof);
  let k = i;
  if (state === 'ok') { k = nextWhere(i, (x) => x.status !== 'todo' && !['ok', 'fix'].includes(x.proof)); if (k < 0) k = Math.min(i + 1, items.value.length - 1); }
  if (k !== i) await select(k);
  try {
    const r = await post('proof/set', { items: [{ file: d.file, key: d.key }], state, note: '' });
    if (r.skipped && r.skipped.length) throw new Error(r.skipped[0][1]);
    if (k === i) { detail.value = await get('entry', { file: d.file, line: d.line }); mirror(); }
    lazyNumbers();
  } catch (e) { it.proof = before; bumpFile(d.file, d.line, 0, -dProof); error.value = e.message; }
}
async function updated(ev = {}) {
  error.value = '';
  try {
    const i = sel.value, d = detail.value;
    if (ev.reload) { forgetFileData(); await load(true); lazyNumbers(); return; }
    detail.value = await get('entry', { file: d.file, line: d.line });
    mirror();
    forgetFileData(); lazyNumbers();
    if (ev.advance) {
      let k;
      if (ev.kind === 'proof') k = nextWhere(i, (x) => x.status !== 'todo' && !['ok', 'fix'].includes(x.proof));
      else k = nextWhere(i, (x) => x.status === 'todo');
      if (k < 0) k = Math.min(i + 1, items.value.length - 1);
      if (k !== i) { await select(k); if (ev.kind !== 'proof' && ev.kind !== 'approve') ld.value?.focusEditor(); }
    }
  } catch (e) { error.value = e.message; }
}
function step(delta) { select(sel.value + delta); }
function rowClick(ev, i, it) {
  const k = it.file + '\t' + it.line;
  if (ev.ctrlKey || ev.metaKey) { const s = new Set(picked.value); if (s.has(k)) s.delete(k); else s.add(k); picked.value = s; return; }
  if (ev.shiftKey && sel.value >= 0) { const [a, b] = [Math.min(sel.value, i), Math.max(sel.value, i)]; const s = new Set(picked.value); for (let j = a; j <= b; j++) s.add(items.value[j].file + '\t' + items.value[j].line); picked.value = s; return; }
  if (picked.value.size) picked.value = new Set();
  select(i);
}
const pickedList = computed(() => [...picked.value].map((k) => { const [file, line] = k.split('\t'); return { file, line: +line }; }));
const isPicked = (it) => picked.value.has(it.file + '\t' + it.line);

/* the keys of the Lines screen are the actions of keys.js (they can be changed in Settings); this screen says what each one does */
const last = () => items.value.length - 1;
const keyHandlers = {
  next: () => select(sel.value + 1), prev: () => select(sel.value - 1),
  pagedown: () => select(Math.min(last(), sel.value + 15)), pageup: () => select(Math.max(0, sel.value - 15)), first: () => select(0), last: () => select(last()),
  approve: () => ld.value?.approve(), approve_all: () => ld.value?.openIdentical(), proof: () => ld.value?.toggleProof(), proof_upto: () => ld.value?.proofUpto('ok'),
  unproof_upto: () => ld.value?.proofUpto(''), flag: () => ld.value?.toggleFix(), open_editor: () => ld.value?.openEditor(),
};
let offKeys = null;
const reloadList = () => { forgetFileData(); load(true); };
const who = (it) => (it.who || '').trim().split(/\s+/)[0];
const full = (it) => project.names[who(it)] || '';
const shown = (s) => String(s ?? '').replace(/\n/g, ' ↵ ');
/* The tools have their own screen; what stays here works on the lines you picked: translating them with a machine. */
const mtOpen = ref(false);
async function openMt() { if (await requireFeature('mt.batch')) mtOpen.value = true; }
/* The numbers of the overview and of the files are not on this screen, and fetching them takes the server for a moment (the next line would wait for it), so they are fetched when you stop for
   half a minute and when you leave the screen. The counters this screen shows are moved by hand meanwhile (bumpFile). */
let lazyT = null, stale = false;
function refreshNumbers() { clearTimeout(lazyT); stale = false; loadOverview().catch(() => {}); loadFiles().catch(() => {}); }
function lazyNumbers() { stale = true; clearTimeout(lazyT); lazyT = setTimeout(refreshNumbers, 30000); }
const after = async () => { forgetFileData(); await load(true); lazyNumbers(); };

onMounted(async () => {
  offKeys = register(keyHandlers);
  window.addEventListener('aside:reload', reloadList);
  loadNames();
  try { if (!project.files) await loadFiles(); } catch (e) { error.value = e.message; }
  load(true);
});
onBeforeUnmount(() => { clearTimeout(focusT); if (stale) refreshNumbers(); if (offKeys) offKeys(); window.removeEventListener('aside:reload', reloadList); clearTimeout(timer); });
</script>

<template>
  <main class="screen" :class="{ focus: ui.focus }">
    <div v-show="!ui.focus" class="toolbar">
      <label class="field search"><input v-model="f.q" type="search" :placeholder="t('Search in the text… (Ctrl+F)')" @input="speaker = ''"></label>
      <label class="check" :title="t('Whole word')"><input type="checkbox" v-model="f.word"> {{ t('Whole word') }}</label>
      <select class="sel wide" v-model="f.file" :title="t('File')"><option value="">{{ t('All files') }}</option><option v-for="x in files" :key="x.name" :value="x.name">{{ x.name }}</option></select>
      <select class="sel" v-model="speaker" :title="t('Speaker')" :aria-label="t('Speaker')" @change="pickSpeaker"><option value="">{{ t('Speaker: all') }}</option><option v-for="[code, name] in speakers" :key="code" :value="code">{{ name }} · {{ code }}</option></select>
      <button class="btn" :aria-expanded="moreOpen" :title="t('Line type, kind, check, sorting, speaker display')" @click="moreOpen = !moreOpen">{{ moreOpen ? '▾' : '▸' }} {{ t('More filters') }}<template v-if="extra"> ({{ extra }})</template></button>
      <template v-if="picked.size"><span class="muted small">{{ tn(picked.size, 'Selected {n} line', 'Selected {n} lines') }}</span><button class="btn small ghost" @click="picked = new Set()">{{ t('Clear the selection') }}</button><button class="btn small" @click="openMt">{{ t('Translate with a machine…') }}</button></template>
      <span class="grow"></span>
      <span class="muted small">{{ tn(total, '{n} line', '{n} lines', { n: fmtN(total) }) }}</span>
      <LayoutMenu @quick="$emit('quick', 'tr')" />
    </div>
    <div v-show="!ui.focus" class="pills" role="group" :aria-label="t('State of the lines')">
      <button v-for="[k, l] in STATUS_OPTS" :key="k" class="chip" :class="{ on: f.status === k }" @click="f.status = k">{{ l }}<span v-if="counts && counts[k] !== undefined" class="faint"> {{ fmtN(counts[k]) }}</span></button>
    </div>
    <div v-if="moreOpen && !ui.focus" class="toolbar more">
      <label>{{ t('Line type') }} <select class="sel" v-model="f.unit"><option v-for="[v, l] in OPTS.unit" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Kind of untranslated') }} <select class="sel" v-model="f.kind"><option v-for="[v, l] in OPTS.kind" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Check') }} <select class="sel" v-model="f.issues"><option v-for="[v, l] in OPTS.issues" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Sort by') }} <select class="sel" v-model="f.sort"><option v-for="[v, l] in OPTS.sort" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Speaker') }} <select class="sel" v-model="mode"><option v-for="[v, l] in OPTS.mode" :key="v" :value="v">{{ l }}</option></select></label>
    </div>
    <div v-if="chips.length && !ui.focus" class="toolbar chips"><button v-for="([label, off], i) in chips" :key="i" class="chip on" :title="t('Remove this filter')" @click="off">{{ label }} ✕</button><button class="btn small ghost" @click="reset">{{ t('Clear all filters') }}</button></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>

    <div class="split" :class="ui.layout">
      <div class="tablewrap">
        <table class="grid">
          <thead><tr>
            <th v-if="showFile">{{ t('File') }}</th><th :title="t('Line in the translation file')">{{ t('Line') }}</th>
            <th :title="t('The original line in the game script (game/…:number) as Ren\'Py recorded it')">{{ t('Script') }}</th>
            <th :title="t('Dialogue, or Choice (text from “translate strings” – mostly a menu choice or interface text)')">{{ t('Type') }}</th>
            <th>{{ t('State') }}</th><th>{{ t('Proofreading') }}</th>
            <th v-if="mode !== 'name'" :title="t('Speaker abbreviation (and image attributes)')">{{ t('Code') }}</th><th v-if="mode !== 'code'" :title="t('Full name from the Glossary')">{{ t('Name') }}</th>
            <th>{{ t('Original') }}</th><th>{{ t('Translation') }}</th><th>{{ t('Check') }}</th>
          </tr></thead>
          <tbody>
            <tr v-for="(it, i) in items" :key="it.file + ':' + it.line" class="ln" :class="{ sel: sel === i, picked: isPicked(it) }" @click="rowClick($event, i, it)" @dblclick="post('open-editor', { file: it.file, line: it.line })">
              <td v-if="showFile" class="nowrap"><span :title="it.file">{{ baseName(it.file) }}</span></td>
              <td class="num">{{ it.line }}</td>
              <td class="mono muted nowrap">{{ it.ref ? it.ref.replace(/^game\//, '') : '–' }}</td>
              <td>{{ it.unit === 'string' ? t('Choice') : t('Dialogue') }}</td>
              <td class="nowrap"><span class="pill" :class="pillOf(it.status)">{{ lineState(it.status) }}</span><div v-if="it.kind && it.status !== 'translated'" class="faint small">{{ lineKind(it.kind) }}</div></td>
              <td class="nowrap"><template v-if="it.status === 'todo'"><span class="muted">–</span></template>
                <span v-else-if="it.proof === 'ok'" class="pill checked">{{ t('✓ Proofread') }}</span><span v-else-if="it.proof === 'fix'" class="pill todo">{{ t('⚑ needs a fix') }}</span>
                <span v-else-if="it.proof === 'stale'" class="pill none">{{ t('↻ changed after proofreading') }}</span><span v-else class="muted">{{ t('Not proofread') }}</span></td>
              <td v-if="mode !== 'name'" class="mono">{{ it.who }}</td><td v-if="mode !== 'code'">{{ full(it) || (mode === 'name' ? it.who : '') }}</td>
              <td class="txt"><div class="clamp">{{ shown(it.source) }}</div></td><td class="txt"><div class="clamp">{{ shown(it.target) }}</div></td>
              <td><span v-if="it.issues.length" class="pill" :class="it.issues.some((x) => x.severity === 'error') ? 'danger' : 'todo'">{{ it.issues.length }}×</span></td>
            </tr>
          </tbody>
        </table>
        <div v-if="loading" class="skeleton" style="height: 30px; margin: var(--s-3)"></div>
        <div v-else-if="!total" class="empty muted"><b>{{ t('Nothing found') }}</b> {{ f.status === 'todo' ? t('There is nothing left to translate here – or change the filters.') : t('Try changing the filters or the search text.') }}</div>
        <div v-else-if="offset + items.length < total" class="more-row"><button class="btn" @click="load(false)">{{ t('Load {n} more (shown {shown} of {total})', { n: Math.min(200, total - offset - items.length), shown: fmtN(offset + items.length), total: fmtN(total) }) }}</button></div>
      </div>

      <aside ref="asideEl" class="detail card">
        <ResultsBar v-if="ui.layout === 'film' && narrowed && total >= 2" :items="items" :total="total" :offset="offset" :sel="sel" :summary="summary" :more="offset + items.length < total" @select="select" @more="load(false)" />
        <LineDetail v-if="detail" ref="ld" :layout="ui.layout" :quick="ui.quick" :key="detail.file + ':' + detail.line" :detail="detail" :position="{ i: sel, n: items.length }" @updated="updated" @approve="doApprove" @proof="doProof" @goto="goto" @step="step" @error="error = $event" />
        <div v-else-if="!total && !loading" class="empty muted"><b>{{ t('Nothing found') }}</b> {{ f.status === 'todo' ? t('There is nothing left to translate here – or change the filters.') : t('Try changing the filters or the search text.') }}</div>
        <template v-else><h3 class="card-title">{{ t('Line detail') }}</h3><p class="muted">{{ t('Select a line. The arrow keys ↑ ↓ move through the lines, {a} approves, {e} opens the editor, {q} shows all shortcuts.', { a: 'A', e: 'E', q: '?' }) }}</p></template>
      </aside>
    </div>
      <MtDialog v-if="mtOpen" :picked="pickedList" :anchor="detail" :file="f.file" @close="mtOpen = false" @done="mtOpen = false; after()" @settings="mtOpen = false" />
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-4); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; height: calc(100vh - var(--topbar-h, 43px)); box-sizing: border-box; overflow: hidden; }
.toolbar { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.toolbar.more label { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-small); color: var(--text-muted); }
.check { display: flex; gap: var(--s-1); align-items: center; font-size: var(--fs-small); color: var(--text-muted); white-space: nowrap; }
.pills { display: flex; flex-wrap: wrap; gap: var(--s-1); }
.search { width: 240px; }
.grow { flex: 1; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 200px; }
.sel.wide { max-width: 260px; }
.split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(380px, 520px); gap: var(--s-3); align-items: stretch; flex: 1; min-height: 0; }
.split > div { min-height: 0; overflow: auto; }
.split.film { grid-template-columns: minmax(0, 1fr); } .split.film .tablewrap { display: none; }
.split.rows { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(120px, 38%) minmax(0, 1fr); }
.split.cols { grid-template-columns: minmax(0, 1fr) minmax(380px, 520px); }
.screen.focus { padding-top: var(--s-10); }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line); z-index: 1; white-space: nowrap; }
.grid td { padding: 6px var(--s-3); border-bottom: 1px solid var(--line-soft); vertical-align: top; }
.ln { cursor: default; }
.ln:hover { background: var(--bg-hover); }
.ln.picked { background: var(--accent-soft); }
.ln.sel { background: var(--bg-hover); }
.ln.sel td:first-child { box-shadow: inset 3px 0 0 var(--accent-hi); }
.nowrap { white-space: nowrap; }
.txt { min-width: 200px; max-width: 300px; }
.clamp { overflow: hidden; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; line-height: var(--lh-tight); word-break: break-word; }
.num { font-variant-numeric: tabular-nums; text-align: right; }
.small { font-size: var(--fs-small); }
.more-row, .empty { padding: var(--s-5); text-align: center; }
.detail { padding: var(--s-4); min-height: 0; overflow: auto; }
@media (max-width: 1100px) { .screen { height: auto; overflow: visible; } .split:not(.cols) { grid-template-columns: 1fr; } .split > div { overflow: visible; } .tablewrap { max-height: 50vh; overflow: auto; } }
</style>
