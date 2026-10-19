import { t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
/* The profile of a character (a Pro feature): the gender, the way the character speaks and the form in which they address the other characters. The batch machine translation
   sends it to the language model with every line of that speaker. It is kept in the note of the glossary entry (the glossary page does not show it). */
export async function profileDialog(code, name) {
  let list;
  try { list = await post('profile/list', {}); } catch (e) { return fail(e); }
  const me = list.items.find((i) => i.code === code) || { profile: { gender: '', style: '', to: {} } };
  const others = list.items.filter((i) => i.code !== code && (i.in_project || me.profile.to[i.code])).slice(0, 40);
  const gender = h('select', null, [['', T('not set')], ['f', T('female')], ['m', T('male')], ['n', T('neither')]].map(([v, l]) => h('option', { value: v, selected: me.profile.gender === v }, l)));
  const style = h('input', { type: 'text', value: me.profile.style || '', maxlength: '200', placeholder: T('e.g. sarcastic, short sentences, speaks slowly'), style: { width: '100%' } });
  const forms = {};
  const rows = others.map((o) => {
    const sel = h('select', null, [['', T('not set')], ['informal', T('informal (tykání)')], ['formal', T('formal (vykání)')]].map(([v, l]) => h('option', { value: v, selected: (me.profile.to[o.code] || '') === v }, l)));
    forms[o.code] = sel;
    return h('tr', null, h('td', { class: 'mono' }, o.code), h('td', null, o.name || ''), h('td', null, sel));
  });
  dialog({
    title: T('Profile of {name}', { name: name || code }),
    body: h('div', null,
      h('p', { class: 'muted' }, T('The batch machine translation tells the language model who speaks: it keeps the feminine and masculine forms right and the form of address (tykání / vykání) the same. Only a language model (OpenAI-compatible service) can use it; DeepL and LibreTranslate cannot.')),
      h('div', { class: 'row', style: { alignItems: 'center', gap: '8px', margin: '6px 0' } }, h('label', null, T('Gender')), gender),
      h('div', { class: 'row', style: { alignItems: 'center', gap: '8px', margin: '6px 0' } }, h('label', null, T('How the character speaks')), style),
      rows.length ? h('div', null, h('b', null, T('Addresses the others')), h('div', { class: 'tablewrap', style: { maxHeight: '36vh', marginTop: '6px' } }, h('table', { class: 'data' },
        h('thead', null, h('tr', null, [T('Code'), T('Name'), T('Form')].map((x) => h('th', null, x)))), h('tbody', null, rows)))) : h('p', { class: 'muted' }, T('There are no other speakers in the project yet.'))),
    buttons: [{ label: T('Cancel') }, { label: T('Save'), kind: 'primary', onclick: async () => {
      const to = {};
      for (const [c, sel] of Object.entries(forms)) if (sel.value) to[c] = sel.value;
      try { await post('profile/set', { code, gender: gender.value, style: style.value.trim(), to }); toast(T('Saved ✓'), 'ok', 1500); } catch (e) { fail(e); return false; }
    } }],
  });
}
