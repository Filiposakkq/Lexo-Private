<script setup>
import { ref } from 'vue';
import { post } from '../api.js';
import { t } from '../i18n.js';

/* Add a game by its folder: type or paste the path, or choose it (the choice dialog exists in the application window; in a browser the path is typed). */
const emit = defineEmits(['added', 'close']);
const path = ref('');
const error = ref('');
const working = ref(false);

async function choose() {
  error.value = '';
  try {
    const r = await post('carry/pick', {});
    if (!r.cancelled) path.value = r.path;
  } catch (e) { error.value = e.message; }
}
async function add() {
  error.value = '';
  if (!path.value.trim()) { error.value = t('Enter the folder of the game.'); return; }
  working.value = true;
  try {
    await post('library/add', { path: path.value.trim() });
    emit('added');
  } catch (e) { error.value = e.message; } finally { working.value = false; }
}
</script>

<template>
  <div class="veil overlay" @keydown.esc="emit('close')" role="dialog" aria-modal="true" :aria-label="t('Add a game')">
    <div class="dialog">
      <header>{{ t('Add a game') }}</header>
      <div class="body">
        <p class="muted">{{ t('Choose the folder of the game (the one with the game folder inside) or a folder of a translation. Nothing in the game is changed.') }}</p>
        <div class="row-f">
          <label class="field" :class="{ invalid: error }"><input v-model="path" :placeholder="t('Folder of the game')" autofocus @keydown.enter="add"></label>
          <button class="btn" @click="choose">{{ t('Browse…') }}</button>
        </div>
        <div v-if="error" class="hint err" role="alert">{{ error }}</div>
      </div>
      <footer>
        <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
        <button class="btn primary" :disabled="working" @click="add">{{ t('Add the game') }}</button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.overlay { position: fixed; inset: 0; z-index: 8; }
.row-f { display: flex; gap: var(--s-2); margin-top: var(--s-3); }
.row-f .field { flex: 1; }
.err { color: var(--danger); }
</style>
