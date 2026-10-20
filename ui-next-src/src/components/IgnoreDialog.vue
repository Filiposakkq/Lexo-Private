<script setup>
import { onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import Dialog from './Dialog.vue';

/* Ignoring findings on single lines. An ignore is bound to the current translation text: when the text changes, the finding shows up again. Ignored findings stay listed
   (Check → Ignored findings) and can be restored. */
const props = defineProps({ items: { type: Array, required: true } });      // [{ file, line, code, severity, message }]
const emit = defineEmits(['close', 'done']);
const note = ref('');
const all = ref(true);
const error = ref('');
const busy = ref(false);
const field = ref(null);
const sevLabel = (s) => (s === 'error' ? t('Error') : s === 'info' ? t('Info') : t('Warning'));
onMounted(() => field.value?.focus());

async function ignore() {
  busy.value = true; error.value = '';
  const chosen = props.items.length > 1 && !all.value ? props.items.slice(0, 1) : props.items;
  try {
    const r = await post('ignore/set', { items: chosen.map((x) => ({ file: x.file, line: x.line, code: x.code })), on: true, note: note.value });
    emit('done', r.changed);
  } catch (e) { error.value = e.message; busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Ignore the finding')" width="520px" @close="emit('close')">
    <p class="muted">{{ t('The finding disappears from the lists, counts and line highlighting. It stays in the list {where}, where you can restore it with one click. If you change the translation of the line, the finding is shown again.', { where: t('Check → Ignored') }) }}</p>
    <div v-for="(x, i) in items" :key="i" class="issue"><span class="pill" :class="x.severity === 'error' ? 'danger' : x.severity === 'info' ? 'none' : 'todo'">{{ sevLabel(x.severity) }}</span> {{ x.message || x.code }}</div>
    <label v-if="items.length > 1" class="check"><input v-model="all" type="checkbox"> {{ tn(items.length, 'Ignore all {n} finding on this line', 'Ignore all {n} findings on this line') }}</label>
    <input ref="field" v-model="note" class="note" type="text" :placeholder="t('Why is it ignored? (optional, you will see it in the list of ignored findings)')" :aria-label="t('Note')" @keydown.enter="ignore">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :disabled="busy" @click="ignore">{{ t('Ignore') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.issue { padding: var(--s-1) 0; font-size: var(--fs-body); }
.check { display: flex; gap: var(--s-2); align-items: center; margin-top: var(--s-2); font-size: var(--fs-body); }
.note { width: 100%; box-sizing: border-box; margin-top: var(--s-3); height: var(--h-control); padding: 0 var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; }
.note:focus { outline: none; border-color: var(--accent-hi); }
</style>
