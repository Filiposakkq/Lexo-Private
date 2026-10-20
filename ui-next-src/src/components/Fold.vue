<script setup>
import { ref } from 'vue';

/* A section that collapses (the preview, the scene around the line, the memory, the history). The choice is remembered by the id, for the next line and the next visit. */
const props = defineProps({ id: { type: String, required: true }, open: { type: Boolean, default: true } });
const emit = defineEmits(['toggle']);
function stored() { try { const v = localStorage.getItem('aside.fold.' + props.id); return v === null ? props.open : v === '1'; } catch { return props.open; } }
const isOpen = ref(stored());
function toggled(ev) {
  isOpen.value = ev.target.open;
  try { localStorage.setItem('aside.fold.' + props.id, isOpen.value ? '1' : '0'); } catch { /* only for this page */ }
  emit('toggle', isOpen.value);
}
</script>

<template>
  <details class="fold" :open="isOpen" @toggle="toggled">
    <summary><slot name="title" /></summary>
    <slot v-if="isOpen" />
  </details>
</template>

<style scoped>
.fold { margin-bottom: var(--s-3); }
.fold > summary { cursor: pointer; list-style: none; display: flex; align-items: center; gap: var(--s-2); font-size: var(--fs-body); font-weight: 500; color: var(--text-2); padding: var(--s-1) 0; }
.fold > summary::-webkit-details-marker { display: none; }
.fold > summary::before { content: '▸'; color: var(--text-faint); width: 1em; }
.fold[open] > summary::before { content: '▾'; }
.fold > summary:focus-visible { outline: none; box-shadow: var(--focus); border-radius: var(--r-sm); }
</style>
