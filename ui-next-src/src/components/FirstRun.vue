<script setup>
import { t } from '../i18n.js';

/* The first start: nothing is on Home yet, so the screen says what to do. */
const emit = defineEmits(['add']);
</script>

<template>
  <section class="first">
    <div class="mark">A</div>
    <h1>{{ t('Welcome to Aside Pro') }}</h1>
    <p class="muted">{{ t('Translate Ren\'Py games without hunting for scripts. Everything stays on your computer.') }}</p>
    <ol class="steps">
      <li class="card"><b class="n on">1</b><div class="heading">{{ t('Choose a game') }}</div><div class="muted">{{ t('The folder of a game, or of a finished translation.') }}</div></li>
      <li class="card"><b class="n">2</b><div class="heading">{{ t('Prepare it') }}</div><div class="muted">{{ t('Aside unpacks the archives and makes the files of the translation.') }}</div></li>
      <li class="card"><b class="n">3</b><div class="heading">{{ t('Translate') }}</div><div class="muted">{{ t('Lines, checks, a glossary. Every change can be undone.') }}</div></li>
    </ol>
    <div class="drop">
      <div class="heading">{{ t('Add your first game') }}</div>
      <button class="btn primary" @click="emit('add')">{{ t('Add a game') }}</button>
    </div>
  </section>
</template>

<style scoped>
.first { max-width: 540px; margin: 0 auto; padding: var(--s-8) var(--s-4); display: flex; flex-direction: column; align-items: center; gap: var(--s-4); text-align: center; }
.mark { width: 56px; height: 56px; border-radius: 14px; background: var(--accent); color: var(--on-accent); font-size: 30px; font-weight: 500; display: flex; align-items: center; justify-content: center; }
h1 { font-size: var(--fs-hero); font-weight: 500; }
.steps { list-style: none; padding: 0; margin: 0; width: 100%; display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: var(--s-3); text-align: left; }
.steps .card { display: flex; flex-direction: column; gap: var(--s-1); }
.n { width: 26px; height: 26px; border-radius: 50%; border: 1px solid var(--line-accent); color: var(--accent-text); display: flex; align-items: center; justify-content: center; font-size: var(--fs-small); font-weight: 500; margin-bottom: var(--s-2); }
.n.on { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
.drop { width: 100%; border: 1.5px dashed var(--line-accent); border-radius: var(--r-xl); padding: var(--s-5) var(--s-4); background: var(--bg-rail); display: flex; flex-direction: column; align-items: center; gap: var(--s-3); }
</style>
