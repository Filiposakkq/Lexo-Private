<script setup>
import { computed, onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import Dialog from './Dialog.vue';

/* Import of the character names from the game's characters.rpy (the game files are only read). The ticked rows are written to the glossary; a name you already have that differs
   is overwritten only if you tick it yourself. One undoable step. */
const emit = defineEmits(['close', 'done']);
const path = ref('');
const data = ref(null);
const rows = ref([]);
const error = ref('');
const busy = ref(false);
const STATE = computed(() => ({ new: [t('New'), 'ok'], same: [t('Matching'), 'none'], conflict: [t('Different from the glossary'), 'danger'], dynamic: [t('Entered by the player'), 'none'] }));

function paint(d) { data.value = d; rows.value = d.items.map((i) => ({ ...i, on: i.state === 'new' })); }
async function load() {
  error.value = '';
  try { paint(await post('glossary/import-preview', { path: path.value })); path.value = data.value.path || path.value; } catch (e) { error.value = e.message; }
}
onMounted(load);
const count = computed(() => rows.value.filter((r) => r.on).length);
async function apply() {
  busy.value = true; error.value = '';
  try { await post('glossary/import-apply', { items: rows.value.filter((r) => r.on).map((r) => ({ code: r.code, name: r.name })) }); emit('done'); } catch (e) { error.value = e.message; busy.value = false; }
}
const shortName = (f) => f.split(/[\\/]/).slice(-3).join('/');
</script>

<template>
  <Dialog :title="t('Import character names from the game')" width="980px" @close="emit('close')">
    <p class="muted">{{ t('The scripts of the game that define characters (for example characters.rpy) are only read. The ticked rows are written to the glossary; names you already have in the glossary that differ are overwritten only if you tick them yourself.') }}</p>
    <div class="bar-row"><label class="field grow"><input v-model="path" type="text" :placeholder="t('game folder or a script file')" :aria-label="t('game folder or a script file')" @keydown.enter="load"></label><button class="btn" @click="load">{{ t('Load') }}</button></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <template v-if="data">
      <div v-if="!rows.length" class="empty muted"><b>{{ t('Nothing to import') }}</b> {{ data.message || t('There are no define … = Character("…") lines in the file.') }}</div>
      <template v-else>
        <div class="muted small">{{ t('Loaded files ({n}): ', { n: data.files.length }) }}<span class="mono">{{ data.files.map(shortName).join('  ·  ') }}</span></div>
        <div v-if="data.duplicates && data.duplicates.length" class="notice warn">{{ t("Codes defined several times (Ren'Py uses the last definition): ") }}<span class="mono">{{ data.duplicates.map((d) => `${d.code}: ${d.names.join(' → ')}`).join(';  ') }}</span></div>
        <div v-if="data.missing && data.missing.length" class="notice warn">{{ tn(data.missing.length, "The translation has {n} speaker code that the game scripts do not define (it may be defined elsewhere, or it is a typo): ", "The translation has {n} speaker codes that the game scripts do not define (they may be defined elsewhere, or it is a typo): ") }}<span class="mono">{{ data.missing.slice(0, 25).join(', ') + (data.missing.length > 25 ? ' …' : '') }}</span></div>
        <div class="wrap"><table>
          <thead><tr><th></th><th>{{ t('Code') }}</th><th>{{ t('Name in the script') }}</th><th>{{ t('In the glossary so far') }}</th><th>{{ t('File') }}</th><th>{{ t('State') }}</th></tr></thead>
          <tbody><tr v-for="r in rows" :key="r.code">
            <td><input v-model="r.on" type="checkbox" :disabled="r.state === 'dynamic' || r.state === 'same'" :title="r.state === 'conflict' ? t('Overwrites the name you have in the glossary') : ''" :aria-label="r.code"></td>
            <td class="mono">{{ r.code }}</td><td>{{ r.name || r.note }}</td><td class="muted">{{ r.current || '–' }}</td><td class="mono muted small">{{ r.source }}</td>
            <td><span class="pill" :class="STATE[r.state][1]">{{ STATE[r.state][0] }}</span><span v-if="!r.in_project" class="muted">{{ t('  (not in the translation)') }}</span></td>
          </tr></tbody></table></div>
      </template>
    </template>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Close') }}</button>
      <button class="btn primary" :disabled="!count || busy" @click="apply">{{ count ? t('Write to the glossary ({n})', { n: count }) : t('Write to the glossary') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.bar-row { display: flex; gap: var(--s-2); margin: var(--s-3) 0; }
.grow { flex: 1; }
.small { font-size: var(--fs-small); }
.wrap { max-height: 48vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin-top: var(--s-2); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.empty { padding: var(--s-5); text-align: center; }
.notice { margin-top: var(--s-2); }
</style>
