<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { get, post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName, fmtN, lineKind, lineState, loadFiles, loadNames, loadOverview, pillOf, project } from '../project.js';

/* Lines: every line of the translation with all its columns (file, line, script, type, state, proofreading, code, name, original, translation, check);
   the line you click opens on the right, where it is translated, approved and marked as proofread. */
const error = ref('');
const items = ref([]);
const total = ref(0);
const counts = ref(null);
const offset = ref(0);
const loading = ref(false);
const sel = ref(-1);
const detail = ref(null);
const text = ref('');
const live = ref(null);            // the check of the edited text
const saving = ref(false);
const note = ref('');
const moreOpen = ref(false);
const f = ref({ file: project.lines.file || '', status: project.lines.status || 'todo', q: project.lines.q || '', unit: '', kind: '', issues: '', sort: 'line' });
let around = project.lines.line || 0;
let timer = null;
let checkTimer = null;

const files = computed(() => project.files || []);
const dirty = computed(() => !!detail.value && text.value !== detail.value.target);
const showFile = computed(() => !f.value.file);
const STATUS_OPTS = computed(() => [['all', t('All')], ['todo', t('To translate')], ['translated', t('Translated ones')], ['approved', t('Approved ones')], ['review', t('For proofreading')], ['fix', t('To fix')], ['stale', t('Changed after proofreading')], ['proofed', t('Proofread ones')]]);
const OPTS = computed(() => ({
  unit: [['', t('Dialogues and choices')], ['dialogue', t('Dialogues only')], ['string', t('Choices only (strings)')]],
  kind: [['', t('All kinds')], ['english', t('In English')], ['same', t('Identical, maybe OK')], ['symbols', t('Symbols only')], ['empty', t('Empty')]],
  issues: [['', t('All')], ['any', t('With a problem')], ['error', t('With an error')], ['warning', t('With a warning')], ['overflow', t('Does not fit the game window')]],
  sort: [['line', t('Line')], ['file', t('File')], ['kind', t('Kind')], ['issues', t('Problems')]],
}));
const extra = computed(() => ['unit', 'kind', 'issues'].filter((k) => f.value[k]).length);

async function load(reset) {
  if (loading.value) return;
  loading.value = true; error.value = '';
  try {
    if (reset) { items.value = []; sel.value = -1; detail.value = null; offset.value = 0; }
    const query = { ...f.value, word: '', limit: 200, offset: reset ? 0 : offset.value + items.value.length, around: reset ? around : 0 };
    if (reset && around) query.status = 'all';
    const r = await get('entries', query);
    if (reset) offset.value = r.offset;
    items.value.push(...r.items);
    total.value = r.total; counts.value = r.counts;
    if (reset && around) { const i = items.value.findIndex((x) => x.line === around); around = 0; if (i >= 0) await select(i); }
  } catch (e) { error.value = e.message; } finally { loading.value = false; }
}
function change() { clearTimeout(timer); timer = setTimeout(() => load(true), 220); }
watch(f, change, { deep: true });

async function select(i) {
  if (i < 0 || i >= items.value.length) return;
  if (dirty.value && !window.confirm(t('You changed the text of this line and did not save it. Discard the changes?'))) return;
  sel.value = i;
  const it = items.value[i];
  try {
    detail.value = await get('entry', { file: it.file, line: it.line });
    text.value = detail.value.target; live.value = null; note.value = '';
    await nextTick();
    document.querySelector('.ln.sel')?.scrollIntoView({ block: 'nearest' });
    if (i >= items.value.length - 15 && offset.value + items.value.length < total.value) load(false);
  } catch (e) { error.value = e.message; detail.value = null; }
}

watch(text, () => {
  clearTimeout(checkTimer);
  if (!dirty.value) { live.value = null; return; }
  const d = detail.value;
  checkTimer = setTimeout(async () => {
    try { const r = await post('entry/check', { file: d.file, line: d.line, text: text.value }); if (detail.value === d) live.value = r; } catch { /* the live check is only a hint */ }
  }, 350);
});

function mirror() {
  // the row of the list follows the line that was just changed
  const d = detail.value, it = items.value[sel.value];
  if (!d || !it) return;
  Object.assign(it, { target: d.target, status: d.status, proof: d.proof, kind: d.kind, issues: (d.issues || []).filter((x) => x.severity !== 'info').map((x) => ({ severity: x.severity, code: x.code })) });
}
async function reloadDetail() {
  const d = detail.value;
  detail.value = await get('entry', { file: d.file, line: d.line });
  text.value = detail.value.target; live.value = null;
  mirror();
  loadOverview().catch(() => {}); loadFiles().catch(() => {});
}
const canSave = computed(() => dirty.value && !saving.value && !(live.value && live.value.literal_error));
async function save(advance) {
  if (!canSave.value) return;
  const d = detail.value; saving.value = true; error.value = '';
  try {
    await post('entry/save', { file: d.file, line: d.line, text: text.value });
    const i = sel.value;
    await reloadDetail();
    if (advance) { let k = i + 1; while (k < items.value.length && items.value[k].status !== 'todo') k++; await select(k < items.value.length ? k : Math.min(i + 1, items.value.length - 1)); }
  } catch (e) { error.value = e.message; } finally { saving.value = false; }
}
async function approve() {
  const d = detail.value; if (!d) return;
  const on = d.status !== 'approved';
  try { await post('approve/apply', { selections: [{ file: d.file, key: d.key }], on }); await reloadDetail(); if (on && f.value.status === 'todo') await select(Math.min(sel.value + 1, items.value.length - 1)); } catch (e) { error.value = e.message; }
}
async function proof(state, noteText) {
  const d = detail.value; if (!d || d.status === 'todo') return;
  try {
    const r = await post('proof/set', { items: [{ file: d.file, key: d.key }], state, note: noteText });
    if (r.skipped && r.skipped.length) { error.value = r.skipped[0][1]; return; }
    await reloadDetail();
    if (state === 'ok') { let k = sel.value + 1; while (k < items.value.length && (items.value[k].status === 'todo' || ['ok', 'fix'].includes(items.value[k].proof))) k++; if (k < items.value.length) await select(k); }
  } catch (e) { error.value = e.message; }
}
const toggleProof = () => proof(detail.value.proof === 'ok' ? '' : 'ok');
const toggleFix = () => proof(detail.value.proof === 'fix' ? '' : 'fix', note.value);
async function undo() { try { await post('fix/undo'); await load(true); loadOverview().catch(() => {}); } catch (e) { error.value = e.message; } }
async function openEditor() { const d = detail.value; try { await post('open-editor', { file: d.file, line: d.line }); } catch (e) { error.value = e.message; } }
async function copy(s) { try { await navigator.clipboard.writeText(s); } catch { error.value = t('Copying failed.'); } }
async function jump(kind) {
  const d = detail.value; if (!d) return;
  try {
    const r = await get('entry/next', { file: d.file, line: d.line, kind });
    if (!r.line) { error.value = t('There is no such line left in this file.'); return; }
    const i = items.value.findIndex((x) => x.file === d.file && x.line === r.line);
    if (i >= 0) await select(i); else { f.value = { ...f.value, file: d.file, status: 'all' }; around = r.line; }
  } catch (e) { error.value = e.message; }
}

function key(ev) {
  if (ev.target.closest('textarea, input, select')) { if (ev.key === 'Enter' && (ev.ctrlKey || ev.metaKey)) { ev.preventDefault(); save(true); } return; }
  if (ev.key === 'ArrowDown') { ev.preventDefault(); select(sel.value + 1); }
  else if (ev.key === 'ArrowUp') { ev.preventDefault(); select(sel.value - 1); }
  else if (ev.key === 'a' || ev.key === 'A') approve();
  else if (ev.key === 'p' || ev.key === 'P') toggleProof();
}
const who = (it) => (it.who || '').trim().split(/\s+/)[0];
const full = (it) => project.names[who(it)] || '';
const shown = (s) => String(s ?? '').replace(/\n/g, ' ↵ ');

onMounted(async () => {
  window.addEventListener('keydown', key);
  loadNames();
  try { if (!project.files) await loadFiles(); } catch (e) { error.value = e.message; }
  load(true);
});
onBeforeUnmount(() => { window.removeEventListener('keydown', key); clearTimeout(timer); clearTimeout(checkTimer); });
</script>

<template>
  <main class="screen">
    <div class="toolbar">
      <label class="field search"><input v-model="f.q" type="search" :placeholder="t('Search in the text… (Ctrl+F)')"></label>
      <select class="sel wide" v-model="f.file" :title="t('File')"><option value="">{{ t('All files') }}</option><option v-for="x in files" :key="x.name" :value="x.name">{{ x.name }}</option></select>
      <button class="btn" :aria-expanded="moreOpen" @click="moreOpen = !moreOpen">{{ moreOpen ? '▾' : '▸' }} {{ t('More filters') }}<template v-if="extra"> ({{ extra }})</template></button>
      <span class="grow"></span>
      <button class="btn ghost" :title="t('↶ Undo the last file change')" @click="undo">{{ t('↶ Undo the last file change') }}</button>
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
    </div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>

    <div class="split">
      <div class="tablewrap">
        <table class="grid">
          <thead><tr>
            <th v-if="showFile">{{ t('File') }}</th><th :title="t('Line in the translation file')">{{ t('Line') }}</th>
            <th :title="t('The original line in the game script (game/…:number) as Ren\'Py recorded it')">{{ t('Script') }}</th>
            <th :title="t('Dialogue, or Choice (text from “translate strings” – mostly a menu choice or interface text)')">{{ t('Type') }}</th>
            <th>{{ t('State') }}</th><th>{{ t('Proofreading') }}</th>
            <th :title="t('Speaker abbreviation (and image attributes)')">{{ t('Code') }}</th><th :title="t('Full name from the Glossary')">{{ t('Name') }}</th>
            <th>{{ t('Original') }}</th><th>{{ t('Translation') }}</th><th>{{ t('Check') }}</th>
          </tr></thead>
          <tbody>
            <tr v-for="(it, i) in items" :key="it.file + ':' + it.line" class="ln" :class="{ sel: sel === i }" @click="select(i)" @dblclick="openEditor">
              <td v-if="showFile" class="nowrap"><span :title="it.file">{{ baseName(it.file) }}</span></td>
              <td class="num">{{ it.line }}</td>
              <td class="mono muted nowrap">{{ it.ref ? it.ref.replace(/^game\//, '') : '–' }}</td>
              <td>{{ it.unit === 'string' ? t('Choice') : t('Dialogue') }}</td>
              <td class="nowrap"><span class="pill" :class="pillOf(it.status)">{{ lineState(it.status) }}</span><div v-if="it.kind && it.status !== 'translated'" class="faint small">{{ lineKind(it.kind) }}</div></td>
              <td class="nowrap"><template v-if="it.status === 'todo'"><span class="muted">–</span></template>
                <span v-else-if="it.proof === 'ok'" class="pill checked">{{ t('✓ Proofread') }}</span><span v-else-if="it.proof === 'fix'" class="pill todo">{{ t('⚑ needs a fix') }}</span>
                <span v-else-if="it.proof === 'stale'" class="pill none">{{ t('↻ changed after proofreading') }}</span><span v-else class="muted">{{ t('Not proofread') }}</span></td>
              <td class="mono">{{ it.who }}</td><td>{{ full(it) }}</td>
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
        <template v-if="detail">
          <div class="dhead">
            <span class="path mono small" :title="detail.ref ? t('original script: {ref}', { ref: detail.ref }) : ''">{{ detail.file }}:{{ detail.line }}</span>
            <span class="pill" :class="pillOf(detail.status)">{{ lineState(detail.status) }}</span>
            <span v-if="detail.proof === 'ok'" class="pill checked">{{ t('✓ Proofread') }}</span><span v-else-if="detail.proof === 'fix'" class="pill todo">{{ t('⚑ needs a fix') }}</span>
          </div>
          <div class="actions">
            <button v-if="detail.status !== 'translated' && detail.kind !== 'empty'" class="btn primary" @click="approve">{{ detail.status === 'approved' ? t('Unapprove (A)') : t('Approve (A)') }}</button>
            <button v-if="detail.status !== 'todo'" class="btn" @click="toggleProof">{{ detail.proof === 'ok' ? t('Remove the mark (P)') : t('✓ Proofread (P)') }}</button>
            <button v-if="detail.status !== 'todo'" class="btn" @click="toggleFix">{{ detail.proof === 'fix' ? t('Clear “to fix” (F)') : t('⚑ To fix (F)') }}</button>
            <button class="btn" :title="t('Opens the file at this line in an external editor')" @click="openEditor">{{ t('Editor (E)') }}</button>
            <button class="btn ghost" @click="copy(detail.path + ':' + detail.line)">{{ t('Copy the path') }}</button>
          </div>
          <div class="pair">
            <div class="head"><h4>{{ t('Original') }}</h4><button class="btn small ghost" :title="t('Copies the original text to the clipboard')" @click="copy(detail.source ?? '')">{{ t('⧉ Original') }}</button></div>
            <div class="box">{{ detail.source ?? t('(source not found)') }}</div>
            <div class="head"><h4>{{ t('Translation') }}</h4>
              <span><button class="btn small ghost" :title="t('Puts the original text into the field (handy as a base with the right tags already)')" @click="text = detail.source ?? ''">{{ t('Original into the field') }}</button>
                <button class="btn small ghost" :disabled="!dirty" @click="text = detail.target">{{ t('Restore the text') }}</button></span></div>
            <textarea v-model="text" class="edit" rows="5" :aria-label="t('Translation')" spellcheck="false"></textarea>
            <div class="actions">
              <button class="btn primary" :disabled="!canSave" :title="t('Writes the text to the .rpy file (a backup is made before writing)')" @click="save(false)">{{ t('Save to the file (Ctrl+Enter)') }}</button>
            </div>
            <div v-if="live" class="issues">
              <h4>{{ t('Check of the edited text') }}</h4>
              <div v-if="live.literal_error" class="issue error"><span class="pill danger">{{ t('Cannot save') }}</span> {{ live.literal_error }}</div>
              <div v-for="(x, i) in live.issues" :key="i" class="issue"><span class="pill" :class="x.severity === 'error' ? 'danger' : 'todo'">{{ x.severity === 'error' ? t('Error') : t('Warning') }}</span> {{ x.message }}</div>
              <div v-if="!live.literal_error && !live.issues.length" class="muted">{{ t('No findings.') }}</div>
            </div>
          </div>
          <div v-if="detail.issues && detail.issues.length" class="issues">
            <h4>{{ t('Check') }}</h4>
            <div v-for="(x, i) in detail.issues" :key="i" class="issue"><span class="pill" :class="x.severity === 'error' ? 'danger' : 'todo'">{{ x.severity === 'error' ? t('Error') : t('Warning') }}</span> {{ x.message }}</div>
          </div>
          <label v-if="detail.status !== 'todo'" class="label">{{ t('Note for the proofreader') }}
            <span class="field"><input v-model="note" :placeholder="t('What needs to be fixed?')" @keydown.enter="proof('fix', note)"></span></label>
          <div class="actions jumps">
            <button class="btn small" :disabled="sel <= 0" :title="t('Previous line (←)')" @click="select(sel - 1)">←</button>
            <button class="btn small" :disabled="sel >= items.length - 1" :title="t('Next line (→)')" @click="select(sel + 1)">→</button>
            <button class="btn small" :title="t('Next untranslated line in the file')" @click="jump('todo')">{{ t('Next untranslated') }}</button>
            <button class="btn small" :title="t('Next translated but unchecked line')" @click="jump('review')">{{ t('Next unchecked') }}</button>
            <button class="btn small" :title="t('Next line that does not fit the game window')" @click="jump('over')">{{ t('Next one that does not fit') }}</button>
          </div>
        </template>
        <template v-else><h3 class="card-title">{{ t('Line detail') }}</h3><p class="muted">{{ t('Select a line. The arrow keys ↑ ↓ move through the lines, {a} approves, {e} opens the editor, {q} shows all shortcuts.', { a: 'A', e: 'E', q: '?' }) }}</p></template>
      </aside>
    </div>
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; }
.toolbar { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.toolbar.more label { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-small); color: var(--text-muted); }
.pills { display: flex; flex-wrap: wrap; gap: var(--s-1); }
.search { width: 260px; }
.grow { flex: 1; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 200px; }
.sel.wide { max-width: 300px; }
.split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(340px, 440px); gap: var(--s-3); align-items: start; }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); max-height: calc(100vh - 200px); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line); z-index: 1; white-space: nowrap; }
.grid td { padding: 6px var(--s-3); border-bottom: 1px solid var(--line-soft); vertical-align: top; }
.ln { cursor: default; }
.ln:hover { background: var(--bg-hover); }
.ln.sel { background: var(--accent-soft); }
.nowrap { white-space: nowrap; }
.txt { min-width: 200px; max-width: 300px; }
.clamp { overflow: hidden; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; line-height: var(--lh-tight); word-break: break-word; }
.num { font-variant-numeric: tabular-nums; text-align: right; }
.small { font-size: var(--fs-small); }
.more-row, .empty { padding: var(--s-5); text-align: center; }
.detail { padding: var(--s-4); position: sticky; top: var(--s-3); max-height: calc(100vh - 120px); overflow: auto; }
.dhead { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin-bottom: var(--s-3); }
.path { word-break: break-all; }
.actions { display: flex; flex-wrap: wrap; gap: var(--s-2); margin: var(--s-2) 0; }
.head { display: flex; justify-content: space-between; align-items: center; margin-top: var(--s-3); }
.head h4, .issues h4 { margin: 0; font-size: var(--fs-body); font-weight: 500; color: var(--text-2); }
.box { margin-top: var(--s-1); padding: var(--s-3); background: var(--bg-input); border: 1px solid var(--line); border-radius: var(--r-md); white-space: pre-wrap; font-size: var(--fs-text); line-height: var(--lh-normal); }
.edit { width: 100%; box-sizing: border-box; margin-top: var(--s-1); padding: var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; font-size: var(--fs-text); line-height: var(--lh-normal); resize: vertical; }
.edit:focus { outline: none; border-color: var(--accent-hi); }
.issues { margin-top: var(--s-3); display: flex; flex-direction: column; gap: var(--s-2); }
.issue { font-size: var(--fs-body); }
.label { margin-top: var(--s-3); }
@media (max-width: 1100px) { .split { grid-template-columns: 1fr; } .detail { position: static; max-height: none; } }
</style>
