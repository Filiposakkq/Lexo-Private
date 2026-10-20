<script setup>
import { onMounted, ref } from 'vue';
import { get } from '../api.js';
import { t } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';

/* Every line whose original contains a glossary term, and whether the translated term is used there. */
const props = defineProps({ code: { type: String, required: true } });
const emit = defineEmits(['close', 'show']);
const only = ref(false);
const data = ref(null);
const error = ref('');

async function load() {
  data.value = null; error.value = '';
  try { data.value = await get('glossary/occurrences', { code: props.code, missing: only.value ? '1' : '' }); } catch (e) { error.value = e.message; }
}
onMounted(load);
const state = (it) => (it.used === true ? [t('used'), 'ok'] : it.used === false ? [t('missing'), 'todo'] : [it.translated ? '—' : t('untranslated'), 'none']);
</script>

<template>
  <Dialog :title="t('Occurrences of the term')" width="1040px" @close="emit('close')">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-else-if="!data" class="skeleton" style="height: 120px"></div>
    <template v-else>
      <p><b>{{ code }}</b> → <b v-if="data.czech">{{ data.czech }}</b><span v-else class="muted">{{ t('(the translation is not filled in in the Glossary)') }}</span></p>
      <p class="muted">{{ t('Used {found}× · missing {missing}× · untranslated {untranslated}×. Words are compared by stem, so endings do not matter; short or heavily inflected forms may be checked badly.', { found: data.found, missing: data.missing, untranslated: data.untranslated }) }}</p>
      <label class="check"><input v-model="only" type="checkbox" @change="load"> {{ t('Only lines where the translated term is missing') }}</label>
      <div v-if="data.items.length" class="wrap"><table>
        <thead><tr><th>{{ t('State') }}</th><th>{{ t('Where') }}</th><th>{{ t('Original') }}</th><th>{{ t('Translation') }}</th><th></th></tr></thead>
        <tbody><tr v-for="it in data.items" :key="it.file + it.line">
          <td><span class="pill" :class="state(it)[1]">{{ state(it)[0] }}</span></td>
          <td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ t('line {n}', { n: it.line }) }}</span></td>
          <td class="txt">{{ it.source }}</td><td class="txt">{{ it.target }}</td>
          <td><button class="btn small" @click="emit('show', it)">{{ t('Show') }}</button></td>
        </tr></tbody></table></div>
      <div v-else class="empty muted"><b>{{ t('No occurrences') }}</b> {{ only ? t('The term is not missing in the translated lines.') : t('The term does not occur in the original.') }}</div>
      <p v-if="data.truncated" class="muted">{{ t('Showing the first 400 occurrences.') }}</p>
    </template>
    <template #footer><button class="btn primary" @click="emit('close')">{{ t('Close') }}</button></template>
  </Dialog>
</template>

<style scoped>
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); margin: var(--s-2) 0; }
.wrap { max-height: 50vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin-top: var(--s-2); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 300px; word-break: break-word; }
.nowrap { white-space: nowrap; }
.empty { padding: var(--s-5); text-align: center; }
</style>
