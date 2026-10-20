<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import Fold from './Fold.vue';
import { t, tn } from '../i18n.js';
import { fontOf, gameText, lastSize, loadGui, metricsOf, peekGui, peekScene, placeChars, prefetchScene } from '../gamewindow.js';

/* The line as the player sees it: the picture of the scene (found in the game's archives by following the game script upwards to the last `scene` / `show`) with the
   dialogue box drawn over it, sized by the game's own gui.rpy values. Without the game folder it is the box alone. The font is the game's own when it can be had;
   wrapping is an estimate. The text follows the editor while you type. */
const props = defineProps({ detail: { type: Object, required: true }, text: { type: String, default: '' } });
const mode = ref('target');
const outer = ref(null);
const info = ref('');
const lines = ref(0);
const over = ref(false);
let gui = null, scene = null, imgOk = true, seq = 0, timer = null, ro = null;
const badLayers = new Set();

/* When the scene, the window values and the fonts are already known (the list prefetches them before the next line opens) the picture is drawn at once, in the same task as the
   rest of the detail; only a line nobody looked at yet waits for them. */
function draw() {
  if (!outer.value) return;
  const my = ++seq;
  gui = gui || peekGui();
  scene = scene || peekScene(props.detail.file, props.detail.line);
  if (gui && scene) { paint(); return; }
  return (async () => {
    if (!gui) gui = await loadGui(t('default values'));
    if (!scene) scene = await prefetchScene(props.detail.file, props.detail.line);
    if (my !== seq || !outer.value) return;
    paint();
  })();
}
function paint() {
  const d = props.detail;
  const g = { ...gui, ...(scene.gui || {}) };
  const fam = fontOf(scene.fonts_text), nfam = fontOf(scene.fonts_name);
  const mText = fam ? metricsOf(scene.fonts_text) : null, mName = nfam ? metricsOf(scene.fonts_name) : null;
  const sp = scene.speaker;                       // the game's own Character(...) name and colour
  const speaker = sp ? sp.name : (d.who || '');
  const W = g.screen_width || 1920, H = g.screen_height || 1080;
  const text = mode.value === 'source' ? (d.source ?? '') : props.text;
  const mk = (cls, style) => { const e = document.createElement('div'); e.className = cls; if (style) Object.assign(e.style, style); return e; };

  const stage = mk('gstage', { width: W + 'px', height: H + 'px', fontSize: g.text_size + 'px', ...(fam ? { fontFamily: `"${fam}", sans-serif` } : {}) });
  if (scene.flat) stage.style.background = scene.flat;
  else if (scene.url && imgOk) {
    const im = document.createElement('img'); im.className = 'gbg'; im.alt = ''; im.src = scene.url; im.onerror = () => { imgOk = false; draw(); }; stage.append(im);
  }
  for (const l of scene.layers || []) {           // pictures `show`n over the background, at their natural size, standing on the bottom edge
    if (badLayers.has(l.url)) continue;
    const im = document.createElement('img'); im.className = 'glay'; im.alt = ''; im.src = l.url;
    Object.assign(im.style, { left: l.x * 100 + '%', transform: `translateX(-${l.x * 100}%)` });
    im.onerror = () => { badLayers.add(l.url); draw(); };
    stage.append(im);
  }
  const tb = mk('gtextbox', { height: g.textbox_height + 'px' });
  let nm = null;
  if (speaker) {
    nm = mk('gname', { left: g.name_xpos + 'px', top: g.name_ypos + 'px', fontSize: g.name_text_size + 'px', ...(nfam ? { fontFamily: `"${nfam}", sans-serif` } : {}), ...(sp?.color ? { color: sp.color } : {}) });
    nm.textContent = speaker;
  }
  const dlg = mk('gbox', { left: g.dialogue_xpos + 'px', top: g.dialogue_ypos + 'px', width: g.dialogue_width + 'px' });
  dlg.append(gameText(text));
  placeChars(dlg, mText, g.text_size);
  if (nm) placeChars(nm, mName, g.name_text_size);
  if (nm) tb.append(nm);
  tb.append(dlg);
  stage.append(tb);
  outer.value.replaceChildren(stage);

  const box = outer.value.parentElement;
  const fit = Math.max(200, innerHeight * 0.45) / H;
  const k = box && box.clientWidth ? Math.min(1, (box.clientWidth - 2) / W, fit) : Math.min(1, fit);
  stage.style.transform = `scale(${k})`;
  outer.value.style.height = Math.ceil(H * k) + 'px'; outer.value.style.width = Math.ceil(W * k) + 'px';
  lastSize.w = Math.ceil(W * k); lastSize.h = Math.ceil(H * k);
  const n = Math.max(1, Math.round(dlg.offsetHeight / (g.text_size * 1.3)));
  lines.value = n;
  over.value = n > g.max_lines || g.dialogue_ypos + dlg.offsetHeight > g.textbox_height;

  let where;
  if (!scene.available) where = scene.reason || t('The scene picture is not available.');
  else if (scene.flat) where = t('Background: {image} ({script}).', { image: scene.image, script: scene.script });
  else if (scene.url && imgOk) where = t('Picture: {image} (according to {script}) – an estimate from the game script.', { image: scene.image, script: scene.script });
  else if (scene.found && !scene.url && scene.layers?.length) where = t('No background found; only the pictures shown on the screen are drawn (according to {script}) – an estimate from the game script.', { script: scene.script });
  else where = scene.reason || (scene.found ? t('The picture could not be loaded.') : t('The scene picture could not be found.'));
  const fontNote = !scene.available ? '' : fam ? ' ' + t('Game font: {file}.', { file: scene.fonts_text.file }) : t(' The game font could not be loaded, a substitute is shown.');
  const shown = (scene.layers || []).filter((l) => !badLayers.has(l.url));
  const overNote = shown.length ? ' ' + t('Shown over it: {names}.', { names: shown.map((l) => l.image).join(', ') }) : '';
  const layerNote = scene.layered ? ' ' + t('Layered images are not supported yet: characters made of layers are not drawn here. Support is planned.') : '';
  info.value = `${where}${overNote}${layerNote}${fontNote} ${t('Window {w}×{h}, font {size} px ({source}); limit {lines}.', { w: g.screen_width, h: g.screen_height, size: g.text_size, source: g.source, lines: tn(g.max_lines, '{n} line', '{n} lines') })}`;
}
const later = () => { clearTimeout(timer); timer = setTimeout(draw, 150); };
watch(() => props.text, later);
watch(mode, draw);
const toggled = (open) => { if (open) nextTick(draw); };
onMounted(() => {
  if (outer.value) {                // the size of the previous picture until this one is drawn (no jump); the first time a 16:9 window as wide as the panel
    const w = lastSize.w || Math.min(outer.value.parentElement.clientWidth - 2, 800);
    outer.value.style.width = w + 'px'; outer.value.style.height = (lastSize.h || Math.round(w * 9 / 16)) + 'px';
  }
  draw();
  if (typeof ResizeObserver !== 'undefined' && outer.value?.parentElement) { ro = new ResizeObserver(later); ro.observe(outer.value.parentElement); }
});
onBeforeUnmount(() => { clearTimeout(timer); seq++; if (ro) ro.disconnect(); });
</script>

<template>
  <Fold id="preview" @toggle="toggled">
    <template #title>{{ t('Scene preview ') }}<span v-if="lines" class="muted">(≈ {{ tn(lines, '{n} line', '{n} lines') }}<template v-if="over"> – {{ t('too much!') }}</template>)</span></template>
    <section class="gp">
      <select v-model="mode" class="sel" :title="t('Which text to show in the game window')" :aria-label="t('Which text to show in the game window')">
        <option value="target">{{ t('Translation') }}</option>
        <option value="source">{{ t('Original') }}</option>
      </select>
      <div ref="outer" class="gwin" :class="{ over }"></div>
      <div class="muted small">{{ info }}</div>
      <div class="muted small note">{{ t("The preview is an estimate from the game script, not a screenshot. For games with layered images (characters or backgrounds built from several parts, animations, conditions) it can be wrong or incomplete, and it may not be a good picture of the scene. The font of the text can be a substitute, and the frames of the text box and the name box (their pictures and colours) may look different in the game.") }}</div>
    </section>
  </Fold>
</template>

<style scoped>
.gp { display: flex; flex-direction: column; gap: var(--s-1); align-items: flex-start; }
.gp .gwin { align-self: center; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.small { font-size: var(--fs-small); }
.note { font-size: var(--fs-caption); }
.gp .muted.small:not(.note) { min-height: 3.6em; }
</style>
