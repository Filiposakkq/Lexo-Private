<script setup>
import { computed, nextTick, onMounted, ref } from 'vue';
import { t } from '../i18n.js';
import { ACTIONS, handlers, keysOf } from '../keys.js';
import { project } from '../project.js';
import Dialog from './Dialog.vue';

/* The command palette: type to find an action or a file. Enter runs the selected one, the arrows move, Esc closes. Only actions that the open screen can do are offered. */
const emit = defineEmits(['close', 'run', 'file']);
const q = ref('');
const sel = ref(0);
const input = ref(null);
const list = ref(null);
const fold = (s) => String(s || '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
const items = computed(() => {
  const x = fold(q.value.trim());
  const acts = ACTIONS.filter((a) => handlers[a.id] && (!x || fold(a.label).includes(x) || fold(a.id).includes(x))).map((a) => ({ label: a.label, hint: keysOf(a)[0] || '', run: () => emit('run', a) }));
  const files = x ? (project.files || []).map((f) => f.name).filter((n) => fold(n).includes(x)).slice(0, 8).map((n) => ({ label: t('File: {name}', { name: n }), hint: t('lines'), run: () => emit('file', n) })) : [];
  return [...acts, ...files].slice(0, 40);
});
onMounted(() => input.value?.focus());
function move(d) { sel.value = Math.max(0, Math.min(items.value.length - 1, sel.value + d)); nextTick(() => list.value?.children[sel.value]?.scrollIntoView({ block: 'nearest' })); }
function choose(i) { const it = items.value[i]; if (it) it.run(); }
function key(e) {
  if (e.key === 'ArrowDown') { e.preventDefault(); move(1); } else if (e.key === 'ArrowUp') { e.preventDefault(); move(-1); } else if (e.key === 'Enter') { e.preventDefault(); choose(sel.value); }
}
</script>

<template>
  <Dialog :title="t('Command palette')" width="560px" @close="emit('close')">
    <input ref="input" v-model="q" class="q" type="search" :placeholder="t('What do you want to do? An action, or the beginning of a file name…')" :aria-label="t('Command palette')" @input="sel = 0" @keydown="key">
    <div ref="list" class="list" role="listbox">
      <div v-for="(it, i) in items" :key="i" class="row-p" :class="{ on: i === sel }" role="option" :aria-selected="i === sel" @click="choose(i)" @mousemove="sel = i"><span>{{ it.label }}</span><kbd v-if="it.hint">{{ it.hint }}</kbd></div>
      <div v-if="!items.length" class="muted none">{{ t('Nothing found.') }}</div>
    </div>
  </Dialog>
</template>

<style scoped>
.q { width: 100%; box-sizing: border-box; height: var(--h-control); padding: 0 var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; }
.q:focus { outline: none; border-color: var(--accent-hi); }
.list { margin-top: var(--s-2); max-height: 46vh; overflow: auto; }
.row-p { display: flex; justify-content: space-between; gap: var(--s-3); padding: 7px var(--s-3); border-radius: var(--r-md); cursor: pointer; font-size: var(--fs-body); }
.row-p.on { background: var(--bg-hover); box-shadow: inset 3px 0 0 var(--accent-hi); }
.none { padding: var(--s-3); }
kbd { padding: 1px 6px; border: 1px solid var(--line); border-radius: var(--r-sm); background: var(--bg-input); font-family: var(--mono); font-size: var(--fs-small); }
</style>
