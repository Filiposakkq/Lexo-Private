<script setup>
import { onMounted, ref } from 'vue';
import Splash from './components/Splash.vue';
import Home from './views/Home.vue';
import { t } from './i18n.js';
import { app, loadApp } from './state.js';

/* The shell: the icon rail, the top bar with the name (Aside, or Aside Pro when Pro is on) and the screen. Home is the first screen of the new interface; the rest of the application is the classic interface for now. */
const splash = ref(true);
let seen = false;
try { seen = sessionStorage.getItem('aside.splash') === '1'; } catch { /* no storage: the animation plays */ }
if (seen) splash.value = false;
function splashDone() {
  splash.value = false;
  try { sessionStorage.setItem('aside.splash', '1'); } catch { /* nothing to remember */ }
}
onMounted(loadApp);
</script>

<template>
  <Splash v-if="splash" @done="splashDone" />
  <div class="shell">
    <nav class="rail" :aria-label="t('Main menu')">
      <a class="rail-item on" href="/next/" :title="t('Home')" aria-current="page">⌂</a>
      <a class="rail-item" href="/" :title="t('The rest of the application is in the classic interface for now')">☰</a>
      <span class="grow"></span>
      <a class="rail-item" href="/" :title="t('Settings (in the classic interface for now)')">⚙</a>
    </nav>
    <div class="main">
      <header class="topbar">
        <span class="logo">A</span>
        <span class="name">Aside<span v-if="app.pro"> <b class="pro">Pro</b></span></span>
        <span class="ver faint" v-if="app.version">{{ app.version }}</span>
        <span class="grow"></span>
        <span v-if="app.pro" class="tag-pro">{{ t('Pro is active') }}</span>
      </header>
      <Home />
    </div>
  </div>
</template>

<style scoped>
.shell { display: flex; min-height: 100vh; }
.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.rail { position: sticky; top: 0; height: 100vh; }
.rail-item { text-decoration: none; font-size: 20px; }
.grow { flex: 1; }
.name { font-size: var(--fs-title); font-weight: 500; }
.pro { font-weight: 500; color: var(--accent-text); }
.ver { font-size: var(--fs-small); margin-top: 4px; }
</style>
