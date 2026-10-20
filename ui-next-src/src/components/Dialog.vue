<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { t } from '../i18n.js';

/* A dialog: a veil, a title, the content, and the buttons in the footer slot. Escape and a click on the veil close it (unless `modal`). */
const props = defineProps({ title: { type: String, required: true }, width: { type: String, default: '560px' }, modal: { type: Boolean, default: false } });
const emit = defineEmits(['close']);
const box = ref(null);
function key(ev) { if (ev.key === 'Escape' && !props.modal) { ev.stopPropagation(); emit('close'); } }
onMounted(() => {
  window.addEventListener('keydown', key, true);
  // the first text field gets the focus (a dialog that asks for something is used by typing); otherwise the dialog itself, so that Escape works
  const first = box.value?.querySelector('input:not([type=checkbox]):not([type=radio]), textarea');
  (first || box.value)?.focus();
});
onBeforeUnmount(() => window.removeEventListener('keydown', key, true));
</script>

<template>
  <div class="veil overlay" role="dialog" aria-modal="true" :aria-label="title" @mousedown.self="!modal && emit('close')">
    <div ref="box" class="dialog" :style="{ width: `min(${width}, 100%)` }" tabindex="-1">
      <header>{{ title }}<button v-if="!modal" class="x" :aria-label="t('Close')" :title="t('Close')" @click="emit('close')">✕</button></header>
      <div class="body"><slot /></div>
      <footer v-if="$slots.footer"><slot name="footer" /></footer>
    </div>
  </div>
</template>

<style scoped>
.overlay { position: fixed; inset: 0; z-index: 20; }
.dialog { max-height: calc(100vh - 48px); display: flex; flex-direction: column; outline: none; }
.dialog > header { display: flex; align-items: center; justify-content: space-between; flex: none; }
.dialog > .body { overflow: auto; flex: 1; min-height: 0; }
.dialog > footer { flex: none; align-items: center; }
.x { all: unset; cursor: pointer; width: 26px; height: 26px; text-align: center; border-radius: var(--r-sm); color: var(--text-muted); }
.x:hover { background: var(--bg-hover); color: var(--text); }
.x:focus-visible { box-shadow: var(--focus); }
</style>
