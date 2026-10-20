<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import { t, tn } from '../i18n.js';
import { forgetFeatures } from '../pro.js';
import { loadApp } from '../state.js';
import Dialog from './Dialog.vue';

/* Settings → Licence: the state of the licence of the Pro pack, entering a key, the regular check and freeing this computer. The pack decides everything (it checks the signed licence file);
   this only shows its state and sends the key. Without the pack the state is "none". */
const st = ref(null);
const error = ref('');
const message = ref('');
const key = ref('');
const name = ref('');
const devices = ref(null);
const devicesError = ref(false);
const renaming = ref(null);          // { name }
const freeing = ref(null);           // the device to remove
const lost = ref(false);
const newKey = ref('');

/* The reasons the pack gives for a licence file it cannot use (English texts of the pack); each is a literal t() so that the pack of texts can be checked. */
const reasonText = (r) => ({
  'the licence file is not readable': () => t('the licence file is not readable'),
  'the signature of the licence is wrong': () => t('the signature of the licence is wrong'),
  'the licence belongs to another computer': () => t('the licence belongs to another computer'),
  'the licence covers another major version of the application': () => t('the licence covers another major version of the application'),
  'this build cannot verify licences': () => t('this build cannot verify licences'),
  'the licence was signed with a key this version does not know': () => t('the licence was signed with a key this version does not know'),
  'the licence was signed with a key that is no longer trusted': () => t('the licence was signed with a key that is no longer trusted'),
  'the licence file comes from a newer version of the licence system': () => t('the licence file comes from a newer version of the licence system'),
  'this licence was cancelled': () => t('this licence was cancelled'),
  'this computer was removed from the licence': () => t('this computer was removed from the licence'),
}[r] || (() => r || ''))();

async function load() {
  error.value = '';
  try { st.value = await get('licence'); } catch (e) { error.value = e.message; return; }
  devices.value = null; devicesError.value = false;
  if (st.value.state === 'licensed' || st.value.state === 'grace') {
    try { devices.value = await post('licence/devices', {}); } catch { devicesError.value = true; }
  }
}
onMounted(load);

async function run(path, body, done) {
  error.value = ''; message.value = '';
  try { await post(path, body); forgetFeatures(); loadApp(); message.value = done; } catch (e) { error.value = e.message; }
  await load();
}
const date = (ts) => new Date(ts * 1000).toLocaleDateString();
const manage = computed(() => st.value && ['licensed', 'grace', 'expired', 'refused'].includes(st.value.state));

/* a lost key: the server mails a code to the address of the purchase; with the code it replaces the key (once per licence) and this computer is activated with the new one */
const lostStep = ref(1);
const email = ref('');
const code = ref('');
const minutes = ref(15);
const lostError = ref('');
const lostBusy = ref(false);
function openLost() { lost.value = true; lostStep.value = 1; email.value = ''; code.value = ''; lostError.value = ''; newKey.value = ''; }
async function lostNext() {
  lostBusy.value = true; lostError.value = '';
  try {
    if (lostStep.value === 1) { const r = await post('licence/recover', { email: email.value }); minutes.value = r.minutes || 15; lostStep.value = 2; }
    else {
      const r = await post('licence/recover-confirm', { email: email.value, code: code.value });
      forgetFeatures();
      try { await post('licence/activate', { key: r.key }); } catch (e) { lostError.value = e.message; }
      newKey.value = r.key; lostStep.value = 3; loadApp(); load();
    }
  } catch (e) { lostError.value = e.message; }
  lostBusy.value = false;
}
async function rename() { const n = renaming.value.name; renaming.value = null; await run('licence/rename', { name: n }, t('Computer renamed.')); }
async function free() { const d = freeing.value; freeing.value = null; await run('licence/release', { target: d.id }, t('Computer removed.')); }
</script>

<template>
  <div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="message" class="notice" role="status">{{ message }}</div>
    <div v-if="!st && !error" class="skeleton" style="height: 60px"></div>
    <template v-else-if="st">
      <p v-if="st.state === 'none'" class="muted">{{ t('The Pro pack is not installed in this copy of Aside, so there is nothing to license.') }}</p>
      <p v-else-if="st.state === 'licensed'"><b>{{ t('Licensed.') }}</b> {{ t('Licence {id}; the next check with the server is due in {n} days.', { id: st.id, n: st.days_to_check }) }}</p>
      <div v-else-if="st.state === 'grace'" class="notice">{{ t('The check with the licence server is overdue. Pro keeps working for {n} more days; connect to the internet and press “Check now”.', { n: st.days_left }) }}</div>
      <div v-else-if="st.state === 'expired'" class="notice danger">{{ t('The licence has expired because it could not be checked with the server for too long. Pro works with its limits. Press “Check now” when you are online, or enter your key again.') }}</div>
      <div v-else-if="st.state === 'refused'" class="notice danger">{{ t('The licence file cannot be used: {reason}', { reason: reasonText(st.reason) }) }}</div>
      <p v-else class="muted">{{ t('No licence is entered: the Pro features that are free to try work with limits.') }}</p>

      <div v-if="manage" class="actions">
        <button class="btn" @click="run('licence/refresh', {}, t('Licence checked.'))">{{ t('Check now') }}</button>
        <button class="btn" @click="run('licence/deactivate', {}, t('Licence removed from this computer.'))">{{ t('Remove from this computer') }}</button>
      </div>

      <template v-if="st.state !== 'none' && st.state !== 'licensed' && st.state !== 'grace'">
        <label class="row"><span>{{ t('Licence key') }}</span><input v-model="key" class="text" type="text" placeholder="ASIDE-XXXXX-XXXXX-XXXXX-XXXXX" autocomplete="off" spellcheck="false"></label>
        <label class="row"><span>{{ t('Name of this computer (optional)') }}</span><input v-model="name" class="text" type="text" maxlength="40" :placeholder="t('e.g. Home PC')" autocomplete="off"></label>
        <div class="actions"><button class="btn primary" @click="run('licence/activate', { key, name }, t('Licence activated.'))">{{ t('Activate') }}</button><button class="btn ghost" @click="openLost">{{ t('I lost my key') }}</button></div>
      </template>

      <div v-if="st.state === 'licensed' || st.state === 'grace'" class="devices">
        <b>{{ t('Your computers') }}</b>
        <p v-if="devicesError" class="muted">{{ t('The list of computers needs a connection to the licence server.') }}</p>
        <template v-else-if="devices">
          <div class="muted">{{ t('Places used: {n} of {max}.', { n: devices.devices.length, max: devices.max }) }}</div>
          <div v-for="d in devices.devices" :key="d.id" class="dev">
            <span class="grow"><b>{{ d.name || t('Unnamed computer') }}</b> <span v-if="d.current" class="pill checked">{{ t('This computer') }}</span><div class="muted small">{{ t('last seen {date}', { date: date(d.last_seen) }) }}</div></span>
            <button v-if="d.current" class="btn" @click="renaming = { name: d.name || '' }">{{ t('Rename') }}</button>
            <button v-else class="btn" :disabled="devices.releases_left === 0" @click="freeing = d">{{ t('Remove') }}</button>
          </div>
          <p v-if="devices.releases_left === 0" class="muted">{{ t('You have used all releases. Write to the seller if you need to free a place.') }}</p>
        </template>
      </div>
    </template>

    <Dialog v-if="renaming" :title="t('Name of this computer:')" width="440px" @close="renaming = null">
      <input v-model="renaming.name" class="text" type="text" maxlength="40" :aria-label="t('Name of this computer:')" @keydown.enter="rename">
      <template #footer><button class="btn" @click="renaming = null">{{ t('Cancel') }}</button><button class="btn primary" @click="rename">{{ t('Rename') }}</button></template>
    </Dialog>
    <Dialog v-if="freeing" :title="t('Remove')" width="480px" @close="freeing = null">
      <p>{{ t('Free the place of “{name}”? You can do this {left} more time(s).', { name: freeing.name || t('Unnamed computer'), left: devices.releases_left }) }}</p>
      <template #footer><button class="btn" @click="freeing = null">{{ t('Cancel') }}</button><button class="btn primary" @click="free">{{ t('Remove') }}</button></template>
    </Dialog>
    <Dialog v-if="lost" :title="lostStep === 3 ? t('Your new key') : t('Lost licence key')" width="520px" @close="lost = false">
      <template v-if="lostStep === 1">
        <p class="muted">{{ t('Enter the e-mail address you bought Aside Pro with. We will send a code to it; your old key keeps working until you enter the code.') }}</p>
        <label class="row"><span>{{ t('E-mail address') }}</span><input v-model="email" class="text" type="text" placeholder="name@example.com" autocomplete="email"></label>
      </template>
      <template v-else-if="lostStep === 2">
        <p>{{ t('If the address belongs to a licence, a code was sent to it (valid for {n} minutes). Enter it here.', { n: minutes }) }}</p>
        <label class="row"><span>{{ t('Code') }}</span><input v-model="code" class="text" type="text" placeholder="XXXX-XXXX" autocomplete="off" spellcheck="false"></label>
        <p class="muted">{{ t('You get a new key and the old one stops working. Computers where Aside is already activated keep working. This can be done once per licence.') }}</p>
      </template>
      <p v-else>{{ t('Your new key is {key}. The old key no longer works. It was also sent to your e-mail.', { key: newKey }) }}</p>
      <div v-if="lostError" class="notice danger" role="alert">{{ lostError }}</div>
      <template #footer>
        <button v-if="lostStep < 3" class="btn" @click="lost = false">{{ t('Cancel') }}</button>
        <button v-if="lostStep < 3" class="btn primary" :disabled="lostBusy" @click="lostNext">{{ t('Continue') }}</button>
        <button v-else class="btn primary" @click="lost = false">{{ t('Close') }}</button>
      </template>
    </Dialog>
  </div>
</template>

<style scoped>
.actions { display: flex; flex-wrap: wrap; gap: var(--s-2); margin: var(--s-3) 0; }
.row { display: grid; grid-template-columns: 260px minmax(0, 1fr); gap: var(--s-3); align-items: center; margin: var(--s-2) 0; font-size: var(--fs-body); }
.text { height: var(--h-control); padding: 0 var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; width: 100%; box-sizing: border-box; }
.text:focus { outline: none; border-color: var(--accent-hi); }
.devices { margin-top: var(--s-4); }
.dev { display: flex; align-items: center; gap: var(--s-3); border-top: 1px solid var(--line-soft); padding: var(--s-2) 0; }
.grow { flex: 1; }
.small { font-size: var(--fs-small); }
</style>
