<script setup>
import { computed, onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';

/* Approve all lines whose original and translation are the same as those of the open line: this line, its file or the whole project. A bulk change is confirmed twice. */
const props = defineProps({ detail: { type: Object, required: true } });
const emit = defineEmits(['close', 'done']);
const scope = ref('file');
const sameSpeaker = ref(true);
const data = ref(null);
const sel = ref(new Set());
const armed = ref(false);
const error = ref('');
const busy = ref(false);
let timer = null;

async function preview() {
  data.value = null; error.value = '';
  try {
    data.value = await post('approve/preview', { file: props.detail.file, line: props.detail.line, scope: scope.value, same_speaker: sameSpeaker.value });
    sel.value = new Set(data.value.items.map((it, i) => (it.selected ? i : -1)).filter((i) => i >= 0));
    armed.value = false;
  } catch (e) { error.value = e.message; }
}
onMounted(preview);
const files = computed(() => new Set([...sel.value].map((i) => data.value.items[i].file)));
const broad = computed(() => sel.value.size > 1);
function toggle(i, on) { const s = new Set(sel.value); if (on) s.add(i); else s.delete(i); sel.value = s; armed.value = false; }
function setScope(k) { scope.value = k; preview(); }
async function apply() {
  if (broad.value && !armed.value) { armed.value = true; clearTimeout(timer); timer = setTimeout(() => { armed.value = false; }, 8000); return; }
  busy.value = true;
  try {
    await post('approve/apply', { selections: [...sel.value].map((i) => ({ file: data.value.items[i].file, key: data.value.items[i].key })), on: true });
    emit('done');
  } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Approve identical lines')" width="980px" @close="emit('close')">
    <p class="muted">{{ t('Only lines whose original and translation are the same as those of the selected line are approved. Lines with another speaker or looking like untranslated English are unticked by default. Every approved line is written separately (by block), so it can be undone with Ctrl+Z.') }}</p>
    <div class="bar-row">
      <div class="seg" role="group"><button v-for="[k, l] in [['occurrence', t('This line only')], ['file', t('This whole file')], ['project', t('The whole project')]]" :key="k" class="chip" :class="{ on: scope === k }" @click="setScope(k)">{{ l }}</button></div>
      <label class="check"><input type="checkbox" v-model="sameSpeaker" @change="preview"> {{ t('Preselect the same speaker only') }}</label>
    </div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!data && !error" class="skeleton" style="height: 120px"></div>
    <template v-if="data">
      <div class="muted">{{ t('Selected text: ') }}<b>{{ data.reference.text }}</b></div>
      <div class="wrap"><table>
        <thead><tr><th></th><th>{{ t('File') }}</th><th>{{ t('Line') }}</th><th>{{ t('Speaker') }}</th><th>{{ t('Text') }}</th><th>{{ t('Note') }}</th></tr></thead>
        <tbody><tr v-for="(it, i) in data.items" :key="i">
          <td><input type="checkbox" :checked="sel.has(i)" :disabled="!it.selectable" :aria-label="baseName(it.file) + ':' + it.line" @change="toggle(i, $event.target.checked)"></td>
          <td>{{ baseName(it.file) }}</td><td class="num">{{ it.line }}</td><td>{{ it.who || '—' }}</td><td>{{ it.text }}</td>
          <td><span v-if="it.is_reference" class="pill checked">{{ t('selected line') }}</span> <span v-if="it.approved" class="pill approved">{{ t('approved') }}</span> <span class="muted">{{ it.notes.join('; ') }}</span></td>
        </tr></tbody></table></div>
    </template>
    <template #footer>
      <span class="muted grow">{{ data ? tn(files.size, 'Found {total} identical · selected {n} in {f} file', 'Found {total} identical · selected {n} in {f} files', { total: data.counts.total, n: sel.size, f: files.size }) + (broad ? t(' · a bulk change must be confirmed twice') : '') : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :class="{ armed }" :disabled="!sel.size || busy" @click="apply">{{ armed ? t('Confirm: approve {n} lines in {f} files', { n: sel.size, f: files.size }) : t('Approve selected ({n})', { n: sel.size }) }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.bar-row { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; margin: var(--s-3) 0; }
.seg { display: flex; gap: var(--s-1); }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); }
.wrap { max-height: 46vh; overflow: auto; margin-top: var(--s-2); border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.grow { flex: 1; font-size: var(--fs-small); }
.armed { background: var(--danger); border-color: var(--danger); color: var(--bg-app); }
</style>
