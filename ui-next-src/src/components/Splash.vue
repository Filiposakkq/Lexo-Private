<script setup>
import { onMounted } from 'vue';
import { t } from '../i18n.js';

/* The animation at the start: the mark, the name, a thin bar. About 3 seconds, only when the library is loading; skipped when the system asks for less motion. */
const emit = defineEmits(['done']);
onMounted(() => {
  const calm = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  setTimeout(() => emit('done'), calm ? 0 : 2800);
});
</script>

<template>
  <div class="splash" aria-live="polite">
    <div class="mark">A</div>
    <div class="word">Aside <span>Pro</span></div>
    <div class="bar"><i></i></div>
    <div class="load">{{ t('Loading the library of games…') }}</div>
  </div>
</template>

<style scoped>
.splash { position: fixed; inset: 0; z-index: 10; background: var(--bg-app); display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px; animation: out var(--t-slow) 2.3s forwards; }
.mark { width: 64px; height: 64px; border-radius: 16px; background: var(--accent); color: var(--on-accent); display: flex; align-items: center; justify-content: center; font-size: 34px; font-weight: 500; transform: scale(.4); opacity: 0; animation: mark .55s .1s var(--ease-pop) forwards; }
.word { font-size: 30px; font-weight: 500; opacity: 0; transform: translateY(8px); animation: up .5s .5s forwards; }
.word span { color: var(--accent-text); }
.bar { width: 180px; height: 3px; background: var(--track); border-radius: 2px; overflow: hidden; opacity: 0; animation: show .2s 1s forwards; }
.bar i { display: block; height: 3px; width: 0; background: var(--accent-hi); animation: fill 1.1s 1.05s var(--ease) forwards; }
.load { font-size: var(--fs-small); color: var(--text-muted); opacity: 0; animation: show .3s 1.05s forwards; }
@keyframes mark { to { transform: scale(1); opacity: 1; } }
@keyframes up { to { opacity: 1; transform: none; } }
@keyframes show { to { opacity: 1; } }
@keyframes fill { to { width: 100%; } }
@keyframes out { to { opacity: 0; visibility: hidden; } }
@media (prefers-reduced-motion: reduce) { .splash { display: none; } }
</style>
