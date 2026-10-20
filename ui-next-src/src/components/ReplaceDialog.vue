<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import { requireFeature } from '../pro.js';
import Dialog from './Dialog.vue';
import ScopePicker from './ScopePicker.vue';

/* Find and replace in the translated texts of all or chosen files, with a preview you tick line by line. */
const props = defineProps({ preselected: { type: Array, default: () => [] } });
const emit = defineEmits(['close', 'done']);
const find = ref('');
const repl = ref('');
const mode = ref('plain');
const matchCase = ref(false);
const protect = ref(true);
const scope = ref(props.preselected.length ? 'chosen' : 'all');
const chosen = ref([...props.preselected]);
const data = ref(null);
const sel = ref(new Set());
const armed = ref(false);
const error = ref('');
const busy = ref(false);
let seq = 0, timer = null, armTimer = null;

const MODES = computed(() => [['plain', t('Text')], ['word', t('Whole word')], ['stem', t('By stem (experimental)')], ['regex', t('Regular expression')]]);
const HELP = computed(() => ({
  plain: t('Finds text anywhere (also inside words).'), word: t('Finds whole words only.'),
  stem: t('Also finds other forms of the word by its stem (dikmince → dikminci) and keeps their ending. Works well only for words with the same declension – always check the preview.'),
  regex: t('A Python regular expression; in the replacement you can use \\1, \\2…'),
}[mode.value]));
const request = () => ({ find: find.value, replace: repl.value, mode: mode.value, case: matchCase.value, protect: protect.value, files: scope.value === 'chosen' ? chosen.value : [] });
const shown = computed(() => (data.value ? data.value.items.slice(0, 500) : []));

async function pickMode(k) { if (k === 'regex' && !await requireFeature('replace.regex')) return; mode.value = k; }
function schedule() { clearTimeout(timer); timer = setTimeout(preview, 350); }
async function preview() {
  const my = ++seq;
  error.value = '';
  if (!find.value || (scope.value === 'chosen' && !chosen.value.length)) { data.value = null; sel.value = new Set(); return; }
  try {
    const r = await post('replace/preview', request());
    if (my !== seq) return;
    data.value = r; sel.value = new Set(r.items.slice(0, 500).map((it, i) => (it.skip ? -1 : i)).filter((i) => i >= 0)); armed.value = false;
  } catch (e) { if (my === seq) { data.value = null; error.value = e.message; } }
}
watch([find, repl, mode, matchCase, protect, scope, chosen], schedule, { deep: true });
onBeforeUnmount(() => { clearTimeout(timer); clearTimeout(armTimer); });
const diff = (a, b) => {
  const n = Math.min(a.length, b.length);
  let i = 0; while (i < n && a[i] === b[i]) i++;
  let j = 0; while (j < n - i && a[a.length - 1 - j] === b[b.length - 1 - j]) j++;
  const part = (s) => ({ head: s.slice(0, i), mid: s.slice(i, s.length - j) || '∅', tail: s.slice(s.length - j) });
  return { before: part(a), after: part(b) };
};
function toggle(i, on) { const s = new Set(sel.value); if (on) s.add(i); else s.delete(i); sel.value = s; armed.value = false; }
async function apply() {
  if (sel.value.size > 1 && !armed.value) { armed.value = true; clearTimeout(armTimer); armTimer = setTimeout(() => { armed.value = false; }, 8000); return; }
  busy.value = true;
  try {
    await post('replace/apply', { ...request(), selections: [...sel.value].map((i) => ({ file: shown.value[i].file, line: shown.value[i].line })) });
    emit('done');
  } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Find and replace')" width="1040px" @close="emit('close')">
    <p class="muted">{{ t('Searches and replaces in already translated text. Tags {i}, variables [name] and \\n sequences are never changed by default. A backup is made before writing; the change can be undone in one step.') }}</p>
    <div class="two"><label class="field"><input v-model="find" :placeholder="t('What to find')" autofocus></label><label class="field"><input v-model="repl" :placeholder="t('Replace with (may be empty)')"></label></div>
    <div class="line">
      <div class="seg" role="group"><button v-for="[k, l] in MODES" :key="k" class="chip" :class="{ on: mode === k }" @click="pickMode(k)">{{ l }}</button></div>
      <label class="check"><input type="checkbox" v-model="matchCase"> {{ t('Match case') }}</label>
      <label class="check"><input type="checkbox" v-model="protect"> {{ t('Leave tags and variables alone') }}</label>
    </div>
    <div class="muted small">{{ HELP }}</div>
    <ScopePicker v-model:scope="scope" v-model:chosen="chosen" />
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!find" class="empty muted"><b>{{ t('Enter the text to find') }}</b> {{ t('The preview shows at once; nothing is written until you confirm.') }}</div>
    <div v-else-if="scope === 'chosen' && !chosen.length" class="empty muted"><b>{{ t('Choose files') }}</b> {{ t('Tick at least one file, or switch to “All files”.') }}</div>
    <div v-else-if="data && !data.items.length" class="empty muted"><b>{{ t('Nothing found') }}</b> {{ t('The text is not in the translated lines (untranslated lines are not searched).') }}</div>
    <template v-else-if="data">
      <div class="line"><span class="muted small">{{ t('{lines}, {matches}', { lines: tn(data.total, '{n} line', '{n} lines'), matches: tn(data.matches, '{n} match', '{n} matches') }) }}<template v-if="data.truncated"> {{ t('(only the first 2000 shown)') }}</template></span>
        <button class="btn small" @click="sel = new Set(shown.map((it, i) => (it.skip ? -1 : i)).filter((i) => i >= 0))">{{ t('Select all') }}</button><button class="btn small" @click="sel = new Set()">{{ t('None') }}</button></div>
      <p v-if="data.items.length > 500" class="muted small">{{ t('Showing the first 500; only the shown ones are replaced.') }}</p>
      <div class="wrap"><table>
        <thead><tr><th></th><th>{{ t('Where') }}</th><th>{{ t('Now') }}</th><th>{{ t('After the replacement') }}</th><th>×</th></tr></thead>
        <tbody><tr v-for="(it, i) in shown" :key="i">
          <td><input type="checkbox" :checked="sel.has(i)" :disabled="!!it.skip" :aria-label="baseName(it.file) + ':' + it.line" @change="toggle(i, $event.target.checked)"></td>
          <td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ t('line {n}', { n: it.line }) }}</span></td>
          <td class="txt"><template v-for="p in [diff(it.before, it.after).before]" :key="'b'">{{ p.head }}<mark class="del">{{ p.mid }}</mark>{{ p.tail }}</template></td>
          <td class="txt"><span v-if="it.skip" class="muted">{{ it.skip }}</span><template v-else v-for="p in [diff(it.before, it.after).after]" :key="'a'">{{ p.head }}<mark>{{ p.mid }}</mark>{{ p.tail }}</template></td>
          <td class="num">{{ it.count }}</td>
        </tr></tbody></table></div>
    </template>
    <template #footer>
      <span class="muted grow small">{{ data ? t('Selected {n} of {total}', { n: sel.size, total: data.applicable }) + (sel.size > 1 ? ' · ' + t('a bulk change must be confirmed twice') : '') : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Close') }}</button>
      <button class="btn primary" :class="{ armed }" :disabled="!sel.size || busy" @click="apply">{{ armed ? tn(sel.size, 'Confirm: replace in {n} line', 'Confirm: replace in {n} lines') : t('Replace ({n})', { n: sel.size }) }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.two { display: grid; grid-template-columns: 1fr 1fr; gap: var(--s-2); margin-bottom: var(--s-2); }
.line { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; margin: var(--s-2) 0; }
.seg { display: flex; gap: var(--s-1); }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); }
.wrap { max-height: 40vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 330px; word-break: break-word; }
.nowrap { white-space: nowrap; }
.num { text-align: right; }
mark { background: var(--accent-soft); color: var(--accent-text); border-radius: 3px; }
mark.del { background: var(--danger-bg); color: var(--danger); }
.empty { padding: var(--s-5); text-align: center; }
.grow { flex: 1; }
.small { font-size: var(--fs-small); }
.armed { background: var(--danger); border-color: var(--danger); color: var(--bg-app); }
</style>
