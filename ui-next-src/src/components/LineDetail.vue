<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { get, post } from '../api.js';
import { contextFor, menuOf, mtOn, peekContext, peekMenu, peekMt, peekScenes, sceneRadius, scenesOf, setSceneRadius } from '../gamewindow.js';
import { t, tn } from '../i18n.js';
import { baseName, fmtN, lineKind, lineState, pctOf, pillOf, project } from '../project.js';
import ApproveIdentical from './ApproveIdentical.vue';
import Fold from './Fold.vue';
import GamePreview from './GamePreview.vue';
import MtDialog from './MtDialog.vue';

/* The detail of a line: the original, the editor (tags, missing variables, glossary terms, machine suggestion, live check), the progress of its file with the jumps
   (scenes, next untranslated / unchecked / too long) and the tabs Check, Proofreading, Menu, Scene, Memory, History and Script. */
const props = defineProps({ detail: { type: Object, required: true }, position: { type: Object, default: () => ({ i: 0, n: 0 }) } });
const emit = defineEmits(['updated', 'goto', 'step', 'advance', 'error']);

const text = ref(props.detail.target);
const live = ref(null);
const saving = ref(false);
const note = ref(props.detail.proof_note || '');
const ta = ref(null);
const menuOpen = ref(false);
const identical = ref(false);
const mt = ref({ on: !!peekMt(), busy: false, result: null, error: '' });
const scenes = ref(peekScenes(props.detail.file));
const tab = ref(storedTab());
const menuAvail = ref(false);      // a line of dialogue that belongs to a menu (its question, the start of a choice, the lines before it) gets the Menu tab too
const sceneMode = ref(storedMode());
const mtScene = ref(false);
const radius = ref(sceneRadius());             // how many lines around the open one the Scene tab shows
const data = ref({});              // what the tabs fetched: scene, memory, history, context, menu
let checkTimer = null;

function storedTab() { try { return localStorage.getItem('aside.detailTab') || ''; } catch { return ''; } }
function storedMode() { try { return localStorage.getItem('aside.sceneMode') === 'bubbles' ? 'bubbles' : 'table'; } catch { return 'table'; } }
function setMode(k) { sceneMode.value = k; try { localStorage.setItem('aside.sceneMode', k); } catch { /* only for this page */ } }
/* The speaker as the lists show it: the full name from the glossary and the code, or the code alone. */
const code = (who) => String(who || '').trim().split(/\s+/)[0];
const speaker = (who) => { const n = project.names[code(who)]; return n ? `${n} · ${who}` : who || '—'; };
const speakerColor = (who) => { let hue = 0; for (const ch of code(who)) hue = (hue * 31 + ch.charCodeAt(0)) % 360; return code(who) ? `hsl(${hue} 55% 52%)` : ''; };
function setTab(k) { tab.value = k; try { localStorage.setItem('aside.detailTab', k); } catch { /* the choice is only remembered while the page lives */ } loadTab(); }

const d = computed(() => props.detail);
const dirty = computed(() => text.value !== d.value.target);
const canSave = computed(() => dirty.value && !saving.value && !(live.value && live.value.literal_error));
const issues = computed(() => d.value.issues || []);
const open = computed(() => issues.value.filter((x) => !x.ignored));
const fileRow = computed(() => (project.files || []).find((f) => f.name === d.value.file));
const isChoice = computed(() => d.value.unit === 'string');

const TABS = computed(() => [
  ['check', open.value.length ? t('Check ({n})', { n: open.value.length }) : t('Check')],
  ['proof', t('Proofreading')],
  ...((isChoice.value && d.value.ref) || menuAvail.value ? [['menu', t('Menu')]] : []),
  ['scene', t('Scene')], ['memory', t('Memory')], ['history', t('History')],
  ...(project.team && project.team.exists ? [['team', t('Team')]] : []),
  ['script', t('Script')],
]);
const current = computed(() => (TABS.value.some(([k]) => k === tab.value) ? tab.value : open.value.length ? 'check' : isChoice.value && d.value.ref ? 'menu' : 'scene'));

watch(() => props.detail, (now, before) => {
  text.value = props.detail.target; live.value = null; note.value = props.detail.proof_note || ''; mt.value.result = null; mt.value.error = '';
  // the same line read again (after approving, saving, marking): the tabs keep what does not depend on it, so nothing flashes or moves
  const same = before && before.file === now.file && before.line === now.line;
  data.value = same ? { menu: data.value.menu, context: data.value.context } : {};
  if (!same) menuAvail.value = false;
  loadScenes(same); loadTab(); if (!same) checkMenu();
}, { immediate: true });

/* Does this line of dialogue belong to a menu? The answer is kept as the data of the Menu tab, so opening the tab costs nothing more. */
async function checkMenu() {
  const line = d.value;
  if (isChoice.value || !line.ref) return;
  try {
    const r = peekMenu(line.file, line.line) || await menuOf(line.file, line.line);
    if (props.detail !== line || !r.available) return;
    data.value.menu = r; menuAvail.value = true;
  } catch { /* no menu tab */ }
}

watch(text, () => {
  clearTimeout(checkTimer);
  if (!dirty.value) { live.value = null; return; }
  const line = d.value;
  checkTimer = setTimeout(async () => {
    try { const r = await post('entry/check', { file: line.file, line: line.line, text: text.value }); if (props.detail === line) live.value = r; } catch { /* the live check is only a hint */ }
  }, 300);
});
onBeforeUnmount(() => clearTimeout(checkTimer));


/* ---- the tabs fetch their data only when they are shown */
async function loadTab() {
  const k = current.value, line = d.value, c = data.value;
  const q = { file: line.file, line: line.line };
  try {
    if (k === 'scene' && !c.scene) {
      c.scene = peekContext(line.file, line.line, radius.value) || 'loading';      // the list fetched it ahead: no "Loading…" between two lines
      if (c.scene === 'loading') c.scene = await contextFor(line.file, line.line, radius.value);
    }
    else if (k === 'memory' && !c.memory) { c.memory = 'loading'; c.memory = await get('entry/memory', q); }
    else if (k === 'history' && !c.history) { c.history = 'loading'; c.history = await get('entry/history', q); }
    else if (k === 'script' && line.ref && !c.context) { c.context = 'loading'; c.context = await get('entry/context', q); }
    else if (k === 'menu' && !c.menu) { c.menu = peekMenu(line.file, line.line) || 'loading'; if (c.menu === 'loading') c.menu = await menuOf(line.file, line.line); }
    else if (k === 'team' && !c.notes) { c.notes = 'loading'; c.notes = await post('team/notes', q); }
  } catch (e) { c[k === 'script' ? 'context' : k] = { error: e.message }; }
}
watch(radius, (n) => { setSceneRadius(n); data.value.scene = null; loadTab(); });
async function loadScenes(keep) {
  const file = d.value.file, known = peekScenes(file);
  if (!(keep && !known)) scenes.value = known;                                 // what is known is shown at once; for a file not seen yet the strip keeps its place (disabled) until the scenes arrive
  if (!known) { const r = await scenesOf(file); if (d.value.file === file) scenes.value = r; }
}
const sceneIndex = computed(() => { let k = -1; (scenes.value || []).forEach((s, i) => { if (s.line <= d.value.line) k = i; }); return k; });

/* ---- the editor */
const TOKEN = /\{[^}]*\}|\[[^\]]*\]/g;
const missing = computed(() => {
  const have = {}; (text.value.match(TOKEN) || []).forEach((x) => { have[x] = (have[x] || 0) + 1; });
  const need = {}; ((d.value.source || '').match(TOKEN) || []).forEach((x) => { need[x] = (need[x] || 0) + 1; });
  return Object.keys(need).filter((x) => !x.startsWith('{#') && (have[x] || 0) < need[x]);
});
const terms = computed(() => (d.value.terms || []).map((x) => ({ ...x, used: text.value.toLowerCase().includes(x.cz.toLowerCase().slice(0, Math.max(3, x.cz.length - 2))) })));
function insert(s) {
  const el = ta.value; if (!el) return;
  const a = el.selectionStart, b = el.selectionEnd;
  el.setRangeText(s, a, b, 'end'); text.value = el.value; el.focus();
}
function wrap(open_, close_) {
  const el = ta.value; if (!el) return;
  const a = el.selectionStart, b = el.selectionEnd;
  el.setRangeText(open_ + el.value.slice(a, b) + close_, a, b, 'end');
  if (a === b) el.selectionStart = el.selectionEnd = a + open_.length;
  text.value = el.value; el.focus();
}
const SNIPS = [
  ['{i}…', () => wrap('{i}', '{/i}'), 'Italic – wraps the selected text'], ['{b}…', () => wrap('{b}', '{/b}'), 'Bold'], ['{u}…', () => wrap('{u}', '{/u}'), 'Underlined'],
  ['{w}', () => insert('{w}'), 'Pause, waits for a click'], ['{p}', () => insert('{p}'), 'A new paragraph with a pause'], ['{nw}', () => insert('{nw}'), 'No waiting for a click'],
  ['\\n', () => insert('\\n'), 'Line break (written as \\n in the game text)'], ['…', () => insert('…'), 'Three dots as one character'],
];
function key(ev) {
  if (ev.key === 'Enter' && (ev.ctrlKey || ev.metaKey)) { ev.preventDefault(); save(true); }
  else if (ev.key === 'Escape') { text.value = d.value.target; ev.target.blur(); }
}
async function save(advance) {
  if (!canSave.value) return;
  saving.value = true;
  try { await post('entry/save', { file: d.value.file, line: d.value.line, text: text.value }); emit('updated', { advance }); } catch (e) { emit('error', e.message); } finally { saving.value = false; }
}
async function suggest() {
  mt.value = { ...mt.value, busy: true, result: null, error: '' };
  try { mt.value.result = await post('entry/mt', { file: d.value.file, line: d.value.line }); } catch (e) { mt.value.error = e.message; }
  mt.value.busy = false;
}
if (peekMt() === null) mtOn().then((on) => { mt.value.on = on; });

/* ---- approving, proofreading, the other actions */
async function run(fn) { try { await fn(); } catch (e) { emit('error', e.message); } }
const approve = () => run(async () => {
  const on = d.value.status !== 'approved';
  await post('approve/apply', { selections: [{ file: d.value.file, key: d.value.key }], on });
  emit('updated', { advance: on, kind: 'approve' });
});
async function proof(state, noteText) {
  return run(async () => {
    const r = await post('proof/set', { items: [{ file: d.value.file, key: d.value.key }], state, note: noteText });
    if (r.skipped && r.skipped.length) { emit('error', r.skipped[0][1]); return; }
    emit('updated', { advance: state === 'ok', kind: 'proof' });
  });
}
const toggleProof = () => proof(d.value.proof === 'ok' ? '' : 'ok');
const toggleFix = () => proof(d.value.proof === 'fix' ? '' : 'fix', note.value);
const saveNote = () => { const state = d.value.proof === 'fix' ? 'fix' : d.value.proof === 'ok' ? 'ok' : (note.value.trim() ? 'fix' : ''); if (state) proof(state, note.value); };
const proofUpto = (state) => run(async () => {
  if (state === '' && !window.confirm(t('Remove the proofread mark up to here?'))) return;
  await post('proof/upto', { file: d.value.file, line: d.value.line, state });
  emit('updated', { kind: 'proof' });
});
const undoLast = () => run(async () => { await post('fix/undo'); emit('updated', { reload: true }); });
const openEditor = () => run(async () => { await post('open-editor', { file: d.value.file, line: d.value.line }); });
const copy = (s) => run(async () => { await navigator.clipboard.writeText(s); });
const unignore = (x) => run(async () => { await post('ignore/set', { items: [{ file: d.value.file, line: d.value.line, code: x.code }], on: false }); emit('updated', {}); });
const ignore = (x) => run(async () => {
  const reason = window.prompt(t('Why is it ignored? (optional, you will see it in the list of ignored findings)'), '');
  if (reason === null) return;
  await post('ignore/set', { items: [{ file: d.value.file, line: d.value.line, code: x.code }], on: true, note: reason });
  emit('updated', {});
});
async function jump(kind) {
  try {
    const r = await get('entry/next', { file: d.value.file, line: d.value.line, kind });
    if (!r.line) { emit('error', t('There is no such line left in this file.')); return; }
    emit('goto', { file: d.value.file, line: r.line });
  } catch (e) { emit('error', e.message); }
}
const goScene = (i) => { const s = (scenes.value || [])[i]; if (s) emit('goto', { file: d.value.file, line: s.line }); };
const newNote = ref('');
const addNote = () => run(async () => { data.value.notes = await post('team/note_add', { file: d.value.file, line: d.value.line, text: newNote.value }); newNote.value = ''; });
const resolveNote = (n) => run(async () => { data.value.notes = await post('team/note_resolve', { id: n.id, file: d.value.file, line: d.value.line }); });
const shown = (s) => String(s ?? '');
defineExpose({ dirty, approve, toggleProof, toggleFix, openEditor, proofUpto, openIdentical: () => { identical.value = true; }, focusEditor: () => nextTick(() => { ta.value?.focus(); ta.value?.setSelectionRange(text.value.length, text.value.length); }) });
const diff = (before, after) => {
  const n = Math.min(before.length, after.length);
  let i = 0; while (i < n && before[i] === after[i]) i++;
  let j = 0; while (j < n - i && before[before.length - 1 - j] === after[after.length - 1 - j]) j++;
  return { head: after.slice(0, i), mid: after.slice(i, after.length - j), tail: after.slice(after.length - j) };
};
</script>

<template>
  <div class="ld">
    <div class="dhead">
      <span class="path mono small" :title="d.ref ? t('original script: {ref}', { ref: d.ref }) : ''">{{ d.file }}:{{ d.line }}</span>
      <span class="pill none">{{ isChoice ? t('Choice') : t('Dialogue') }}</span>
      <span class="pill" :class="pillOf(d.status)">{{ lineState(d.status) }}<template v-if="d.kind && d.status !== 'translated'"> · {{ lineKind(d.kind) }}</template></span>
      <template v-if="d.status !== 'todo'"><span v-if="d.proof === 'ok'" class="pill checked">{{ t('✓ Proofread') }}</span><span v-else-if="d.proof === 'fix'" class="pill todo">{{ t('⚑ needs a fix') }}</span><span v-else-if="d.proof === 'stale'" class="pill none">{{ t('↻ changed after proofreading') }}</span><span v-else class="pill none">{{ t('Not proofread') }}</span></template>
    </div>
    <div class="actions">
      <button v-if="d.status !== 'translated' && d.kind !== 'empty'" class="btn primary" @click="approve">{{ d.status === 'approved' ? t('Unapprove (A)') : t('Approve (A)') }}</button>
      <button class="btn" :title="t('Opens the file at this line in an external editor')" @click="openEditor">{{ t('Editor (E)') }}</button>
      <div class="menu-wrap">
        <button class="btn" :aria-expanded="menuOpen" :title="t('More actions with the line')" @click="menuOpen = !menuOpen" @blur="menuOpen = false">{{ t('More') }} ▾</button>
        <div v-if="menuOpen" class="menu" role="menu" @mousedown.prevent>
          <div v-if="d.status !== 'translated' && d.kind !== 'empty'" class="menu-item" role="menuitem" @click="identical = true; menuOpen = false">{{ t('Approve all identical…') }}<span class="faint">Shift+A</span></div>
          <div class="menu-item" role="menuitem" @click="copy(d.path + ':' + d.line); menuOpen = false">{{ t('Copy the path') }}</div>
          <div class="menu-item" role="menuitem" @click="undoLast(); menuOpen = false">{{ t('↶ Undo the last file change') }}</div>
        </div>
      </div>
    </div>

    <div class="filestrip">
      <div class="fsname" :title="d.file">{{ baseName(d.file) }}</div>
      <template v-if="fileRow">
        <div class="fsb"><span>{{ t('Translated {n} %', { n: Math.round(pctOf(fileRow.done, fileRow.total)) }) }}</span><div class="bar"><i :style="{ width: pctOf(fileRow.done, fileRow.total) + '%' }"></i></div><span class="muted">{{ t('{n} left to translate', { n: fmtN(fileRow.todo) }) }}</span></div>
        <div class="fsb"><span>{{ t('Proofread {n} %', { n: Math.round(pctOf(fileRow.corr, fileRow.total)) }) }}</span><div class="bar two"><i :style="{ width: pctOf(fileRow.corr, fileRow.total) + '%' }"></i></div><span class="muted">{{ t('{n} left to check', { n: fmtN(Math.max(0, fileRow.total - fileRow.corr)) }) }}</span></div>
      </template>
      <template v-else><div class="fsb skel"><div class="skeleton"></div></div><div class="fsb skel"><div class="skeleton"></div></div></template>
      <div class="jumps">
        <template v-if="scenes === null">
          <button class="btn small" disabled>{{ t('◀ Scene') }}</button><select class="sel" disabled :aria-label="t('Loading…')"><option>{{ t('Loading…') }}</option></select><button class="btn small" disabled>{{ t('Scene ▶') }}</button>
        </template>
        <template v-else-if="scenes.length">
          <button class="btn small" :disabled="sceneIndex <= 0" :title="t('First line of the previous scene')" @click="goScene(sceneIndex - 1)">{{ t('◀ Scene') }}</button>
          <select class="sel" :value="sceneIndex" :title="t('List of the scenes of the file: translated / lines · proofread')" :aria-label="t('Scene {n} / {total}', { n: sceneIndex + 1, total: scenes.length })" @change="goScene(+$event.target.value)">
            <option v-for="(s, i) in scenes" :key="i" :value="i">{{ i + 1 }}. {{ s.name || '—' }} ({{ s.done }}/{{ s.count }} · ✓ {{ s.proofed }})</option>
          </select>
          <button class="btn small" :disabled="sceneIndex < 0 || sceneIndex >= scenes.length - 1" :title="t('First line of the next scene')" @click="goScene(sceneIndex + 1)">{{ t('Scene ▶') }}</button>
        </template>
        <button class="btn small" :title="t('Next untranslated line in the file')" @click="jump('todo')">{{ t('Next untranslated') }}</button>
        <button class="btn small" :title="t('Next translated but unchecked line')" @click="jump('review')">{{ t('Next unchecked') }}</button>
        <button class="btn small" :title="t('Next line that does not fit the game window')" @click="jump('over')">{{ t('Next one that does not fit') }}</button>
      </div>
    </div>

    <div class="head"><h4>{{ t('Original') }}</h4><span><button class="btn small ghost" :title="t('Copies the original text to the clipboard')" @click="copy(d.source ?? '')">{{ t('⧉ Original') }}</button>
      <button class="btn small ghost" :title="t('Copies the original and the translation one under the other')" @click="copy((d.source ?? '') + '\n' + text)">{{ t('⧉ Both') }}</button></span></div>
    <div class="box">{{ d.source ?? t('(source not found)') }}</div>

    <div class="head"><h4>{{ t('Translation') }} <span class="muted">{{ t('– edited directly in the file') }}</span></h4></div>
    <textarea ref="ta" v-model="text" class="edit" rows="4" :aria-label="t('Translation')" spellcheck="true" lang="cs" @keydown="key"></textarea>
    <div class="snips"><span class="muted">{{ t('Insert:') }}</span><button v-for="[label, fn, title] in SNIPS" :key="label" class="btn small snip" :title="t(title)" @click.prevent="fn">{{ label }}</button></div>
    <div v-if="missing.length" class="snips"><span class="muted">{{ t('Missing from the original:') }}</span>
      <button v-for="m in missing" :key="m" class="btn small snip" :title="t('Inserts a tag or variable from the original at the cursor')" @click.prevent="insert(m)">{{ m }}</button>
      <button v-if="missing.length > 1" class="btn small snip" :title="t('Inserts all missing ones at the cursor')" @click.prevent="insert(missing.join(''))">{{ t('all') }}</button></div>
    <div v-if="terms.length" class="snips"><span class="muted">{{ t('Glossary:') }}</span>
      <button v-for="x in terms" :key="x.en" class="btn small snip" :title="t('Inserts the translated term from the Glossary at the cursor')" @click.prevent="insert(x.cz)">{{ x.used ? '✓ ' : '' }}{{ x.en }} → {{ x.cz }}</button></div>
    <div class="actions">
      <button class="btn primary" :disabled="!canSave" :title="t('Writes the text to the .rpy file (a backup is made before writing)')" @click="save(false)">{{ t('Save to the file (Ctrl+Enter)') }}</button>
      <button class="btn" :disabled="!dirty" @click="text = d.target">{{ t('Restore the text') }}</button>
      <button class="btn" :title="t('Puts the original text into the field (handy as a base with the right tags already)')" @click="text = d.source ?? ''; ta && ta.focus()">{{ t('Original into the field') }}</button>
      <button class="btn ghost" :title="t('Copies the text of the translation field to the clipboard')" @click="copy(text)">{{ t('⧉ Translation') }}</button>
      <button v-if="mt.on" class="btn" :disabled="mt.busy" :title="t('Sends the source text of this line to the configured service and shows a suggestion. Nothing is written by itself.')" @click="suggest">{{ t('Machine suggestion') }}</button>
    </div>
    <div v-if="mt.busy" class="muted">{{ t('Asking the service…') }}</div>
    <div v-if="mt.error" class="notice danger" role="alert">{{ mt.error }}</div>
    <div v-if="mt.result" class="mt">
      <h4>{{ t('Machine suggestion ({provider})', { provider: mt.result.provider }) }}</h4>
      <div class="box">{{ mt.result.suggestion }}</div>
      <div v-for="(w, i) in mt.result.warnings" :key="'w' + i" class="issue"><span class="pill todo">{{ t('Careful') }}</span> {{ w }}</div>
      <div v-for="(x, i) in mt.result.issues" :key="'i' + i" class="issue"><span class="pill" :class="x.severity === 'error' ? 'danger' : 'todo'">{{ x.severity === 'error' ? t('Error') : t('Warning') }}</span> {{ x.message }}</div>
      <div class="actions"><button class="btn primary" @click="text = mt.result.suggestion; ta && ta.focus()">{{ t('Use the suggestion') }}</button><button class="btn" @click="insert(mt.result.suggestion)">{{ t('Insert') }}</button><button class="btn ghost" @click="mt.result = null">{{ t('Close') }}</button></div>
      <p class="muted small">{{ t('A machine suggestion is often literal and lacks the case, gender or tone of the character – always edit it.') }}</p>
    </div>
    <div v-if="live" class="issues">
      <h4>{{ t('Check of the edited text') }}</h4>
      <div v-if="live.literal_error" class="issue"><span class="pill danger">{{ t('Cannot save') }}</span> {{ live.literal_error }}</div>
      <div v-for="(x, i) in live.issues" :key="i" class="issue"><span class="pill" :class="x.severity === 'error' ? 'danger' : 'todo'">{{ x.severity === 'error' ? t('Error') : t('Warning') }}</span> {{ x.message }}</div>
      <div v-if="!live.literal_error && !live.issues.length" class="muted">{{ t('No findings.') }}</div>
    </div>
    <p class="muted small">{{ t('Write a new line as \\n, a quote as \\". Leave the tags {i}…{/i} and [variables] as in the original.') }}</p>

    <div class="tabs" role="tablist"><button v-for="[k, label] in TABS" :key="k" class="tab" :class="{ on: current === k, warn: k === 'check' && open.length }" role="tab" :aria-selected="current === k" @click="setTab(k)">{{ label }}</button></div>
    <div class="pane" role="tabpanel">
      <template v-if="current === 'check'">
        <div v-if="issues.length">
          <div v-for="x in issues" :key="x.code" class="issue" :class="{ ignored: x.ignored }">
            <span v-if="x.ignored" class="pill none">{{ t('ignored') }}</span><span v-else class="pill" :class="x.severity === 'error' ? 'danger' : x.severity === 'info' ? 'none' : 'todo'">{{ x.severity === 'error' ? t('Error') : x.severity === 'info' ? t('Info') : t('Warning') }}</span>
            {{ x.message }}<span v-if="x.ignored && x.ignore_note" class="muted"> – {{ x.ignore_note }}</span>
            <div class="actions">
              <button v-if="x.ignored" class="btn small ghost" @click="unignore(x)">{{ t('Stop ignoring') }}</button>
              <button v-else class="btn small ghost" :title="t('Hides this finding on this line (can be undone)')" @click="ignore(x)">{{ t('Ignore…') }}</button>
              <a v-if="x.fixable" class="btn small primary" href="/" :title="t('The fixes are in the Tools menu of the lines')">{{ x.kind === 'speaker' || !x.kind ? t('Fix from the original') : t('Fix…') }}</a>
            </div>
            <div v-if="x.ratio" class="muted">{{ t('Length: original {a}, translation {b} characters (ratio {r}). Decide yourself whether it is intended.', { a: x.source_len, b: x.target_len, r: x.ratio }) }}</div>
          </div>
        </div>
        <div v-else class="muted">{{ t('The line has no findings.') }}</div>
      </template>

      <template v-else-if="current === 'proof'">
        <div v-if="d.status !== 'todo'">
          <div class="actions">
            <button class="btn small" :class="{ primary: d.proof !== 'ok' }" @click="toggleProof">{{ d.proof === 'ok' ? t('Remove the mark (P)') : t('✓ Proofread (P)') }}</button>
            <button class="btn small" @click="toggleFix">{{ d.proof === 'fix' ? t('Clear “to fix” (F)') : t('⚑ To fix (F)') }}</button>
            <button class="btn small" :title="t('Marks all translated lines of this file from the start up to this line as proofread. Lines marked “to fix” do not change. It can be undone (Ctrl+Z).')" @click="proofUpto('ok')">{{ t('✓ Proofread everything up to here (Shift+P)') }}</button>
            <button class="btn small" :title="t('Removes the “Proofread” mark from all lines of this file from the start up to this line. Lines marked “to fix” stay. It can be undone (Ctrl+Z).')" @click="proofUpto('')">{{ t('↺ Remove the proofread mark up to here (Shift+U)') }}</button>
          </div>
          <span v-if="d.proof_date && d.proof === 'ok'" class="muted small">{{ d.proof_date }}</span>
          <textarea v-model="note" class="edit" rows="2" :placeholder="t('Note for the proofreading (what to fix)…')" :aria-label="t('Note for the proofreader')" @change="saveNote"></textarea>
        </div>
        <div v-else class="muted">{{ t('The line is not translated yet – proofreading is done after translating.') }}</div>
      </template>

      <template v-else-if="current === 'menu'">
        <div v-if="!data.menu || data.menu === 'loading'" class="muted">{{ t('Loading…') }}</div>
        <div v-else-if="data.menu.error || !data.menu.available" class="muted">{{ data.menu.error || data.menu.reason }}</div>
        <template v-else>
          <div class="muted mono small">{{ t('Game script: ') }}{{ data.menu.script }}</div>
          <h4>{{ data.menu.question.length ? t('Question') : t('Said before the menu') }}</h4>
          <div v-for="(x, i) in (data.menu.question.length ? data.menu.question : data.menu.before)" :key="'q' + i" class="mline" :class="{ cur: x.here }" @click="x.entry && emit('goto', { file: x.entry.file, line: x.entry.line })">
            <template v-if="x.entry"><b>{{ speaker(x.entry.who) }}</b> <span v-if="x.here" class="pill none">{{ t('this line') }}</span><div>{{ x.entry.status === 'todo' ? t('untranslated') : x.entry.target }}</div><div class="muted">{{ x.entry.source }}</div></template>
            <span v-else class="muted mono">{{ x.script }}</span>
          </div>
          <h4>{{ t('Choices') }}</h4>
          <div v-for="(o, i) in data.menu.options" :key="'o' + i" class="mopt" :class="{ cur: o.current }">
            <div class="mline" @click="o.entry && emit('goto', { file: o.entry.file, line: o.entry.line })">
              <span class="pill none">{{ t('Choice') }}</span> <span v-if="o.cond" class="pill none" :title="o.cond">{{ t('if {cond}', { cond: o.cond }) }}</span> <span v-if="o.current" class="pill none">{{ t('this line') }}</span>
              <div>{{ o.entry ? (o.entry.status === 'todo' ? t('untranslated') : o.entry.target) : t('(not found in the translation)') }}</div><div class="muted">{{ o.text }}</div>
              <div v-if="o.where" class="muted small">{{ t('The same text is used in an earlier menu ({ref}): all of them share one translation, and the line is listed there.', { ref: o.where }) }}</div>
            </div>
            <div v-if="!o.after.length && o.body && o.body.length" class="muted small"><div>{{ t('No dialogue follows; the script continues with:') }}</div><pre class="code">{{ o.body.join('\n') }}</pre></div>
            <div v-if="o.after.length" class="after"><div class="muted small">{{ t('then:') }}</div>
              <div v-for="(x, j) in o.after" :key="j" class="mline" @click="x.entry && emit('goto', { file: x.entry.file, line: x.entry.line })"><template v-if="x.entry"><b>{{ speaker(x.entry.who) }}</b><div>{{ x.entry.status === 'todo' ? t('untranslated') : x.entry.target }}</div><div class="muted">{{ x.entry.source }}</div></template><span v-else class="muted mono">{{ x.script }}</span></div></div>
          </div>
        </template>
      </template>

      <template v-else-if="current === 'scene'">
        <GamePreview :detail="d" :text="text" />
        <Fold id="scenectx">
        <template #title>{{ t('Scene around the line ') }}<span class="muted">{{ t('(original next to the translation)') }}</span></template>
        <div class="actions">
          <select class="sel" v-model.number="radius" :title="t('How many surrounding lines to show')" :aria-label="t('How many surrounding lines to show')"><option v-for="n in [3, 6, 10, 15]" :key="n" :value="n">{{ t('±{n} lines', { n }) }}</option></select>
          <div class="seg" role="group"><button v-for="[k, label] in [['table', t('Table')], ['bubbles', t('Bubbles')]]" :key="k" class="btn small" :class="{ primary: sceneMode === k }" :aria-pressed="sceneMode === k" @click="setMode(k)">{{ label }}</button></div>
          <button class="btn small" :title="t('Translates the untranslated lines around this one (±15) by machine. You review the suggestions before writing.')" @click="mtScene = true">{{ t('Translate the scene with a machine…') }}</button>
        </div>
        <div v-if="!data.scene || data.scene === 'loading'" class="muted">{{ t('Loading…') }}</div>
        <div v-else-if="data.scene.error" class="notice danger">{{ data.scene.error }}</div>
        <div v-else-if="sceneMode === 'bubbles'" class="bubbles">
          <div v-for="x in data.scene.items" :key="x.line" class="bubble" :class="{ cur: x.current }" :style="{ '--sc': speakerColor(x.who) }" @click="emit('goto', { file: d.file, line: x.line })">
            <div class="bwho"><b>{{ speaker(x.who) }}</b><span class="muted"> {{ x.line }}</span></div>
            <div class="btxt">{{ x.status === 'todo' ? t('untranslated') : shown(x.target) }}</div>
            <div class="bsrc muted">{{ shown(x.source) }}</div>
          </div>
        </div>
        <table v-else class="scene"><thead><tr><th>{{ t('Line') }}</th><th>{{ t('Speaker') }}</th><th>{{ t('Original') }}</th><th>{{ t('Translation') }}</th></tr></thead>
          <tbody><tr v-for="x in data.scene.items" :key="x.line" :class="{ cur: x.current }" @click="emit('goto', { file: d.file, line: x.line })">
            <td class="num muted">{{ x.line }}</td><td class="mono">{{ speaker(x.who) }}</td><td>{{ shown(x.source) }}</td><td>{{ x.status === 'todo' ? t('untranslated') : shown(x.target) }}</td></tr></tbody></table>
        </Fold>
        <MtDialog v-if="mtScene" :anchor="{ file: d.file, line: d.line }" :file="d.file" scope="scene" @close="mtScene = false" @done="mtScene = false; emit('updated', { reload: true })" @settings="mtScene = false" />
      </template>

      <template v-else-if="current === 'memory'"><Fold id="memory"><template #title>{{ t('Translation memory ') }}<span class="muted">{{ !data.memory || data.memory === 'loading' ? t('loading…') : data.memory.error ? t('(unavailable)') : (data.memory.exact.length + data.memory.similar.length) ? '(' + (data.memory.exact.length + data.memory.similar.length) + ')' : t('(nothing similar)') }}</span></template>
        <div v-if="!data.memory || data.memory === 'loading'" class="muted">{{ t('loading…') }}</div>
        <div v-else-if="data.memory.error" class="notice danger">{{ data.memory.error }}</div>
        <template v-else>
          <div v-if="!data.memory.exact.length && !data.memory.similar.length" class="muted">{{ t('Neither this nor a similar original is translated yet.') }}</div>
          <p v-else class="muted small">{{ t('Mind the case, gender and number: the text is inserted as it was translated before – adapt it to the new sentence.') }}</p>
          <div v-for="(g, i) in data.memory.exact" :key="'e' + i" class="memitem">
            <div><span class="pill ok">{{ t('same original') }}</span> ×{{ g.count }} <span v-if="g.vars_differ" class="pill todo">{{ t('⚠ different variables') }}</span></div>
            <div class="box">{{ g.target }}</div>
            <div class="actions"><button class="btn small" :title="t('Replaces the whole text in the field')" @click="text = g.target">{{ t('Apply') }}</button><button class="btn small" :title="t('Inserts the text at the cursor')" @click="insert(g.target)">{{ t('Insert') }}</button>
              <button v-if="g.lines && g.lines[0]" class="btn small ghost" @click="emit('goto', { file: g.lines[0].file, line: g.lines[0].line })">{{ t('Show the line') }}</button></div>
          </div>
          <div v-for="(x, i) in data.memory.similar" :key="'s' + i" class="memitem">
            <div><span class="pill none">{{ t('{n} % similar', { n: Math.round(x.ratio * 100) }) }}</span> <span v-if="x.vars_differ" class="pill todo">{{ t('⚠ different variables') }}</span></div>
            <div class="box">{{ x.target }}</div>
            <div class="muted small">{{ t('Original there: ') }}<template v-for="p in [diff(d.source || '', x.source)]" :key="x.source">{{ p.head }}<mark>{{ p.mid || '∅' }}</mark>{{ p.tail }}</template></div>
            <div class="actions"><button class="btn small" @click="text = x.target">{{ t('Apply') }}</button><button class="btn small" @click="insert(x.target)">{{ t('Insert') }}</button>
              <button v-if="x.file" class="btn small ghost" @click="emit('goto', { file: x.file, line: x.line })">{{ t('Show the line') }}</button></div>
          </div>
        </template>
      </Fold></template>

      <template v-else-if="current === 'history'"><Fold id="history"><template #title>{{ t('Text history ') }}<span class="muted">{{ !data.history || data.history === 'loading' ? t('loading…') : data.history.error ? t('(unavailable)') : data.history.count ? '(' + tn(data.history.count, '{n} change', '{n} changes') + ')' : t('(no recorded change)') }}</span></template>
        <div v-if="!data.history || data.history === 'loading'" class="muted">{{ t('loading…') }}</div>
        <div v-else-if="data.history.error" class="notice danger">{{ data.history.error }}</div>
        <template v-else>
          <div class="muted small">{{ data.history.count ? tn(data.history.count, '{n} change', '{n} changes') : t('(no recorded change)') }}</div>
          <div v-if="!data.history.versions.length" class="muted">{{ t('No change has been recorded for this line yet. Every edit in the app is recorded, and also a change made outside it that the app detects when the files are refreshed (F5).') }}</div>
          <div v-for="(v, i) in data.history.versions" :key="i" class="memitem">
            <div><span v-if="v.current" class="pill checked">{{ t('current') }}</span> <span class="muted small">{{ [v.time ? v.time.replace('T', ' ') : '', v.via].filter(Boolean).join(' · ') }}</span></div>
            <div class="box"><template v-if="v.current">{{ v.text }}</template><template v-else v-for="p in [diff(d.target, v.text)]" :key="v.text">{{ p.head }}<mark>{{ p.mid || '∅' }}</mark>{{ p.tail }}</template></div>
            <div v-if="!v.current" class="actions"><button class="btn small" :title="t('Puts this version into the field. It is written only after saving.')" @click="text = v.text; ta && ta.focus()">{{ t('Restore into the field') }}</button><button class="btn small ghost" @click="copy(v.text)">{{ t('⧉ Copy') }}</button></div>
          </div>
        </template>
      </Fold></template>

      <template v-else-if="current === 'team'">
        <div v-if="!data.notes || data.notes === 'loading'" class="muted">{{ t('Loading…') }}</div>
        <template v-else>
          <div v-if="!data.notes.notes.length" class="muted">{{ t('No note on this line yet.') }}</div>
          <div v-for="n in data.notes.notes" :key="n.id" class="memitem">
            <div><b>{{ n.name }}</b> <span class="muted small">{{ String(n.t).slice(0, 16).replace('T', ' ') }}</span> <span v-if="n.resolved" class="pill checked">{{ t('dealt with') }}</span></div>
            <div :style="n.resolved ? { opacity: .6, textDecoration: 'line-through' } : {}">{{ n.text }}</div>
            <div v-if="!n.resolved" class="actions"><button class="btn small" @click="resolveNote(n)">{{ t('Mark as dealt with') }}</button></div>
          </div>
          <textarea v-model="newNote" class="edit" rows="2" :placeholder="t('A note to the team about this line…')" :aria-label="t('A note to the team about this line…')"></textarea>
          <div class="actions"><button class="btn primary" :disabled="!newNote.trim()" @click="addNote">{{ t('Add the note') }}</button></div>
        </template>
      </template>

      <template v-else-if="current === 'script'">
        <div class="muted small">{{ t('Block: ') }}<b class="mono">{{ d.block }}</b><template v-if="d.who">{{ t('  ·  speaker: ') }}<b class="mono">{{ d.who }}</b></template><template v-if="d.src_line"> · {{ t('original on line {n}', { n: d.src_line }) }}</template></div>
        <template v-if="d.ref">
          <button v-if="!data.context" class="btn small ghost" @click="loadTab">{{ t('Show the context in the game script') }}</button>
          <div v-else-if="data.context === 'loading'" class="muted">{{ t('Loading…') }}</div>
          <div v-else-if="data.context.error || !data.context.available" class="muted">{{ data.context.error || data.context.reason }}</div>
          <template v-else><div class="muted mono small">{{ t('Game script: ') }}{{ data.context.ref }}</div><pre class="code"><div v-for="x in data.context.lines" :key="x.n" :class="{ hit: x.hit }">{{ String(x.n).padStart(5) }}  {{ x.text }}</div></pre></template>
        </template>
      </template>
    </div>

    <ApproveIdentical v-if="identical" :detail="d" @close="identical = false" @done="identical = false; emit('updated', { reload: true })" />
  </div>
</template>

<style scoped>
.ld { display: flex; flex-direction: column; gap: var(--s-2); }
.dhead { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.path { word-break: break-all; }
.actions { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.menu-wrap { position: relative; }
.menu-wrap .menu { position: absolute; z-index: 5; top: 100%; left: 0; margin-top: 4px; min-width: 240px; }
.menu-item { justify-content: space-between; cursor: pointer; }
.filestrip { border: 1px solid var(--line-soft); border-radius: var(--r-md); padding: var(--s-2) var(--s-3); display: flex; flex-direction: column; gap: var(--s-2); font-size: var(--fs-small); }
.fsname { font-weight: 500; }
.fsb.skel { grid-template-columns: 1fr; height: 21px; }
.fsb.skel .skeleton { height: 14px; }
.fsb { display: grid; grid-template-columns: 120px 1fr 120px; gap: var(--s-2); align-items: center; }
.jumps { display: flex; flex-wrap: wrap; gap: var(--s-1); align-items: center; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 220px; }
.head { display: flex; justify-content: space-between; align-items: center; margin-top: var(--s-2); }
.head h4, h4 { margin: var(--s-2) 0 var(--s-1); font-size: var(--fs-body); font-weight: 500; color: var(--text-2); }
.head h4 { margin: 0; }
.box { padding: var(--s-3); background: var(--bg-input); border: 1px solid var(--line); border-radius: var(--r-md); white-space: pre-wrap; font-size: var(--fs-text); line-height: var(--lh-normal); }
.edit { width: 100%; box-sizing: border-box; padding: var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; font-size: var(--fs-text); line-height: var(--lh-normal); resize: vertical; }
.edit:focus { outline: none; border-color: var(--accent-hi); }
.snips { display: flex; flex-wrap: wrap; gap: var(--s-1); align-items: center; font-size: var(--fs-small); }
.snip { font-family: var(--mono); }
.issues, .mt { display: flex; flex-direction: column; gap: var(--s-2); }
.issue { font-size: var(--fs-body); padding: var(--s-1) 0; }
.issue.ignored { opacity: .7; }
.tabs { display: flex; flex-wrap: wrap; border-bottom: 1px solid var(--line); margin-top: var(--s-3); }
.tab { all: unset; cursor: pointer; padding: var(--s-2) var(--s-3); font-size: var(--fs-small); color: var(--text-muted); border-bottom: 2px solid transparent; }
.tab:hover { color: var(--text); }
.tab.on { color: var(--text); border-bottom-color: var(--accent-hi); }
.tab.warn:not(.on) { color: var(--todo); }
.tab:focus-visible { box-shadow: var(--focus); }
.pane { padding-top: var(--s-2); }
.mline { padding: var(--s-2); border-radius: var(--r-md); cursor: pointer; font-size: var(--fs-body); }
.mline:hover { background: var(--bg-hover); }
.mline.cur, .mopt.cur > .mline { background: var(--bg-hover); box-shadow: inset 3px 0 0 var(--accent-hi); }
.after { margin-left: var(--s-4); }
.memitem { border: 1px solid var(--line-soft); border-radius: var(--r-md); padding: var(--s-2) var(--s-3); margin-bottom: var(--s-2); display: flex; flex-direction: column; gap: var(--s-2); }
.scene { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.scene th { text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-1) var(--s-2); }
.scene td { padding: var(--s-1) var(--s-2); border-top: 1px solid var(--line-soft); vertical-align: top; cursor: pointer; }
.scene tr:hover { background: var(--bg-hover); }
.scene tr.cur { background: var(--bg-hover); box-shadow: inset 3px 0 0 var(--accent-hi); }
.seg { display: inline-flex; gap: var(--s-1); }
.bubbles { display: flex; flex-direction: column; gap: var(--s-2); margin-top: var(--s-2); }
.bubble { border-left: 4px solid var(--sc, var(--line)); background: var(--bg-input); border-radius: 4px var(--r-lg) var(--r-lg) 4px; padding: var(--s-2) var(--s-3); cursor: pointer; }
.bubble:hover { background: var(--bg-hover); }
.bubble.cur { background: var(--bg-hover); box-shadow: inset 0 0 0 1px var(--accent-hi); }
.bubble .bwho { font-size: var(--fs-small); margin-bottom: 2px; }
.bubble .btxt { font-size: var(--fs-text); }
.bubble .bsrc { font-size: var(--fs-small); margin-top: 2px; }
.code { margin: var(--s-2) 0 0; padding: var(--s-3); background: var(--bg-code); border-radius: var(--r-md); font-family: var(--mono); font-size: var(--fs-small); overflow: auto; }
.code .hit { background: var(--accent-soft); }
mark { background: var(--accent-soft); color: var(--accent-text); border-radius: 3px; }
.small { font-size: var(--fs-small); }
</style>
