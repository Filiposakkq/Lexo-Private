<script setup>
import { onBeforeUnmount, ref } from 'vue';
import { t } from '../i18n.js';
import { ACTIONS, comboOf, currentMap, isChanged, keysOf, saveKeys } from '../keys.js';
import Dialog from './Dialog.vue';

/* Settings → Shortcuts: change the keys of an action. The press of the new key is captured; a key that another action has asks before it is taken over. Saved at once. */
const capturing = ref(null);          // { action, add }
const clash = ref(null);              // { combo, other, action, add }
const notice = ref('');
const restoreAll = ref(false);
let off = null;

function start(action, add) {
  notice.value = ''; capturing.value = { action, add };
  const onKey = (e) => {
    const c = comboOf(e);
    if (!c) return;
    e.preventDefault(); e.stopPropagation();
    stop();
    if (c !== 'Escape') assign(action, add, c);
  };
  document.addEventListener('keydown', onKey, true);
  off = () => document.removeEventListener('keydown', onKey, true);
}
function stop() { if (off) off(); off = null; capturing.value = null; }
onBeforeUnmount(stop);

function assign(a, add, c) {
  const bare = !/^(Ctrl|Alt)\+/.test(c) && !/^F\d+$/.test(c);
  if (a.typing && bare) { notice.value = t('This action also works while typing, so it needs Ctrl or Alt (or an F1–F12 key).'); return; }
  const other = ACTIONS.find((o) => o.id !== a.id && keysOf(o).includes(c));
  if (other) { clash.value = { combo: c, other, action: a, add }; return; }
  put(a, add, c);
}
function put(a, add, c, takeFrom) {
  const map = currentMap();
  if (takeFrom) map[takeFrom.id] = map[takeFrom.id].filter((k) => k !== c);
  map[a.id] = add ? [...new Set([...map[a.id], c])] : [c];
  saveKeys(map);
}
function move() { const { action, add, combo, other } = clash.value; clash.value = null; put(action, add, combo, other); }
function clear(a) { const m = currentMap(); m[a.id] = []; saveKeys(m); }
function reset(a) { saveKeys(Object.fromEntries(ACTIONS.filter((x) => isChanged(x) && x.id !== a.id).map((x) => [x.id, keysOf(x)]))); }
function resetAll() { restoreAll.value = false; saveKeys({}); }
</script>

<template>
  <div>
    <p class="muted">{{ t('Saved at once. All actions can also be found in the command palette {palette}. The fixed ones are {enter} and {esc} in the translation field.', { palette: 'Ctrl+K', enter: 'Ctrl+Enter', esc: 'Esc' }) }}</p>
    <div v-if="notice" class="notice danger" role="alert">{{ notice }}</div>
    <div class="wrap"><table>
      <thead><tr><th>{{ t('Action') }}</th><th>{{ t('Shortcut') }}</th><th></th></tr></thead>
      <tbody><tr v-for="a in ACTIONS" :key="a.id">
        <td>{{ a.label }}</td>
        <td><template v-if="keysOf(a).length"><kbd v-for="k in keysOf(a)" :key="k">{{ k }}</kbd></template><span v-else class="muted">{{ t('no shortcut') }}</span></td>
        <td class="nowrap"><button class="btn small" @click="start(a, false)">{{ t('Change') }}</button>
          <button class="btn small ghost" :title="t('Add a second shortcut')" @click="start(a, true)">+</button>
          <button v-if="keysOf(a).length" class="btn small ghost" :title="t('Remove all shortcuts of this action')" @click="clear(a)">{{ t('Remove') }}</button>
          <button v-if="isChanged(a)" class="btn small ghost" :title="t('Restore the default')" @click="reset(a)">{{ t('Default') }}</button></td>
      </tr></tbody></table></div>
    <div class="actions"><button class="btn small" @click="restoreAll = true">{{ t('Restore all defaults') }}</button></div>

    <Dialog v-if="capturing" :title="t('New shortcut')" width="420px" modal>
      <p>{{ t('Press the key or combination you want to use. {esc} cancels.', { esc: 'Esc' }) }}</p>
      <template #footer><button class="btn" @click="stop">{{ t('Cancel') }}</button></template>
    </Dialog>
    <Dialog v-if="clash" :title="t('The shortcut is already used')" width="460px" @close="clash = null">
      <p>{{ t('{c} is now “{other}”. Move it to “{action}”?', { c: clash.combo, other: clash.other.label, action: clash.action.label }) }}</p>
      <template #footer><button class="btn" @click="clash = null">{{ t('Cancel') }}</button><button class="btn primary" @click="move">{{ t('Move') }}</button></template>
    </Dialog>
    <Dialog v-if="restoreAll" :title="t('Restore all shortcuts?')" width="420px" @close="restoreAll = false">
      <p>{{ t('All shortcuts return to the default.') }}</p>
      <template #footer><button class="btn" @click="restoreAll = false">{{ t('Cancel') }}</button><button class="btn primary" @click="resetAll">{{ t('Restore') }}</button></template>
    </Dialog>
  </div>
</template>

<style scoped>
.wrap { max-height: 380px; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin-top: var(--s-2); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); }
kbd { display: inline-block; margin-right: var(--s-1); padding: 1px 6px; border: 1px solid var(--line); border-radius: var(--r-sm); background: var(--bg-input); font-family: var(--mono); font-size: var(--fs-small); }
.nowrap { white-space: nowrap; }
.nowrap .btn { margin-left: var(--s-1); }
.actions { margin-top: var(--s-3); }
</style>
