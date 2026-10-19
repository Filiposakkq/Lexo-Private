import { t as T } from './05-i18n.js';
import { $, h } from './00-core.js';
import { S } from './11-state.js';
/* ---------------------------------------------------------------- feedback */
export function status(msg, kind) {
  const el = $('#statusText');
  el.textContent = msg;
  el.className = kind || '';
}
export function toast(msg, kind = 'ok', ms = 4500) {
  const t = h('div', { class: 'toast ' + kind }, h('span', null, msg), h('button', { class: 'x', title: T('Close'), onclick: () => t.remove() }, '✕'));
  $('#toasts').append(t);
  if (ms) setTimeout(() => t.remove(), kind === 'bad' ? ms * 2 : ms);
  status(msg, kind === 'bad' ? 'bad' : 'ok');
}
export const fail = (e) => toast(e.message || String(e), 'bad');

export function dialog({ title, body, buttons = [], narrow = false, onclose }) {
  const dlg = $('#dlg');
  dlg.className = narrow ? 'narrow' : '';
  dlg.replaceChildren(h('div', { class: 'dlg' },
    h('div', { class: 'dlg-head' }, title),
    h('div', { class: 'dlg-body' }, body),
    h('div', { class: 'dlg-foot' }, ...buttons.map((b) => b.node || h('button', {
      class: 'btn ' + (b.kind || ''), disabled: b.disabled,
      onclick: async () => { const keep = await b.onclick?.(); if (keep !== false) close(); },
    }, b.label)))));
  const close = () => { if (dlg.open) dlg.close(); };
  dlg.onclose = () => { S.dlgDirty = false; onclose?.(); };
  dlg.showModal();
  return close;
}
export const dialogOpen = () => $('#dlg').open;
export function confirmDialog(title, text, okLabel = T('Continue')) {
  return new Promise((resolve) => {
    let answered = false;
    dialog({ title, narrow: true, body: h('p', null, text),
      buttons: [{ label: T('Cancel'), onclick: () => { answered = true; resolve(false); } },
                { label: okLabel, kind: 'primary', onclick: () => { answered = true; resolve(true); } }],
      onclose: () => { if (!answered) resolve(false); } });
  });
}

