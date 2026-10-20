<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import FocusBar from './components/FocusBar.vue';
import HelpDialog from './components/HelpDialog.vue';
import Palette from './components/Palette.vue';
import ProLocked from './components/ProLocked.vue';
import Splash from './components/Splash.vue';
import Home from './views/Home.vue';
import Prepare from './views/Prepare.vue';
import Overview from './views/Overview.vue';
import Files from './views/Files.vue';
import Lines from './views/Lines.vue';
import Check from './views/Check.vue';
import Glossary from './views/Glossary.vue';
import Publish from './views/Publish.vue';
import Settings from './views/Settings.vue';
import Team from './views/Team.vue';
import ToolsScreen from './views/ToolsScreen.vue';
import { post } from './api.js';
import { enterQuick, setFocus, ui } from './layout.js';
import { handlers, onKeydown, register } from './keys.js';
import { t, tn } from './i18n.js';
import { app, loadApp, loadGames } from './state.js';
import { loadOverview, loadTeam, openCheck, openLines, project, reloadAll } from './project.js';

/* The shell: the icon rail, the top bar with the name (Aside, or Aside Pro when Pro is on) and the screen. Home and the first screens of a game (Overview, Files, Lines) are in the new interface;
   the other screens of a game (Check, Glossary, Tools, Team, Release, Settings) are still the classic ones and are linked from the menu of the game. */
const splash = ref(true);
const view = ref('home');          // home | prepare | overview | files | lines
const prepPath = ref('');
function prepare(path) { prepPath.value = path || ''; view.value = 'prepare'; }
function home() { view.value = 'home'; loadGames(); }
const PROJECT_VIEWS = ['overview', 'files', 'lines', 'check', 'glossary', 'tools', 'team', 'publish', 'settings'];
const inProject = computed(() => PROJECT_VIEWS.includes(view.value));
async function opened() { view.value = 'overview'; loadTeam(); try { await loadOverview(); } catch { /* the screen shows the error itself */ } }
const projectName = computed(() => project.overview?.project?.name || project.overview?.project?.dir || '');
const MENU = [['overview', () => t('Overview'), '◔'], ['files', () => t('Files'), '▤'], ['lines', () => t('Lines'), '≡'], ['check', () => t('Check'), '✓'], ['glossary', () => t('Glossary'), 'Aa'], ['tools', () => t('Tools'), '⚒'], ['team', () => t('Team'), '☺'], ['publish', () => t('Publishing'), '⇪'], ['settings', () => t('Settings'), '⚙']];
function openSettings() { if (project.overview) view.value = 'settings'; }
let seen = false;
try { seen = sessionStorage.getItem('aside.splash') === '1'; } catch { /* no storage: the animation plays */ }
if (seen) splash.value = false;
function splashDone() {
  splash.value = false;
  try { sessionStorage.setItem('aside.splash', '1'); } catch { /* nothing to remember */ }
}
const help = ref(false);
/* The focus mode (only the list and the detail, a floating bar) works on the Lines screen; the quick modes are the film layout + focus + only the lines still to translate / to check. */
const focusOn = computed(() => ui.focus && view.value === 'lines');
function startQuick(kind) {
  if (!project.overview) return;
  enterQuick(kind);
  openLines({ file: project.lines.file || '', status: kind === 'proof' ? 'review' : 'todo' });
  view.value = 'lines';
}
function focusAct(id) { if (id === 'quick') startQuick('tr'); else if (id === 'proofmode') startQuick('proof'); else if (handlers[id]) handlers[id](); }
ui.left = ui.left || {};
const palette = ref(false);
const flash = ref('');
let flashT = null;
function say(text) { flash.value = text; clearTimeout(flashT); flashT = setTimeout(() => { flash.value = ''; }, 4000); }
const goView = (v) => { if (project.overview) view.value = v; };
function goCheck(mode) { if (!project.overview) return; openCheck({ mode }); view.value = 'check'; }
function search() { const el = document.querySelector('main input[type="search"]'); if (el) { el.focus(); el.select?.(); } }
async function afterChange() { try { await reloadAll(); } catch { /* the screens show their own errors */ } window.dispatchEvent(new Event('aside:reload')); }
async function doUndo() {
  if (!project.overview) return;
  try { const r = await post('undo'); say(t('Undone: {what}', { what: r.undone })); await afterChange(); } catch (e) { say(e.message); }
}
async function doRefresh() {
  if (!project.overview) return;
  try {
    let r = await post('refresh', { force: false, dirty: false });
    if (r.needs_confirm) { if (!window.confirm(r.message)) return; r = await post('refresh', { force: false, discard: true }); }
    say(r.changed ? t('Refreshed – {summary}', { summary: r.summary }) : t('Refreshed – no changes.'));
    await afterChange();
  } catch (e) { say(e.message); }
}
async function snapshot() {
  try { const r = await post('snapshots/create'); say(t('Backup created ({files} files, {size}).', { files: r.snapshot.files, size: Math.max(1, Math.round(r.snapshot.bytes / 1024)) + ' kB' })); } catch (e) { say(e.message); }
}
async function fullscreen() {
  try { const r = await post('window/fullscreen', {}); if (r.ok) return; } catch { /* the browser way */ }
  try { if (document.fullscreenElement) await document.exitFullscreen(); else await document.documentElement.requestFullscreen(); } catch (e) { say(e.message); }
}
function runAction(a) { palette.value = false; setTimeout(() => { const fn = handlersOf(a.id); if (fn) fn(); }, 0); }
let handlersMap = {};
const handlersOf = (id) => handlersMap[id];
function paletteFile(name) { palette.value = false; if (!project.overview) return; openLines({ file: name, status: 'all' }); view.value = 'lines'; }
onMounted(() => {
  loadApp();
  handlersMap = {
    palette: () => { palette.value = true; }, help: () => { help.value = true; }, search, search_slash: search, refresh: doRefresh, undo: doUndo, fullscreen,
    tab_overview: () => goView('overview'), tab_files: () => goView('files'), tab_lines: () => goView('lines'), tab_check: () => goView('check'), tab_glossary: () => goView('glossary'),
    tab_tools: () => goView('tools'), tab_team: () => goView('team'), tab_publish: () => goView('publish'), tab_settings: () => goView('settings'),
    focus: () => { if (view.value === 'lines') setFocus(!ui.focus); }, quick: () => startQuick('tr'), proofmode: () => startQuick('proof'),
    consistency: () => goCheck('cons'), ignored: () => goCheck('ign'), lint: () => goCheck('lint'), findings: () => goCheck('findings'), snapshot,
  };
  offKeys = register(handlersMap);
  window.addEventListener('keydown', onKeydown);
});
let offKeys = null;
onBeforeUnmount(() => { if (offKeys) offKeys(); window.removeEventListener('keydown', onKeydown); clearTimeout(flashT); });
</script>

<template>
  <Splash v-if="splash" @done="splashDone" />
  <ProLocked />
  <Palette v-if="palette" @close="palette = false" @run="runAction" @file="paletteFile" />
  <HelpDialog v-if="help" @close="help = false" @settings="help = false; goView('settings')" />
  <div v-if="flash" class="flash toast" role="status">{{ flash }}</div>
  <FocusBar v-if="focusOn" :left="ui.left" @act="focusAct" />
  <div class="shell" :class="{ focus: focusOn }">
    <nav class="rail" :aria-label="t('Main menu')">
      <a class="rail-item" :class="{ on: view === 'home' }" href="#" :title="t('Home')" :aria-current="view === 'home' ? 'page' : null" @click.prevent="home">⌂</a>
      <a class="rail-item" :class="{ on: view === 'prepare' }" href="#" :title="t('Prepare a game')" :aria-current="view === 'prepare' ? 'page' : null" @click.prevent="prepare()">⚒</a>
      <a class="rail-item" href="/" :title="t('The rest of the application is in the classic interface for now')">☰</a>
      <span class="grow"></span>
      <a class="rail-item" :class="{ on: view === 'settings' }" href="#" :title="t('Settings')" @click.prevent="openSettings">⚙</a>
    </nav>
    <div class="main">
      <header class="topbar">
        <span class="logo">A</span>
        <span class="name">Aside<span v-if="app.pro"> <b class="pro">Pro</b></span></span>
        <span class="ver faint" v-if="app.version">{{ app.version }}</span>
        <span class="grow"></span>
        <span v-if="app.pro" class="tag-pro">{{ t('Pro is active') }}</span>
      </header>
      <div class="body">
        <nav v-if="inProject" class="nav" :aria-label="t('Game menu')">
          <div class="game" :title="projectName">{{ projectName }}</div>
          <a v-for="[k, label, glyph] in MENU" :key="k" class="nav-item" :class="{ on: view === k }" href="#" :aria-current="view === k ? 'page' : null" @click.prevent="view = k"><span aria-hidden="true">{{ glyph }}</span>{{ label() }}</a>
        </nav>
        <div class="screen-host">
          <Home v-if="view === 'home'" @prepare="prepare" @opened="opened" />
          <Prepare v-else-if="view === 'prepare'" :path="prepPath" @back="home" @done="loadGames" @opened="opened" />
          <Overview v-else-if="view === 'overview'" @go="view = $event" @quick="startQuick" />
          <Files v-else-if="view === 'files'" @go="view = $event" />
          <Lines v-else-if="view === 'lines'" @go="view = $event" @quick="startQuick" />
          <Check v-else-if="view === 'check'" @go="view = $event" />
          <Glossary v-else-if="view === 'glossary'" @go="view = $event" />
          <Publish v-else-if="view === 'publish'" @go="view = $event" />
          <Settings v-else-if="view === 'settings'" @go="view = $event" />
          <ToolsScreen v-else-if="view === 'tools'" />
          <Team v-else-if="view === 'team'" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.shell { display: flex; min-height: 100vh; }
.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.body { flex: 1; display: flex; min-height: 0; }
.screen-host { flex: 1; min-width: 0; display: flex; }
.screen-host > :deep(*) { flex: 1; min-width: 0; }
.nav-item { text-decoration: none; }
.shell.focus { --topbar-h: 0px; }
.shell.focus .rail, .shell.focus .topbar, .shell.focus .nav { display: none; }
.flash { position: fixed; right: var(--s-5); bottom: var(--s-5); z-index: 30; }
.nav { flex: none; min-height: calc(100vh - 52px); }
.game { padding: var(--s-2) var(--s-3) var(--s-3); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rail { position: sticky; top: 0; height: 100vh; }
.rail-item { text-decoration: none; font-size: 20px; }
.grow { flex: 1; }
.name { font-size: var(--fs-title); font-weight: 500; }
.pro { font-weight: 500; color: var(--accent-text); }
.ver { font-size: var(--fs-small); margin-top: 4px; }
</style>
