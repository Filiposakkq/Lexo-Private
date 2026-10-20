<script setup>
import { t } from '../i18n.js';
import { speakerLabel } from '../project.js';

/* The menu a line belongs to (read from the game script): the question or the lines said before it, all choices with their translations, the conditions, and the first lines after each
   choice, because what a choice means is what follows it. The open line is marked; a click on any line opens it. */
defineProps({ menu: { type: [Object, String], default: null } });
const emit = defineEmits(['goto']);
const text = (e) => (e.status === 'todo' ? t('untranslated') : e.target);
</script>

<template>
  <div>
    <div v-if="!menu || menu === 'loading'" class="muted">{{ t('Loading…') }}</div>
    <div v-else-if="menu.error || !menu.available" class="muted">{{ menu.error || menu.reason }}</div>
    <template v-else>
      <div class="muted mono small">{{ t('Game script: ') }}{{ menu.script }}</div>
      <h4>{{ menu.question.length ? t('Question') : t('Said before the menu') }}</h4>
      <div v-for="(x, i) in (menu.question.length ? menu.question : menu.before)" :key="'q' + i" class="mline" :class="{ cur: x.here }" @click="x.entry && emit('goto', { file: x.entry.file, line: x.entry.line })">
        <template v-if="x.entry"><b>{{ speakerLabel(x.entry.who) }}</b> <span v-if="x.here" class="pill none">{{ t('this line') }}</span><div>{{ text(x.entry) }}</div><div class="muted">{{ x.entry.source }}</div></template>
        <span v-else class="muted mono">{{ x.script }}</span>
      </div>
      <h4>{{ t('Choices') }}</h4>
      <div v-for="(o, i) in menu.options" :key="'o' + i" class="mopt" :class="{ cur: o.current }">
        <div class="mline" @click="o.entry && emit('goto', { file: o.entry.file, line: o.entry.line })">
          <span class="pill none">{{ t('Choice') }}</span> <span v-if="o.cond" class="pill none" :title="o.cond">{{ t('if {cond}', { cond: o.cond }) }}</span> <span v-if="o.current" class="pill none">{{ t('this line') }}</span>
          <div>{{ o.entry ? text(o.entry) : t('(not found in the translation)') }}</div><div class="muted">{{ o.text }}</div>
          <div v-if="o.where" class="muted small">{{ t('The same text is used in an earlier menu ({ref}): all of them share one translation, and the line is listed there.', { ref: o.where }) }}</div>
        </div>
        <div v-if="!o.after.length && o.body && o.body.length" class="muted small"><div>{{ t('No dialogue follows; the script continues with:') }}</div><pre class="code">{{ o.body.join('\n') }}</pre></div>
        <div v-if="o.after.length" class="after"><div class="muted small">{{ t('then:') }}</div>
          <div v-for="(x, j) in o.after" :key="j" class="mline" @click="x.entry && emit('goto', { file: x.entry.file, line: x.entry.line })"><template v-if="x.entry"><b>{{ speakerLabel(x.entry.who) }}</b><div>{{ text(x.entry) }}</div><div class="muted">{{ x.entry.source }}</div></template><span v-else class="muted mono">{{ x.script }}</span></div></div>
      </div>
    </template>
  </div>
</template>

<style scoped>
h4 { margin: var(--s-2) 0 var(--s-1); font-size: var(--fs-body); font-weight: 500; color: var(--text-2); }
.mline { padding: var(--s-2); border-radius: var(--r-md); cursor: pointer; font-size: var(--fs-body); }
.mline:hover { background: var(--bg-hover); }
.mline.cur, .mopt.cur > .mline { background: var(--bg-hover); box-shadow: inset 3px 0 0 var(--accent-hi); }
.after { margin-left: var(--s-4); }
.small { font-size: var(--fs-small); }
.code { margin: var(--s-2) 0 0; padding: var(--s-3); background: var(--bg-code); border-radius: var(--r-md); font-family: var(--mono); font-size: var(--fs-small); overflow: auto; }
</style>
