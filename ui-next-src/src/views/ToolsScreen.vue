<script setup>
import { loadFiles, loadOverview } from '../project.js';
import Tools from '../components/Tools.vue';
import { t } from '../i18n.js';

/* Tools: the bulk edits of the translation (also in the Tools menu of the lines, where they can work on the selected lines, the open line and its scene). Here they work on all files. */
async function changed() { try { await Promise.all([loadOverview(), loadFiles()]); } catch { /* the numbers are fetched again by the next screen */ } }
</script>

<template>
  <main class="screen">
    <h2 class="title">{{ t('Tools') }}</h2>
    <p class="muted">{{ t('Bulk edits of translations') }}. {{ t('Every one shows a preview first; nothing is written until you confirm, and a change can be undone.') }}</p>
    <Tools mode="grid" @changed="changed" />
  </main>
</template>

<style scoped>
.screen { padding: var(--s-5) var(--s-6) var(--s-8); display: flex; flex-direction: column; gap: var(--s-3); max-width: 1100px; width: 100%; margin: 0 auto; }
.title { margin: 0; font-size: var(--fs-title); font-weight: 500; }
</style>
