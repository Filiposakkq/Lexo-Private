<script setup>
import { computed, onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import DemoNotice from './DemoNotice.vue';
import Dialog from './Dialog.vue';

/* Safe fixes: speaker / attributes from the original, double spaces, typo hints. Every line is ticked or unticked by you; doubtful ones start unticked.
   Variables and tags are never offered (their order may legitimately differ). */
const props = defineProps({ items: { type: Array, default: () => [] }, file: { type: String, default: '' }, kinds: { type: Array, default: () => ['speaker', 'spaces', 'typo'] } });
const emit = defineEmits(['close', 'done']);
const data = ref(null);
const sel = ref(new Set());
const only = ref(props.kinds.length === 1 ? props.kinds[0] : '');
const error = ref('');
const busy = ref(false);
const key = (it) => `${it.file}|${it.line}|${it.kind}`;
const KIND = () => ({ speaker: t('Speaker / attributes'), spaces: t('Double spaces'), typo: t('Typos') });
const HELP = () => ({
  speaker: t('The speaker, attributes and the code after the text are copied from the original (the line with #). The translation text is not changed.'),
  spaces: t('Several spaces in a row between words become one (only when the original does not have them either).'),
  typo: t('Suggestions from the dictionary of your own translation. Short and uncertain words are unticked because they may be a false alarm (byt / být). Tick only what you really want.'),
});
const visible = computed(() => (data.value ? data.value.items.filter((it) => !only.value || it.kind === only.value).slice(0, 600) : []));
const total = computed(() => (data.value ? data.value.items.filter((it) => !only.value || it.kind === only.value).length : 0));

onMounted(async () => {
  try {
    data.value = await post('fix/preview', { items: props.items, file: props.file, kinds: props.kinds });
    sel.value = new Set(data.value.items.filter((it) => it.default && !it.skip).map(key));
  } catch (e) { error.value = e.message; }
});
function toggle(it, on) { const s = new Set(sel.value); if (on) s.add(key(it)); else s.delete(key(it)); sel.value = s; }
function selectAll() { const s = new Set(sel.value); visible.value.forEach((it) => { if (!it.skip) s.add(key(it)); }); sel.value = s; }
function selectNone() { const s = new Set(sel.value); visible.value.forEach((it) => s.delete(key(it))); sel.value = s; }
function recommended() { const s = new Set(sel.value); visible.value.forEach((it) => { if (it.default && !it.skip) s.add(key(it)); else s.delete(key(it)); }); sel.value = s; }
const diff = (a, b) => {
  const n = Math.min(a.length, b.length);
  let i = 0; while (i < n && a[i] === b[i]) i++;
  let j = 0; while (j < n - i && a[a.length - 1 - j] === b[b.length - 1 - j]) j++;
  const part = (s) => ({ head: s.slice(0, i), mid: s.slice(i, s.length - j) || '∅', tail: s.slice(s.length - j) });
  return { before: part(a), after: part(b) };
};
async function apply() {
  busy.value = true;
  try {
    await post('fix/apply', { selections: data.value.items.filter((it) => sel.value.has(key(it))).map((it) => ({ file: it.file, line: it.line, kind: it.kind })) });
    emit('done');
  } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Fix')" width="1000px" @close="emit('close')">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-else-if="!data" class="skeleton" style="height: 100px"></div>
    <div v-else-if="!data.total" class="empty muted">{{ t('Nothing to fix.') }}</div>
    <template v-else>
      <DemoNotice :limited="data.limited" />
      <p v-if="only">{{ HELP()[only] }}</p>
      <p v-else class="muted">{{ t('Choose what to fix. A backup of the file is made in the subfolder {folder} of the local project data before writing; the change can be undone with the button “Undo the last file change”. The files must be saved in the editor.', { folder: 'backups' }) }}</p>
      <div class="line">
        <div class="seg" role="group">
          <button class="chip" :class="{ on: only === '' }" @click="only = ''">{{ t('All ({n})', { n: data.total }) }}</button>
          <button v-for="k in Object.keys(data.counts).filter((x) => data.counts[x])" :key="k" class="chip" :class="{ on: only === k }" @click="only = k">{{ KIND()[k] }} ({{ data.counts[k] }})</button>
        </div>
        <span class="grow"></span>
        <button class="btn small" @click="selectAll">{{ t('Select all') }}</button><button class="btn small" @click="selectNone">{{ t('None') }}</button>
        <button class="btn small" :title="t('Only what is safe (uncertain suggestions stay unticked)')" @click="recommended">{{ t('Only recommended') }}</button>
      </div>
      <p v-if="visible.length < total" class="muted small">{{ t('Showing the first 600 lines.') }}</p>
      <div class="wrap"><table>
        <thead><tr><th></th><th>{{ t('Where') }}</th><th>{{ t('Now') }}</th><th>{{ t('After the fix') }}</th></tr></thead>
        <tbody><tr v-for="it in visible" :key="key(it)">
          <td><input type="checkbox" :checked="sel.has(key(it))" :disabled="!!it.skip" :aria-label="baseName(it.file) + ':' + it.line" @change="toggle(it, $event.target.checked)"></td>
          <td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ t('line {n}', { n: it.line }) }}</span><div v-if="!only"><span class="pill none">{{ KIND()[it.kind] }}</span></div></td>
          <td class="txt" :class="{ mono: it.code }"><template v-if="it.code">{{ it.before }}</template><template v-else v-for="p in [diff(it.before, it.after).before]" :key="'b'">{{ p.head }}<mark class="del">{{ p.mid }}</mark>{{ p.tail }}</template></td>
          <td class="txt" :class="{ mono: it.code }"><span v-if="it.skip" class="muted">{{ it.skip }}</span><template v-else-if="it.code">{{ it.after }}</template><template v-else v-for="p in [diff(it.before, it.after).after]" :key="'a'">{{ p.head }}<mark>{{ p.mid }}</mark>{{ p.tail }}</template>
            <div v-if="!it.code && it.note" class="muted small">{{ it.note }}</div></td>
        </tr></tbody></table></div>
    </template>
    <template #footer>
      <span class="muted grow small">{{ data ? t('Selected {n} of {total} applicable', { n: sel.size, total: data.fixable }) : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :disabled="!sel.size || busy" @click="apply">{{ tn(sel.size, 'Fix {n} line', 'Fix {n} lines') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin: var(--s-2) 0; }
.seg { display: flex; gap: var(--s-1); flex-wrap: wrap; }
.wrap { max-height: 46vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 340px; word-break: break-word; }
.nowrap { white-space: nowrap; }
mark { background: var(--accent-soft); color: var(--accent-text); border-radius: 3px; }
mark.del { background: var(--danger-bg); color: var(--danger); }
.empty { padding: var(--s-5); text-align: center; }
.grow { flex: 1; }
.small { font-size: var(--fs-small); }
</style>
