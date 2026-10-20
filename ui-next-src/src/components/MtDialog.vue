<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import { locale, t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import DemoNotice from './DemoNotice.vue';
import Dialog from './Dialog.vue';

/* Machine translation of several lines. Suggestions come in small batches (progress, Stop), are shown in a table you can edit and tick, and only the ticked ones are
   written, as one undoable step. Approved lines are never overwritten. */
const props = defineProps({ picked: { type: Array, default: () => [] }, anchor: { type: Object, default: null }, file: { type: String, default: '' }, scope: { type: String, default: '' } });
const emit = defineEmits(['close', 'done', 'settings']);
const SCENE_R = 15, LIMIT = 500;
const cfg = ref(null);
const error = ref('');
const sc = ref('');
const onlyTodo = ref(true);
const target = ref({ items: [], chars: 0, total: 0 });
const counting = ref(false);
const running = ref(false);
const progress = ref('');
const results = ref([]);
const sel = ref(new Set());
let stop = false;

const opts = computed(() => {
  const o = [];
  if (props.picked.length) o.push(['picked', t('Selected lines ({n})', { n: props.picked.length })]);
  if (props.anchor) o.push(['scene', t('Scene around the open line (±{n})', { n: SCENE_R })]);
  if (props.file) o.push(['file', t('File in the filter: {name}', { name: baseName(props.file) })]);
  o.push(['all', t('All files')]);
  return o;
});
const ready = computed(() => cfg.value && cfg.value.provider && cfg.value.consent && cfg.value.has_key);
const rk = (r) => r.file + '\t' + r.line;
const flagged = (r) => (r.issues || []).some((i) => i.severity === 'error') || (r.warnings || []).length > 0;
const ok = computed(() => results.value.filter((r) => !r.error));
const errs = computed(() => results.value.filter((r) => r.error));

onMounted(async () => {
  try {
    cfg.value = (await get('settings')).mt || {};
    sc.value = props.scope && opts.value.some(([k]) => k === props.scope) ? props.scope : opts.value[0][0];
    if (ready.value) plan();
  } catch (e) { error.value = e.message; }
});
async function plan() {
  counting.value = true; error.value = '';
  try {
    let lines = null;
    if (sc.value === 'picked') lines = props.picked;
    else if (sc.value === 'scene') { const r = await get('entry/scene', { file: props.anchor.file, line: props.anchor.line, radius: SCENE_R }); lines = r.items.map((x) => ({ file: props.anchor.file, line: x.line })); }
    target.value = await post('mt/targets', { file: sc.value === 'file' ? props.file : '', lines, only_todo: onlyTodo.value });
  } catch (e) { error.value = e.message; }
  counting.value = false;
}
async function run() {
  const todo = target.value.items.slice(0, LIMIT);
  running.value = true; stop = false; results.value = []; sel.value = new Set();
  let stopped = '';
  for (let i = 0; i < todo.length && !stop; i += 10) {
    progress.value = t('Translating… {done} / {total}', { done: Math.min(i + 10, todo.length), total: todo.length });
    let r;
    try { r = await post('mt/batch', { items: todo.slice(i, i + 10) }); } catch (e) { stopped = e.message; break; }
    const s = new Set(sel.value);
    for (const x of r.results) { results.value.push(x); if (!x.error && !flagged(x)) s.add(rk(x)); }
    sel.value = s;
    if (r.stopped) { stopped = r.stopped; break; }
  }
  progress.value = stopped || (stop ? t('Stopped.') : t('Done – review the suggestions and write the selected ones.'));
  running.value = false; plan();
}
function toggle(r, on) { const s = new Set(sel.value); if (on) s.add(rk(r)); else s.delete(rk(r)); sel.value = s; }
async function write() {
  try {
    await post('mt/apply', { selections: results.value.filter((r) => !r.error && sel.value.has(rk(r))).map((r) => ({ file: r.file, line: r.line, text: r.suggestion })) });
    emit('done');
  } catch (e) { error.value = e.message; }
}
</script>

<template>
  <Dialog :title="cfg && !ready ? t('Machine translation is not set up') : t('Translate with a machine')" :width="ready ? '980px' : '460px'" @close="emit('close')">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!cfg && !error" class="skeleton" style="height: 80px"></div>
    <p v-else-if="cfg && !ready">{{ t('Set the service, the API key and the consent to send text in {where}. The key stays on your computer only and text is sent only after your consent.', { where: t('Settings → Machine translation') }) }}</p>
    <template v-else-if="cfg">
      <p class="muted">{{ t('The source text is sent to the configured service; nothing is written until you tick and confirm. Approved lines are never overwritten.') }}</p>
      <div class="line">
        <label class="check">{{ t('What: ') }} <select class="sel" v-model="sc" @change="plan"><option v-for="[k, l] in opts" :key="k" :value="k">{{ l }}</option></select></label>
        <label class="check"><input type="checkbox" v-model="onlyTodo" @change="plan"> {{ t('only untranslated') }}</label>
      </div>
      <div class="muted">
        <template v-if="counting">{{ t('Counting…') }}</template>
        <template v-else>{{ t('{lines}, about {chars} characters will be sent to {provider}.', { lines: tn(target.total, '{n} line', '{n} lines'), chars: target.chars.toLocaleString(locale()), provider: cfg.provider }) }}</template>
      </div>
      <DemoNotice :limited="target.limited" />
      <div v-if="target.total > LIMIT" class="notice danger">{{ t('At most {n} lines at once – the first {n} will be translated, run again for the rest.', { n: LIMIT }) }}</div>
      <div class="muted">{{ t('The service charges by the text sent – check the price with your provider.') }}</div>
      <div class="line">
        <button class="btn primary" :disabled="!target.total || running || counting" @click="run">{{ target.total ? tn(Math.min(target.total, LIMIT), 'Translate {n} line', 'Translate {n} lines') : t('Nothing to translate') }}</button>
        <button v-if="running" class="btn" @click="stop = true">{{ t('Stop') }}</button><span class="muted">{{ progress }}</span>
      </div>
      <template v-if="ok.length">
        <div class="line">
          <button class="btn small" @click="sel = new Set(ok.map(rk))">{{ t('Select all') }}</button>
          <button class="btn small" :title="t('Unticks the lines where the validator found something')" @click="sel = new Set(ok.filter((r) => !flagged(r)).map(rk))">{{ t('Only without findings') }}</button>
          <button class="btn small" @click="sel = new Set()">{{ t('None') }}</button>
        </div>
        <div class="wrap"><table>
          <thead><tr><th></th><th>{{ t('Where') }}</th><th>{{ t('Original') }}</th><th>{{ t('Suggestion (editable)') }}</th></tr></thead>
          <tbody><tr v-for="r in ok" :key="rk(r)">
            <td><input type="checkbox" :checked="sel.has(rk(r))" :aria-label="baseName(r.file) + ':' + r.line" @change="toggle(r, $event.target.checked)"></td>
            <td class="nowrap">{{ baseName(r.file) }} <span class="mono muted">{{ r.line }}</span><div v-if="r.who" class="mono muted">{{ r.who }}</div></td>
            <td class="txt">{{ r.source }}</td>
            <td class="txt"><input v-model="r.suggestion" class="edit" :aria-label="t('Suggestion (editable)')">
              <div v-if="flagged(r)"><span v-for="(w, i) in r.warnings || []" :key="'w' + i" class="pill todo">{{ w }}</span> <span v-for="(x, i) in r.issues || []" :key="'i' + i" class="pill" :class="x.severity === 'error' ? 'danger' : 'todo'">{{ x.message || x.code }}</span></div></td>
          </tr></tbody></table></div>
      </template>
      <p v-if="errs.length" class="muted">{{ tn(errs.length, '{n} line failed', '{n} lines failed') }}: {{ errs[0].error }}</p>
    </template>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Close') }}</button>
      <button v-if="cfg && !ready" class="btn primary" @click="emit('settings')">{{ t('Open Settings') }}</button>
      <button v-else class="btn primary" :disabled="!sel.size || running" @click="write">{{ tn(sel.size, 'Write {n} line', 'Write {n} lines') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; margin: var(--s-2) 0; }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.wrap { max-height: 44vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 380px; word-break: break-word; }
.nowrap { white-space: nowrap; }
.edit { width: 100%; box-sizing: border-box; background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-1) var(--s-2); }
</style>
