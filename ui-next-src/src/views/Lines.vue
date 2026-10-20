<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { get, post } from '../api.js';
import LineDetail from '../components/LineDetail.vue';
import Tools from '../components/Tools.vue';
import { t, tn } from '../i18n.js';
import { baseName, fmtN, lineKind, lineState, loadFiles, loadNames, loadOverview, pillOf, project } from '../project.js';

/* Lines: every line of the translation with all its columns (file, line, script, type, state, proofreading, code, name, original, translation, check);
   the line you click opens on the right, where it is translated, approved and marked as proofread. */
const emit = defineEmits(['go']);
const error = ref('');
const items = ref([]);
const total = ref(0);
const counts = ref(null);
const offset = ref(0);
const loading = ref(false);
const sel = ref(-1);
const detail = ref(null);
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
    if (resetList && around) { const i = items.value.findIndex((x) => x.line === around); around = 0; if (i >= 0) await select(i); }
  } catch (e) { error.value = e.message; } finally { loading.value = false; }
}
function change() { clearTimeout(timer); timer = setTimeout(() => load(true), 220); }
watch(f, change, { deep: true });

async function select(i) {
  if (i < 0 || i >= items.value.length) return;
  if (ld.value && ld.value.dirty && !window.confirm(t('You changed the text of this line and did not save it. Discard the changes?'))) return;
  sel.value = i;
  const it = items.value[i];
  try {
    detail.value = await get('entry', { file: it.file, line: it.line });
    try { localStorage.setItem('aside.last', JSON.stringify({ file: it.file, line: it.line })); } catch { /* only for the Continue button */ }
    post('focus', { file: it.file, line: it.line }).catch(() => {});
    await nextTick();
    document.querySelector('.ln.sel')?.scrollIntoView({ block: 'nearest' });
    if (i >= items.value.length - 15 && offset.value + items.value.length < total.value) load(false);
  } catch (e) { error.value = e.message; detail.value = null; }
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
async function updated(ev = {}) {
  error.value = '';
  try {
    const i = sel.value, d = detail.value;
    if (ev.reload) { await load(true); loadOverview().catch(() => {}); loadFiles().catch(() => {}); return; }
    detail.value = await get('entry', { file: d.file, line: d.line });
    mirror();
    loadOverview().catch(() => {}); loadFiles().catch(() => {});
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

function key(ev) {
  if (ev.target.closest('textarea, input, select, [role="dialog"]')) return;
  const L = ld.value;
  if (ev.key === 'ArrowDown') { ev.preventDefault(); select(sel.value + 1); }
  else if (ev.key === 'ArrowUp') { ev.preventDefault(); select(sel.value - 1); }
  else if (!L || ev.ctrlKey || ev.metaKey || ev.altKey) return;
  else if (ev.key === 'A') L.openIdentical();
  else if (ev.key === 'a') L.approve();
  else if (ev.key === 'P') L.proofUpto('ok');
  else if (ev.key === 'U') L.proofUpto('');
  else if (ev.key === 'p') L.toggleProof();
  else if (ev.key === 'f' || ev.key === 'F') L.toggleFix();
  else if (ev.key === 'e' || ev.key === 'E') L.openEditor();
}
const who = (it) => (it.who || '').trim().split(/\s+/)[0];
const full = (it) => project.names[who(it)] || '';
const shown = (s) => String(s ?? '').replace(/\n/g, ' ↵ ');
const after = async () => { await load(true); loadOverview().catch(() => {}); loadFiles().catch(() => {}); };

onMounted(async () => {
  window.addEventListener('keydown', key);
  loadNames();
  try { if (!project.files) await loadFiles(); } catch (e) { error.value = e.message; }
  load(true);
});
onBeforeUnmount(() => { window.removeEventListener('keydown', key); clearTimeout(timer); });
</script>

<template>
  <main class="screen">
    <div class="toolbar">
      <label class="field search"><input v-model="f.q" type="search" :placeholder="t('Search in the text… (Ctrl+F)')" @input="speaker = ''"></label>
      <label class="check" :title="t('Whole word')"><input type="checkbox" v-model="f.word"> {{ t('Whole word') }}</label>
      <select class="sel wide" v-model="f.file" :title="t('File')"><option value="">{{ t('All files') }}</option><option v-for="x in files" :key="x.name" :value="x.name">{{ x.name }}</option></select>
      <select class="sel" v-model="speaker" :title="t('Speaker')" :aria-label="t('Speaker')" @change="pickSpeaker"><option value="">{{ t('Speaker: all') }}</option><option v-for="[code, name] in speakers" :key="code" :value="code">{{ name }} · {{ code }}</option></select>
      <button class="btn" :aria-expanded="moreOpen" :title="t('Line type, kind, check, sorting, speaker display')" @click="moreOpen = !moreOpen">{{ moreOpen ? '▾' : '▸' }} {{ t('More filters') }}<template v-if="extra"> ({{ extra }})</template></button>
      <Tools :picked="pickedList" :anchor="detail" :file="f.file" @changed="after" />
      <template v-if="picked.size"><span class="muted small">{{ tn(picked.size, 'Selected {n} line', 'Selected {n} lines') }}</span><button class="btn small ghost" @click="picked = new Set()">{{ t('Clear the selection') }}</button></template>
      <span class="grow"></span>
      <span class="muted small">{{ tn(total, '{n} line', '{n} lines', { n: fmtN(total) }) }}</span>
    </div>
    <div class="pills" role="group" :aria-label="t('State of the lines')">
      <button v-for="[k, l] in STATUS_OPTS" :key="k" class="chip" :class="{ on: f.status === k }" @click="f.status = k">{{ l }}<span v-if="counts && counts[k] !== undefined" class="faint"> {{ fmtN(counts[k]) }}</span></button>
    </div>
    <div v-if="moreOpen" class="toolbar more">
      <label>{{ t('Line type') }} <select class="sel" v-model="f.unit"><option v-for="[v, l] in OPTS.unit" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Kind of untranslated') }} <select class="sel" v-model="f.kind"><option v-for="[v, l] in OPTS.kind" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Check') }} <select class="sel" v-model="f.issues"><option v-for="[v, l] in OPTS.issues" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Sort by') }} <select class="sel" v-model="f.sort"><option v-for="[v, l] in OPTS.sort" :key="v" :value="v">{{ l }}</option></select></label>
      <label>{{ t('Speaker') }} <select class="sel" v-model="mode"><option v-for="[v, l] in OPTS.mode" :key="v" :value="v">{{ l }}</option></select></label>
    </div>
    <div v-if="chips.length" class="toolbar chips"><button v-for="([label, off], i) in chips" :key="i" class="chip on" :title="t('Remove this filter')" @click="off">{{ label }} ✕</button><button class="btn small ghost" @click="reset">{{ t('Clear all filters') }}</button></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>

    <div class="split">
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

      <aside class="detail card">
        <LineDetail v-if="detail" ref="ld" :key="detail.file + ':' + detail.line" :detail="detail" :position="{ i: sel, n: items.length }" @updated="updated" @goto="goto" @step="step" @error="error = $event" />
        <template v-else><h3 class="card-title">{{ t('Line detail') }}</h3><p class="muted">{{ t('Select a line. The arrow keys ↑ ↓ move through the lines, {a} approves, {e} opens the editor, {q} shows all shortcuts.', { a: 'A', e: 'E', q: '?' }) }}</p></template>
      </aside>
    </div>
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; }
.toolbar { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.toolbar.more label { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-small); color: var(--text-muted); }
.check { display: flex; gap: var(--s-1); align-items: center; font-size: var(--fs-small); color: var(--text-muted); white-space: nowrap; }
.pills { display: flex; flex-wrap: wrap; gap: var(--s-1); }
.search { width: 240px; }
.grow { flex: 1; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 200px; }
.sel.wide { max-width: 260px; }
.split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(380px, 520px); gap: var(--s-3); align-items: start; }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); max-height: calc(100vh - 200px); }
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
.detail { padding: var(--s-4); position: sticky; top: var(--s-3); max-height: calc(100vh - 120px); overflow: auto; }
@media (max-width: 1100px) { .split { grid-template-columns: 1fr; } .detail { position: static; max-height: none; } }
</style>
