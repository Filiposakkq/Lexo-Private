<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import { locale, t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import DemoNotice from './DemoNotice.vue';
import Dialog from './Dialog.vue';

/* The style check (Pro): the translated lines are sent, with the original, the profile of the speaker and the lines before, to the language model set up for the machine
   translation; it says what is wrong (the form of address, the gender forms, the tone, the meaning) and gives a corrected text. Lines with a remark are shown with the
   correction you can edit and tick; only the ticked ones are written (undoable). Approved lines are never overwritten. */
const props = defineProps({ picked: { type: Array, default: () => [] }, anchor: { type: Object, default: null }, file: { type: String, default: '' }, scope: { type: String, default: '' } });
const emit = defineEmits(['close', 'done', 'settings']);
const SCENE_R = 15, LIMIT = 200, BATCH = 5;
const cfg = ref(null);
const error = ref('');
const sc = ref('');
const target = ref({ items: [], chars: 0, total: 0 });
const counting = ref(false);
const running = ref(false);
const progress = ref('');
const results = ref([]);
const checked = ref(0);
const sel = ref(new Set());
let stop = false;
const KIND = () => ({ address: t('form of address'), gender: t('gender'), tone: t('tone'), meaning: t('meaning'), other: t('other') });
const opts = computed(() => {
  const o = [];
  if (props.picked.length) o.push(['picked', t('Selected lines ({n})', { n: props.picked.length })]);
  if (props.anchor) o.push(['scene', t('Scene around the open line (±{n})', { n: SCENE_R })]);
  if (props.file) o.push(['file', t('File in the filter: {name}', { name: baseName(props.file) })]);
  o.push(['all', t('All files')]);
  return o;
});
const ready = computed(() => cfg.value && cfg.value.provider === 'openai' && cfg.value.consent && cfg.value.has_key);
const rk = (r) => r.file + '\t' + r.line;
const flagged = (r) => (r.issues || []).some((i) => i.severity === 'error');
const bad = computed(() => results.value.filter((r) => !r.error && !r.ok));
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
    target.value = await post('style/targets', { file: sc.value === 'file' ? props.file : '', lines });
  } catch (e) { error.value = e.message; }
  counting.value = false;
}
async function run() {
  const todo = target.value.items.slice(0, LIMIT);
  running.value = true; stop = false; results.value = []; checked.value = 0; sel.value = new Set();
  let stopped = '';
  for (let i = 0; i < todo.length && !stop; i += BATCH) {
    progress.value = t('Checking… {done} / {total}', { done: Math.min(i + BATCH, todo.length), total: todo.length });
    let r;
    try { r = await post('style/batch', { items: todo.slice(i, i + BATCH) }); } catch (e) { stopped = e.message; break; }
    const s = new Set(sel.value);
    for (const x of r.results) { results.value.push(x); if (!x.error) checked.value += 1; if (!x.error && !x.ok && x.fix && !flagged(x)) s.add(rk(x)); }
    sel.value = s;
    if (r.stopped) { stopped = r.stopped; break; }
  }
  progress.value = stopped || (stop ? t('Stopped.') : t('Done – review the remarks and write the corrections you agree with.'));
  running.value = false; plan();
}
function toggle(r, on) { const s = new Set(sel.value); if (on) s.add(rk(r)); else s.delete(rk(r)); sel.value = s; }
async function write() {
  try {
    await post('mt/apply', { selections: results.value.filter((r) => !r.error && !r.ok && sel.value.has(rk(r))).map((r) => ({ file: r.file, line: r.line, text: r.fix })) });
    emit('done');
  } catch (e) { error.value = e.message; }
}
</script>

<template>
  <Dialog :title="cfg && !ready ? t('The style check needs a language model') : t('Check the style')" :width="ready ? '1000px' : '460px'" @close="emit('close')">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!cfg && !error" class="skeleton" style="height: 80px"></div>
    <p v-else-if="cfg && !ready">{{ t('Choose the OpenAI-compatible service, enter the key and give the consent to send text in {where}. DeepL and LibreTranslate are not language models and cannot check the style.', { where: t('Settings → Machine translation') }) }}</p>
    <template v-else-if="cfg">
      <p class="muted">{{ t('The original and the translation are sent to the configured language model, with the profile of the speaker if there is one; nothing is written until you tick and confirm. Approved lines are never overwritten.') }}</p>
      <div class="line"><label class="check">{{ t('What: ') }} <select class="sel" v-model="sc" @change="plan"><option v-for="[k, l] in opts" :key="k" :value="k">{{ l }}</option></select></label></div>
      <div class="muted"><template v-if="counting">{{ t('Counting…') }}</template>
        <template v-else>{{ t('{lines}, about {chars} characters will be sent to {provider}.', { lines: tn(target.total, '{n} translated line', '{n} translated lines'), chars: (target.chars || 0).toLocaleString(locale()), provider: cfg.provider }) }}</template></div>
      <DemoNotice :limited="target.limited" />
      <div v-if="target.items.length > LIMIT" class="notice danger">{{ t('At most {n} lines at once – the first {n} will be checked, run again for the rest.', { n: LIMIT }) }}</div>
      <div class="muted">{{ t('The service charges by the text sent – check the price with your provider.') }}</div>
      <div class="line">
        <button class="btn primary" :disabled="!target.items.length || running || counting" @click="run">{{ target.items.length ? tn(Math.min(target.items.length, LIMIT), 'Check {n} line', 'Check {n} lines') : t('Nothing to check') }}</button>
        <button v-if="running" class="btn" @click="stop = true">{{ t('Stop') }}</button><span class="muted">{{ progress }}</span>
      </div>
      <p v-if="results.length" class="muted">{{ t('{checked} lines checked, {n} with a remark.', { checked, n: bad.length }) }}</p>
      <template v-if="bad.length">
        <div class="line">
          <button class="btn small" @click="sel = new Set(bad.filter((r) => r.fix).map(rk))">{{ t('Select all') }}</button>
          <button class="btn small" :title="t('Unticks the lines where the validator found something in the correction')" @click="sel = new Set(bad.filter((r) => r.fix && !flagged(r)).map(rk))">{{ t('Only safe ones') }}</button>
          <button class="btn small" @click="sel = new Set()">{{ t('None') }}</button>
        </div>
        <div class="wrap"><table>
          <thead><tr><th></th><th>{{ t('Where') }}</th><th>{{ t('Original and translation') }}</th><th>{{ t('Remark') }}</th><th>{{ t('Correction (editable)') }}</th></tr></thead>
          <tbody><tr v-for="r in bad" :key="rk(r)">
            <td><input type="checkbox" :checked="sel.has(rk(r))" :disabled="!r.fix" :aria-label="baseName(r.file) + ':' + r.line" @change="toggle(r, $event.target.checked)"></td>
            <td class="nowrap">{{ baseName(r.file) }} <span class="mono muted">{{ r.line }}</span><div v-if="r.who" class="mono muted">{{ r.who }}</div></td>
            <td class="txt"><div>{{ r.source }}</div><div class="muted">{{ r.current }}</div></td>
            <td class="txt"><div v-for="(m, i) in r.remarks || []" :key="i"><span class="pill todo">{{ KIND()[m.kind] || m.kind }}</span> {{ m.explanation }}</div></td>
            <td class="txt"><input class="edit" :value="r.fix || r.current" :aria-label="t('Correction (editable)')" @input="r.fix = $event.target.value">
              <div v-if="flagged(r)"><span v-for="(x, i) in r.issues || []" :key="i" class="pill" :class="x.severity === 'error' ? 'danger' : 'todo'">{{ x.message || x.code }}</span></div></td>
          </tr></tbody></table></div>
      </template>
      <p v-if="errs.length" class="muted">{{ tn(errs.length, '{n} line failed', '{n} lines failed') }}: {{ errs[0].error }}</p>
    </template>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Close') }}</button>
      <button v-if="cfg && !ready" class="btn primary" @click="emit('settings')">{{ t('Open Settings') }}</button>
      <button v-else class="btn primary" :disabled="!sel.size || running" @click="write">{{ tn(sel.size, 'Write {n} correction', 'Write {n} corrections') }}</button>
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
.txt { max-width: 280px; word-break: break-word; }
.nowrap { white-space: nowrap; }
.edit { width: 100%; box-sizing: border-box; background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--s-1) var(--s-2); }
</style>
