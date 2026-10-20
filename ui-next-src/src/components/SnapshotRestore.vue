<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';

/* Restore files from a backup. The files you tick are overwritten; the current state is saved first as another backup, so a restore can be undone too. Two clicks: the second confirms. */
const props = defineProps({ name: { type: String, required: true } });
const emit = defineEmits(['close', 'done']);
const data = ref(null);
const sel = ref(new Set());
const armed = ref(false);
const error = ref('');
const busy = ref(false);
let timer = null;
const STATE = computed(() => ({ changed: [t('differs from the backup'), 'todo'], missing: [t('missing in the folder'), 'danger'], same: [t('same'), 'none'] }));

onMounted(async () => {
  try {
    data.value = await get('snapshots/files', { name: props.name });
    sel.value = new Set(data.value.files.filter((f) => f.state !== 'same').map((f) => f.file));
  } catch (e) { error.value = e.message; }
});
onBeforeUnmount(() => clearTimeout(timer));
function toggle(f, on) { const s = new Set(sel.value); if (on) s.add(f.file); else s.delete(f.file); sel.value = s; armed.value = false; }
async function restore() {
  if (!armed.value) { armed.value = true; clearTimeout(timer); timer = setTimeout(() => { armed.value = false; }, 8000); return; }
  busy.value = true; error.value = '';
  try { const r = await post('snapshots/restore', { name: props.name, files: [...sel.value] }); emit('done', r.restored.length); } catch (e) { error.value = e.message; busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Restore from a backup')" width="780px" @close="emit('close')">
    <p>{{ t('The selected files are overwritten with the version from the backup. What is new in them since the backup is lost from these files, but another backup (kind “before a restore”) is saved first, so even a restore can be undone. Files that are not in the backup stay unchanged.') }}</p>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!data && !error" class="skeleton" style="height: 100px"></div>
    <div v-if="data" class="wrap"><table>
      <thead><tr><th></th><th>{{ t('File') }}</th><th>{{ t('State') }}</th></tr></thead>
      <tbody><tr v-for="f in data.files" :key="f.file"><td><input type="checkbox" :checked="sel.has(f.file)" :aria-label="f.file" @change="toggle(f, $event.target.checked)"></td>
        <td>{{ baseName(f.file) }}</td><td><span class="pill" :class="STATE[f.state][1]">{{ STATE[f.state][0] }}</span></td></tr></tbody></table></div>
    <template #footer>
      <span class="muted grow">{{ data ? t('Selected {n} of {total}', { n: sel.size, total: data.files.length }) : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :class="{ armed }" :disabled="!sel.size || busy" @click="restore">{{ armed ? tn(sel.size, 'Confirm: overwrite {n} file', 'Confirm: overwrite {n} files') : tn(sel.size, 'Restore {n} file', 'Restore {n} files') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.wrap { max-height: 46vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin-top: var(--s-2); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); }
.grow { flex: 1; font-size: var(--fs-small); }
.armed { background: var(--danger); border-color: var(--danger); color: var(--bg-app); }
</style>
