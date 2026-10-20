<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';
import ScopePicker from './ScopePicker.vue';

/* Approve by rule: lines that need no translation because they are the same in the target language (symbols only, interjections, names). */
const props = defineProps({ preselected: { type: Array, default: () => [] } });
const emit = defineEmits(['close', 'done']);
const rules = ref({ symbols: true, interjections: true, names: true });
const scope = ref(props.preselected.length ? 'chosen' : 'all');
const chosen = ref([...props.preselected]);
const data = ref(null);
const sel = ref(new Set());
const armed = ref(false);
const error = ref('');
const busy = ref(false);
let timer = null, armTimer = null;
const RULE = () => ({ symbols: t('symbols only'), interjections: t('interjections'), names: t('name') });

async function load() {
  error.value = '';
  if (scope.value === 'chosen' && !chosen.value.length) { data.value = null; return; }
  try {
    data.value = await post('approve/rules', { rules: Object.keys(rules.value).filter((k) => rules.value[k]), files: scope.value === 'chosen' ? chosen.value : [] });
    sel.value = new Set(data.value.items.slice(0, 500).map((it, i) => (it.selected ? i : -1)).filter((i) => i >= 0)); armed.value = false;
  } catch (e) { data.value = null; error.value = e.message; }
}
onMounted(load);
watch([rules, scope, chosen], () => { clearTimeout(timer); timer = setTimeout(load, 300); }, { deep: true });
onBeforeUnmount(() => { clearTimeout(timer); clearTimeout(armTimer); });
function toggle(i, on) { const s = new Set(sel.value); if (on) s.add(i); else s.delete(i); sel.value = s; armed.value = false; }
async function apply() {
  if (sel.value.size > 1 && !armed.value) { armed.value = true; clearTimeout(armTimer); armTimer = setTimeout(() => { armed.value = false; }, 8000); return; }
  busy.value = true;
  try {
    await post('approve/apply', { selections: [...sel.value].map((i) => ({ file: data.value.items[i].file, key: data.value.items[i].key })), on: true });
    emit('done');
  } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Approve by rule')" width="960px" @close="emit('close')">
    <ScopePicker v-model:scope="scope" v-model:chosen="chosen" :label="t('Where to search:')" />
    <p class="muted">{{ t('Lines that need no translation because they are the same in the target language. Nothing changes until you confirm. An approval can be undone with Ctrl+Z. Names are taken from the Glossary – the more names you have there, the more lines the rule finds.') }}</p>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="scope === 'chosen' && !chosen.length" class="empty muted"><b>{{ t('Choose files') }}</b> {{ t('Tick at least one file, or switch to “All files”.') }}</div>
    <template v-else-if="data">
      <label v-for="(label, k) in data.rules" :key="k" class="check"><input type="checkbox" v-model="rules[k]"> {{ label }} <span class="muted">({{ data.counts.by_rule[k] || 0 }})</span></label>
      <p v-if="data.items.length > 500" class="muted small">{{ t('Showing the first 500 of {total}; only the shown ones are approved.', { total: data.items.length }) }}</p>
      <div v-if="data.items.length" class="wrap"><table>
        <thead><tr><th></th><th>{{ t('Rule') }}</th><th>{{ t('Where') }}</th><th>{{ t('Speaker') }}</th><th>{{ t('Text') }}</th><th></th></tr></thead>
        <tbody><tr v-for="(it, i) in data.items.slice(0, 500)" :key="i">
          <td><input type="checkbox" :checked="sel.has(i)" :disabled="!it.selectable" :aria-label="baseName(it.file) + ':' + it.line" @change="toggle(i, $event.target.checked)"></td>
          <td><span class="pill none">{{ RULE()[it.rule] }}</span></td><td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ t('line {n}', { n: it.line }) }}</span></td>
          <td>{{ it.who || '—' }}</td><td class="txt">{{ it.text }}</td><td class="muted">{{ it.notes.join('; ') }}</td>
        </tr></tbody></table></div>
      <div v-else class="empty muted"><b>{{ t('Nothing found') }}</b> {{ t('No untranslated line matches the selected rules.') }}</div>
    </template>
    <div v-else-if="!error" class="skeleton" style="height: 100px"></div>
    <template #footer>
      <span class="muted grow small">{{ data ? t('Found {total} · selected {n}', { total: data.counts.total, n: sel.size }) + (sel.size > 1 ? ' · ' + t('a bulk change must be confirmed twice') : '') : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :class="{ armed }" :disabled="!sel.size || busy" @click="apply">{{ armed ? tn(sel.size, 'Confirm: approve {n} line', 'Confirm: approve {n} lines') : t('Approve selected ({n})', { n: sel.size }) }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); padding: 2px 0; }
.wrap { max-height: 44vh; overflow: auto; margin-top: var(--s-2); border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 300px; word-break: break-word; }
.nowrap { white-space: nowrap; }
.empty { padding: var(--s-5); text-align: center; }
.grow { flex: 1; }
.small { font-size: var(--fs-small); }
.armed { background: var(--danger); border-color: var(--danger); color: var(--bg-app); }
</style>
