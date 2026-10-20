<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import GlossaryImport from '../components/GlossaryImport.vue';
import OccurrencesDialog from '../components/OccurrencesDialog.vue';
import ProfileDialog from '../components/ProfileDialog.vue';
import { t, tn } from '../i18n.js';
import { requireFeature } from '../pro.js';
import { loadNames, openLines } from '../project.js';
import Dialog from '../components/Dialog.vue';

/* Glossary: the speakers of the project (only abbreviations in the files, here they get a full name) and the terms of the game with their translation. A name is saved when you leave the
   field; the terms found in a line are checked and given to the machine translation. */
const emit = defineEmits(['go']);
const data = ref(null);
const error = ref('');
const message = ref('');
const f = ref({ filter: stored('filter', 'all'), q: '', word: false });
const mode = ref(stored('speaker', 'both'));
const occ = ref('');                 // the term whose occurrences are open
const profile = ref(null);           // { code, name }
const importing = ref(false);
const adding = ref(false);
const add = ref({ type: 'speaker', code: '', name: '', note: '' });
const addError = ref('');

function stored(key, def) { try { return localStorage.getItem('aside.glossary.' + key) || (key === 'speaker' ? localStorage.getItem('aside.speaker') : '') || def; } catch { return def; } }
function remember(key, v) { try { localStorage.setItem('aside.glossary.' + key, v); if (key === 'speaker') localStorage.setItem('aside.speaker', v); } catch { /* only for this page */ } }
const FILTERS = computed(() => [['all', t('All')], ['unnamed', t('Without a name')], ['speaker', t('Speaker')], ['term', t('Terms')]]);

async function load() {
  error.value = '';
  try { data.value = await get('glossary'); } catch (e) { error.value = e.message; }
}
onMounted(load);
const word = computed(() => {
  const q = f.value.q.trim().toLowerCase();
  return f.value.word && q ? new RegExp('(?<![\\p{L}\\p{N}_])' + q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '(?![\\p{L}\\p{N}_])', 'iu') : null;
});
const rows = computed(() => {
  if (!data.value) return [];
  const q = f.value.q.trim().toLowerCase(), flt = f.value.filter;
  const has = (text) => (word.value ? word.value.test(text || '') : String(text || '').toLowerCase().includes(q));
  return data.value.items.filter((i) => (!q || has(i.code) || has(i.name) || has(i.note))
    && (flt === 'all' || (flt === 'unnamed' && i.type === 'speaker' && i.in_project && !i.name) || (flt === 'speaker' && i.type === 'speaker') || (flt === 'term' && i.type === 'term')));
});

async function save(it, quiet) {
  try {
    data.value = await post('glossary/set', { items: [{ code: it.code, name: (it.name || '').trim(), type: it.type, note: (it.note || '').trim() }] });
    loadNames();
    if (!quiet) message.value = t('Saved ✓');
  } catch (e) { error.value = e.message; }
}
async function remove(it) {
  try { data.value = await post('glossary/delete', { codes: [it.code] }); loadNames(); message.value = t('Entry deleted. Undo: Ctrl+Z'); } catch (e) { error.value = e.message; }
}
function setFilter(k) { f.value.filter = k; remember('filter', k); }
function setMode(v) { mode.value = v; remember('speaker', v); message.value = t('Speaker display changed.'); }
function show(it) { openLines({ file: '', q: it.code, status: 'all' }); emit('go', 'lines'); }
function showLine(it) { occ.value = ''; openLines({ file: it.file, line: it.line, status: 'all' }); emit('go', 'lines'); }
async function openProfile(it) { if (await requireFeature('mt.profiles')) profile.value = { code: it.code, name: it.name }; }
async function addItem() {
  if (!add.value.code.trim()) { addError.value = t('Fill in the code or the term.'); return; }
  try {
    data.value = await post('glossary/set', { items: [{ code: add.value.code, name: add.value.name, type: add.value.type, note: add.value.note }] });
    loadNames(); adding.value = false; add.value = { type: 'speaker', code: '', name: '', note: '' }; addError.value = ''; message.value = t('Saved ✓');
  } catch (e) { addError.value = e.message; }
}
const stats = computed(() => (data.value ? t('{n} items · without a name: {unnamed}', { n: data.value.items.length, unnamed: data.value.unnamed }) : ''));
</script>

<template>
  <main class="screen">
    <p class="muted">{{ t('Glossary of names. Speakers in the files are only abbreviations (mc, de, ji…); here you give them the full name. ') }}{{ t('You switch the display in the app (Lines, detail, surroundings of a line) on the right: code / full name / both. A name is saved automatically when you leave the field. The abbreviations are found in the files by themselves – nothing is made up.') }}</p>
    <div class="toolbar">
      <span class="label-inline">{{ t('Type') }}</span>
      <div class="seg" role="group"><button v-for="[k, l] in FILTERS" :key="k" class="btn small" :class="{ primary: f.filter === k }" :aria-pressed="f.filter === k" @click="setFilter(k)">{{ l }}</button></div>
      <label class="field search"><input v-model="f.q" type="search" :placeholder="t('Search code or name… (Ctrl+F)')" :aria-label="t('Search')"></label>
      <label class="check"><input v-model="f.word" type="checkbox"> {{ t('Whole word') }}</label>
      <select class="sel" :value="mode" :title="t('Speaker')" :aria-label="t('Speaker')" @change="setMode($event.target.value)"><option value="both">{{ t('Code and name') }}</option><option value="code">{{ t('Code only') }}</option><option value="name">{{ t('Name only') }}</option></select>
    </div>
    <div class="toolbar">
      <button class="btn" @click="adding = true">{{ t('+ Add an item (term / name)') }}</button>
      <button class="btn" :title="t('Loads the character names from the game\'s characters.rpy')" @click="importing = true">{{ t('Import from characters.rpy…') }}</button>
      <span class="grow"></span><span class="muted small">{{ stats }}</span>
    </div>
    <div v-if="message" class="notice" role="status">{{ message }}<span class="act" role="button" tabindex="0" @click="message = ''" @keydown.enter="message = ''">{{ t('Close') }}</span></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>

    <div v-if="!data && !error" class="skeleton" style="height: 120px"></div>
    <div v-else-if="data && !rows.length" class="empty muted"><b>{{ t('Nothing to show') }}</b> {{ f.filter === 'all' ? t('No speakers were found in the project.') : t('Change the filter or the search text.') }}</div>
    <div v-else-if="data" class="tablewrap"><table class="grid">
      <thead><tr><th>{{ t('Type') }}</th><th>{{ t('Code / term') }}</th><th>{{ t('Full name / translation') }}</th><th>{{ t('Note') }}</th><th>{{ t('Line count') }}</th><th>{{ t('Example') }}</th><th></th></tr></thead>
      <tbody><tr v-for="it in rows" :key="it.code">
        <td><span class="pill" :class="it.type === 'speaker' ? 'none' : 'checked'">{{ it.type === 'speaker' ? t('speaker') : t('term') }}</span></td>
        <td class="mono">{{ it.code }}</td>
        <td><input v-model="it.name" class="cell" type="text" :placeholder="it.type === 'speaker' ? t('e.g. the full name of the character') : t('translation / name')" :aria-label="t('Full name / translation') + ' ' + it.code" @change="save(it)"></td>
        <td><input v-model="it.note" class="cell" type="text" :aria-label="t('Note') + ' ' + it.code" @change="save(it)"></td>
        <td class="num">{{ it.count || '–' }}</td>
        <td class="txt muted">{{ it.example }}</td>
        <td class="nowrap">
          <button v-if="it.type === 'term'" class="btn small" :title="t('Where the term occurs in the original and whether it is translated according to the glossary')" @click="occ = it.code">{{ t('Occurrences') }}</button>
          <button v-if="it.type === 'speaker'" class="btn small" :title="t('Pro: gender, way of speaking and form of address of this character, used by the batch machine translation')" @click="openProfile(it)">{{ t('Profile…') }}</button>
          <button v-if="it.count" class="btn small" :title="t('Show the lines of this speaker')" @click="show(it)">{{ t('Lines') }}</button>
          <button v-if="it.name || it.note" class="btn small ghost" :title="t('Delete the entry from the glossary')" @click="remove(it)">{{ t('Delete') }}</button>
        </td>
      </tr></tbody></table></div>

    <OccurrencesDialog v-if="occ" :code="occ" @close="occ = ''" @show="showLine" />
    <ProfileDialog v-if="profile" :code="profile.code" :name="profile.name" @close="profile = null" @saved="profile = null; message = t('Saved ✓'); load()" />
    <GlossaryImport v-if="importing" @close="importing = false" @done="importing = false; message = t('Names imported ✓  (Ctrl+Z undoes)'); loadNames(); load()" />
    <Dialog v-if="adding" :title="t('New glossary item')" width="520px" @close="adding = false">
      <div class="form">
        <label>{{ t('Type') }}<select v-model="add.type" class="sel"><option value="speaker">{{ t('Speaker (abbreviation of a character)') }}</option><option value="term">{{ t('Term / name / place') }}</option></select></label>
        <label>{{ t('Code / term') }}<input v-model="add.code" class="cell" type="text" :placeholder="t('code or term (e.g. de, Delta Iota Kappa)')" @keydown.enter="addItem"></label>
        <label>{{ t('Name') }}<input v-model="add.name" class="cell" type="text" :placeholder="t('full name / translation')" @keydown.enter="addItem"></label>
        <label>{{ t('Note') }}<input v-model="add.note" class="cell" type="text" :placeholder="t('note (optional)')" @keydown.enter="addItem"></label>
        <div v-if="addError" class="notice danger" role="alert">{{ addError }}</div>
      </div>
      <template #footer><button class="btn" @click="adding = false">{{ t('Cancel') }}</button><button class="btn primary" @click="addItem">{{ t('Add') }}</button></template>
    </Dialog>
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; box-sizing: border-box; }
.toolbar { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.label-inline { font-size: var(--fs-small); color: var(--text-muted); }
.seg { display: flex; flex-wrap: wrap; gap: var(--s-1); }
.grow { flex: 1; }
.search { width: 280px; }
.check { display: flex; gap: var(--s-1); align-items: center; font-size: var(--fs-small); color: var(--text-muted); white-space: nowrap; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.small { font-size: var(--fs-small); }
.act { margin-left: auto; cursor: pointer; text-decoration: underline; }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line); }
.grid td { padding: 6px var(--s-3); border-bottom: 1px solid var(--line-soft); vertical-align: middle; }
.cell { width: 100%; box-sizing: border-box; height: var(--h-control-sm); padding: 0 var(--s-2); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; }
.cell:focus { outline: none; border-color: var(--accent-hi); }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.txt { max-width: 320px; }
.nowrap { white-space: nowrap; }
.nowrap .btn { margin-left: var(--s-1); }
.empty { padding: var(--s-5); text-align: center; }
.form { display: flex; flex-direction: column; gap: var(--s-3); }
.form label { display: flex; flex-direction: column; gap: var(--s-1); font-size: var(--fs-small); color: var(--text-muted); }
</style>
