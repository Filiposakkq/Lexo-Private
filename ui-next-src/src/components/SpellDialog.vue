<script setup>
import { onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';

/* Spelling with a dictionary (Pro): the translated lines are checked against a Hunspell dictionary (.dic + .aff) that you add for the language of the project. The words it does
   not know are listed with how often they occur and with suggestions; a correction is planned first (the lines that would change) and written as one undoable step, a word you
   ignore is remembered in the project. Words of the original, of the glossary and tags or variables are never reported. */
const props = defineProps({ file: { type: String, default: '' } });
const emit = defineEmits(['close', 'done']);
const state = ref(null);
const scope = ref(props.file ? 'file' : 'all');
const path = ref('');
const items = ref([]);
const info = ref('');
const busy = ref(false);
const error = ref('');
const plans = ref({});            // word -> {to, total, items}
const changed = ref(false);

onMounted(loadState);
async function loadState() { try { state.value = await post('spell/state', {}); } catch (e) { error.value = e.message; } }
function close() { emit(changed.value ? 'done' : 'close'); }
async function add() {
  error.value = '';
  try { await post('spell/add', { path: path.value.trim() }); await loadState(); } catch (e) { error.value = e.message; }
}
async function pick() { try { const r = await post('carry/pick', {}); if (!r.cancelled) path.value = r.path; } catch (e) { error.value = e.message; } }
async function removeDict() { try { await post('spell/remove', {}); await loadState(); items.value = []; info.value = ''; } catch (e) { error.value = e.message; } }
async function check() {
  busy.value = true; info.value = t('Checking…'); items.value = []; plans.value = {}; error.value = '';
  try {
    const r = await post('spell/check', { file: scope.value === 'file' ? props.file : '', lines: null });
    items.value = r.items.map((it) => ({ ...it, to: (it.suggestions || [])[0] || '' }));
    info.value = items.value.length ? tn(items.value.length, '{n} unknown word.', '{n} unknown words.') + (r.truncated ? ' ' + t('Only the first ones are listed; correct or ignore them and check again.') : '') : t('No unknown words.');
  } catch (e) { info.value = ''; error.value = e.message; } finally { busy.value = false; }
}
function gone(it) { items.value = items.value.filter((x) => x !== it); info.value = items.value.length ? tn(items.value.length, '{n} unknown word.', '{n} unknown words.') : t('No unknown words.'); }
async function plan(it) {
  if (!it.to.trim()) { error.value = t('Enter the word to replace it with.'); return; }
  error.value = '';
  try { plans.value = { ...plans.value, [it.word]: await post('spell/replace', { word: it.word, to: it.to.trim() }) }; } catch (e) { error.value = e.message; }
}
async function replace(it) {
  const usable = plans.value[it.word].items.filter((x) => !x.approved);
  try {
    await post('mt/apply', { selections: usable.map((x) => ({ file: x.file, line: x.line, text: x.after })) });
    changed.value = true; gone(it);
  } catch (e) { error.value = e.message; }
}
async function ignore(it) { try { await post('spell/ignore', { words: [it.word] }); gone(it); } catch (e) { error.value = e.message; } }
</script>

<template>
  <Dialog :title="t('Spelling with a dictionary')" width="900px" @close="close">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!state && !error" class="skeleton" style="height: 80px"></div>
    <template v-else-if="state && !state.installed">
      <p>{{ t('Add a Hunspell dictionary for the language of this project ({language}): the .dic file and the .aff file with the same name. It is copied to the settings of the application and checked at once.', { language: state.language || '–' }) }}</p>
      <div class="line"><label class="field grow"><input v-model="path" :placeholder="t('Folder or .dic file of the dictionary')" @keydown.enter="add"></label><button class="btn" @click="pick">{{ t('Browse…') }}</button><button class="btn primary" @click="add">{{ t('Add the dictionary') }}</button></div>
    </template>
    <template v-else-if="state">
      <p class="muted">{{ t('Dictionary: {name}. Words of the original and of the glossary, tags and variables are not reported. A correction is written as one step that can be undone; approved lines are never overwritten.', { name: state.name || '–' }) }}</p>
      <div class="line">
        <select v-if="file" class="sel" v-model="scope"><option value="file">{{ t('File in the filter: {name}', { name: baseName(file) }) }}</option><option value="all">{{ t('All files') }}</option></select>
        <button class="btn primary" :disabled="busy" @click="check">{{ t('Check') }}</button><span class="grow"></span>
        <button class="btn small ghost" @click="removeDict">{{ t('Remove the dictionary') }}</button>
      </div>
      <div class="muted">{{ info }}</div>
      <div v-if="items.length" class="wrap"><table>
        <thead><tr><th>{{ t('Word') }}</th><th>{{ t('Times') }}</th><th>{{ t('Where') }}</th><th>{{ t('Replace with') }}</th><th></th></tr></thead>
        <tbody><tr v-for="it in items" :key="it.word">
          <td><b>{{ it.word }}</b></td><td class="num">{{ it.count }}</td>
          <td class="muted small">{{ it.places.map((p) => baseName(p.file) + ':' + p.line).join(', ') }}<template v-if="it.count > it.places.length"> …</template></td>
          <td>
            <input v-model="it.to" class="edit" :list="'sp_' + it.word" :placeholder="t('the correct word')" :aria-label="t('Replace with')"><datalist :id="'sp_' + it.word"><option v-for="s in it.suggestions || []" :key="s" :value="s"></option></datalist>
            <div v-if="plans[it.word]" class="plan">
              <div v-if="!plans[it.word].total" class="muted">{{ t('No line contains this word on its own.') }}</div>
              <template v-else>
                <div v-for="(x, i) in plans[it.word].items.slice(0, 4)" :key="i" class="muted small"><s>{{ x.before }}</s> → {{ x.after }}<i v-if="x.approved"> {{ t('(approved: stays)') }}</i></div>
                <div v-if="plans[it.word].total > 4" class="muted small">{{ tn(plans[it.word].total - 4, '… and {n} more line.', '… and {n} more lines.') }}</div>
                <button class="btn small primary" :disabled="!plans[it.word].items.filter((x) => !x.approved).length" @click="replace(it)">{{ tn(plans[it.word].items.filter((x) => !x.approved).length, 'Replace in {n} line', 'Replace in {n} lines') }}</button>
              </template>
            </div>
          </td>
          <td class="nowrap"><button class="btn small" @click="plan(it)">{{ t('Replace…') }}</button> <button class="btn small ghost" :title="t('Do not report this word again in this project')" @click="ignore(it)">{{ t('Ignore') }}</button></td>
        </tr></tbody></table></div>
    </template>
    <template #footer><button class="btn" @click="close">{{ t('Close') }}</button></template>
  </Dialog>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin: var(--s-2) 0; }
.grow { flex: 1; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.wrap { max-height: 48vh; overflow: auto; margin-top: var(--s-2); border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.edit { width: 170px; background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-1) var(--s-2); }
.plan { margin-top: var(--s-2); display: flex; flex-direction: column; gap: 2px; align-items: flex-start; }
.num { text-align: right; }
.nowrap { white-space: nowrap; }
.small { font-size: var(--fs-small); }
</style>
