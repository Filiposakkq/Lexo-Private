<script setup>
import { computed, onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import DemoNotice from './DemoNotice.vue';
import Dialog from './Dialog.vue';

/* Carry the translation of the previous version of the game over to the open project (Pro), or fill untranslated lines from similar translations of this project (`own`):
   choose the old folder, look at what was found, tick, write. Lines with the very same original are ticked; lines whose original changed a little ("near") are ticked only
   when they are almost the same and have the same variables. */
const props = defineProps({ own: { type: Boolean, default: false } });
const emit = defineEmits(['close', 'done']);
const path = ref('');
const data = ref(null);
const sel = ref(new Set());
const mark = ref(true);
const error = ref('');
const busy = ref(false);
const searching = ref(false);
const route = props.own ? { preview: 'memory/fill-preview', apply: 'memory/fill-apply' } : { preview: 'carry/preview', apply: 'carry/apply' };
const key = (it) => `${it.file}|${it.line}`;
const recommended = (it) => it.kind === 'exact' || (it.ratio >= 0.9 && !it.tags_differ);
const visible = computed(() => (data.value ? data.value.items.slice(0, 600) : []));

async function find() {
  if (!props.own && !path.value.trim()) { error.value = t('Enter the path to the folder.'); return; }
  searching.value = true; error.value = ''; data.value = null;
  try {
    data.value = await post(route.preview, props.own ? {} : { old: path.value.trim() });
    sel.value = new Set(data.value.items.filter(recommended).map(key));
  } catch (e) { error.value = e.message; }
  searching.value = false;
}
onMounted(() => { if (props.own) find(); });
async function pick() { try { const r = await post('carry/pick', {}); if (!r.cancelled) { path.value = r.path; find(); } } catch (e) { error.value = e.message; } }
function toggle(it, on) { const s = new Set(sel.value); if (on) s.add(key(it)); else s.delete(key(it)); sel.value = s; }
const diff = (a, b) => {
  const n = Math.min(a.length, b.length);
  let i = 0; while (i < n && a[i] === b[i]) i++;
  let j = 0; while (j < n - i && a[a.length - 1 - j] === b[b.length - 1 - j]) j++;
  return { head: b.slice(0, i), mid: b.slice(i, b.length - j) || '∅', tail: b.slice(b.length - j) };
};
async function apply() {
  busy.value = true;
  try {
    await post(route.apply, { ...(props.own ? {} : { old: path.value.trim() }), selections: data.value.items.filter((it) => sel.value.has(key(it))).map((it) => ({ file: it.file, line: it.line })), mark: mark.value });
    emit('done');
  } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
</script>

<template>
  <Dialog :title="own ? t('Fill from similar translations') : t('Carry the translation over from the previous version')" width="1000px" @close="emit('close')">
    <p class="muted">{{ own ? t('Untranslated lines get the translation of a similar original of the same speaker from this project. Nothing is written until you confirm; the change can be undone.') : t('The open project is the NEW version of the game. Choose the folder of the OLD, translated version: the translation of every line whose original is the same (or almost the same) is looked up there. Nothing is written until you confirm; the change can be undone.') }}</p>
    <div v-if="!own" class="line"><label class="field grow"><input v-model="path" :placeholder="t('Folder of the previous (translated) version')" @keydown.enter="find"></label>
      <button class="btn" @click="pick">{{ t('Browse…') }}</button><button class="btn primary" :disabled="searching" @click="find">{{ t('Find') }}</button></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <p v-if="searching" class="muted">{{ t('Looking for translations…') }}</p>
    <p v-else-if="data && !data.items.length">{{ t('Nothing could be carried over from that folder.') }}</p>
    <template v-else-if="data">
      <p>{{ t('Found {exact} identical and {near} changed lines of {total}; {none} have no match.', { exact: data.counts.exact, near: data.counts.near, total: data.counts.total, none: data.counts.none }) }}</p>
      <DemoNotice :limited="data.limited" />
      <div class="line">
        <button class="btn small" @click="sel = new Set(data.items.map(key))">{{ t('Select all') }}</button><button class="btn small" @click="sel = new Set()">{{ t('None') }}</button>
        <button class="btn small" @click="sel = new Set(data.items.filter(recommended).map(key))">{{ t('Only recommended') }}</button>
      </div>
      <label class="check"><input type="checkbox" v-model="mark"> {{ own ? t('Mark the filled lines as “to fix”, so that they can be checked in the list') : t('Mark the lines whose original changed as “to fix”, so that they can be checked in the list') }}</label>
      <p v-if="data.items.length > 600" class="muted small">{{ t('Showing the first 600 lines.') }}</p>
      <div class="wrap"><table>
        <thead><tr><th></th><th>{{ t('Where') }}</th><th>{{ t('Original (the old one below if it changed)') }}</th><th>{{ t('Translation from the previous version') }}</th></tr></thead>
        <tbody><tr v-for="it in visible" :key="key(it)">
          <td><input type="checkbox" :checked="sel.has(key(it))" :aria-label="baseName(it.file) + ':' + it.line" @change="toggle(it, $event.target.checked)"></td>
          <td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ t('line {n}', { n: it.line }) }}</span>
            <div><span v-if="it.kind === 'exact'" class="pill checked">{{ t('same') }}</span><span v-else class="pill todo">{{ t('changed {pct} %', { pct: Math.round(it.ratio * 100) }) }}</span> <span v-if="it.tags_differ" class="pill todo">{{ t('other variables') }}</span></div></td>
          <td class="txt"><template v-if="it.kind === 'near'" v-for="p in [diff(it.old_source, it.source)]" :key="'d'">{{ p.head }}<mark>{{ p.mid }}</mark>{{ p.tail }}<div class="muted small">{{ it.old_source }}</div></template><template v-else>{{ it.source }}</template></td>
          <td class="txt">{{ it.target }}</td>
        </tr></tbody></table></div>
    </template>
    <template #footer>
      <span class="muted grow small">{{ data ? t('Selected {n} of {total}', { n: sel.size, total: data.items.length }) : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :disabled="!sel.size || busy" @click="apply">{{ tn(sel.size, 'Carry over {n} line', 'Carry over {n} lines') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin: var(--s-2) 0; }
.grow { flex: 1; }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); margin: var(--s-2) 0; }
.wrap { max-height: 44vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 340px; word-break: break-word; }
.nowrap { white-space: nowrap; }
mark { background: var(--accent-soft); color: var(--accent-text); border-radius: 3px; }
.small { font-size: var(--fs-small); }
</style>
