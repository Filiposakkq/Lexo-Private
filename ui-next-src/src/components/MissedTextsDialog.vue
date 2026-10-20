<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';

/* Texts the generator missed (experimental): texts shown by show text, an input prompt or a title kept in a variable, guessed from the scripts, that you can add to the translation.
   The project remembers its own text functions (notify, popup...) and the texts you ignored. */
const emit = defineEmits(['close', 'done']);
const data = ref(null);
const level = ref('likely');
const sel = ref(new Set());
const intoFiles = ref(false);
const fn = ref('');
const suggest = ref(null);
const error = ref('');
const busy = ref(false);
const shown = computed(() => (data.value ? data.value.items.slice(0, 500) : []));
const KIND = () => ({ text: t('text'), dialogue: t('dialogue without a block') });

async function load() {
  error.value = '';
  try {
    data.value = await get('missed', { level: level.value });
    sel.value = new Set(data.value.items.slice(0, 500).map((it, i) => (it.level === 'likely' ? i : -1)).filter((i) => i >= 0));
    fn.value = data.value.settings.functions.join(', ');
  } catch (e) { error.value = e.message; }
}
onMounted(load);
const names = () => fn.value.split(/[,;\s]+/).filter(Boolean);
async function saveFunctions() { try { await post('missed/settings', { functions: names() }); await load(); } catch (e) { error.value = e.message; } }
async function findFunctions() {
  suggest.value = { busy: true };
  try { suggest.value = await get('missed/functions'); } catch (e) { suggest.value = null; error.value = e.message; }
}
function addFunction(name) { if (!names().includes(name)) fn.value = names().concat(name).join(', '); }
function toggle(i, on) { const s = new Set(sel.value); if (on) s.add(i); else s.delete(i); sel.value = s; }
const chosen = () => [...sel.value].filter((i) => i < 500).map((i) => data.value.items[i]);
async function add() {
  busy.value = true;
  try { await post('missed/add', { items: chosen(), into_files: intoFiles.value }); emit('done'); } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
async function ignore() { try { await post('missed/ignore', { texts: chosen().map((i) => i.text) }); await load(); } catch (e) { error.value = e.message; } }
async function forget() { try { await post('missed/ignore', { clear: true }); await load(); } catch (e) { error.value = e.message; } }
</script>

<template>
  <Dialog :title="t('Texts the generator missed')" width="1080px" @close="emit('close')">
    <div class="notice warn"><div><b class="up">{{ t('Experimental feature') }}</b><br>{{ t("The list is a guess from the game's scripts and can be wrong in both directions (texts missing from it, texts that do not belong there). It still saves a lot of searching, but always review it before adding.") }}</div></div>
    <p class="muted">{{ t('Ren\'Py\'s generator takes only dialogue, menu choices, screens and strings marked _("…"). Texts shown in other ways, such as show text, an input prompt or a title kept in a variable, are not in the translation files. This list is a guess from the game\'s scripts – tick what is really shown to the player. The chosen texts are added to extra_strings.rpy and appear as untranslated lines.') }}</p>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!data && !error" class="skeleton" style="height: 100px"></div>
    <div v-else-if="data && data.message" class="empty muted"><b>{{ t('Nothing to search') }}</b> {{ data.message }}</div>
    <template v-else-if="data">
      <div class="line">
        <label class="check"><input type="checkbox" :checked="level === 'all'" @change="level = $event.target.checked ? 'all' : 'likely'; load()"> {{ t('Also show weaker guesses ({n})', { n: data.counts.maybe }) }}</label>
        <button v-if="data.settings.ignored.length" class="btn small ghost" @click="forget">{{ t('Forget the {n} ignored texts', { n: data.settings.ignored.length }) }}</button>
      </div>
      <label class="check"><input type="checkbox" v-model="intoFiles"> {{ t('Put each text into the translation file of its script') }} <span class="pill todo">{{ t('Experimental') }}</span></label>
      <div class="muted small">{{ t('Experimental, may not work correctly everywhere, but it mostly does and saves work. Without it all the texts go to extra_strings.rpy. The game does not care where they are; it only keeps the file and group statistics tidy.') }}</div>
      <div class="line"><b>{{ t('Functions of this game that show text:') }}</b><label class="field fn"><input v-model="fn" placeholder="notify, show_message"></label>
        <button class="btn small" @click="saveFunctions">{{ t('Save') }}</button><button class="btn small" @click="findFunctions">{{ t('Find functions…') }}</button></div>
      <div v-if="suggest" class="line">
        <span v-if="suggest.busy" class="muted">{{ t('Searching…') }}</span>
        <template v-else>
          <span class="muted">{{ suggest.suggested.length ? t('Functions called again and again with a text (click to add):') : t('No function called with a text again and again was found.') }}</span>
          <button v-for="x in suggest.suggested" :key="x.name" class="btn small" :title="t('{n} calls, for example: {text}', { n: x.calls, text: x.example })" @click="addFunction(x.name)">{{ x.name }}</button>
        </template>
      </div>
      <p v-if="data.truncated || data.items.length > 500" class="muted small">{{ t('Showing the first 500 of {total}; only the shown ones can be added.', { total: data.items.length }) }}</p>
      <div v-if="data.items.length" class="line"><b>{{ t('Selection:') }}</b>
        <button class="btn small" @click="sel = new Set(shown.map((_, i) => i))">{{ t('Select all') }}</button><button class="btn small" @click="sel = new Set()">{{ t('Select none') }}</button></div>
      <div v-if="data.items.length" class="wrap"><table>
        <thead><tr><th></th><th>{{ t('Kind') }}</th><th>{{ t('Where') }}</th><th>{{ t('Text') }}</th><th>{{ t('Code') }}</th></tr></thead>
        <tbody><tr v-for="(it, i) in shown" :key="i">
          <td><input type="checkbox" :checked="sel.has(i)" :aria-label="baseName(it.file) + ':' + it.line" @change="toggle(i, $event.target.checked)"></td>
          <td><span class="pill" :class="it.level === 'likely' ? 'checked' : 'none'">{{ it.level === 'likely' ? t('likely') : t('maybe') }}</span><div class="muted small">{{ KIND()[it.kind] || it.kind }}</div></td>
          <td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ t('line {n}', { n: it.line }) }}</span><div v-if="it.count > 1" class="muted">{{ tn(it.count, '{n} place', '{n} places') }}</div></td>
          <td class="txt">{{ it.text }}</td><td class="mono muted txt">{{ it.context }}</td>
        </tr></tbody></table></div>
      <div v-else class="empty muted"><b>{{ t('Nothing found') }}</b> {{ t('The scripts hold no text that the translation lacks.') }}</div>
    </template>
    <template #footer>
      <span class="muted grow small">{{ data && !data.message ? t('Found {total} · selected {n}', { total: data.counts.likely + (level === 'all' ? data.counts.maybe : 0), n: sel.size }) : '' }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn" :disabled="!sel.size" :title="t('The project will not show these texts again')" @click="ignore">{{ t('Ignore selected ({n})', { n: sel.size }) }}</button>
      <button class="btn primary" :disabled="!sel.size || busy" @click="add">{{ t('Add selected ({n})', { n: sel.size }) }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.up { text-transform: uppercase; letter-spacing: .06em; }
.line { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin: var(--s-2) 0; }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); }
.fn { width: 260px; }
.wrap { max-height: 38vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.txt { max-width: 340px; word-break: break-word; }
.nowrap { white-space: nowrap; }
.empty { padding: var(--s-5); text-align: center; }
.grow { flex: 1; }
.small { font-size: var(--fs-small); }
</style>
