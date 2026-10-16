"use strict";
/* Lexo – UI. Vanilla JS, no build step. All data is inserted with textContent
   (the h() helper), never innerHTML, so file contents can never inject markup. */

const TOKEN = document.querySelector('meta[name=token]').content;
const $ = (s, r = document) => r.querySelector(s);

/* append()/replaceChildren() turn null into the text "null"; conditional children are common here,
   so skip null / undefined / false everywhere. */
for (const m of ['append', 'prepend', 'replaceChildren']) {
  const native = Element.prototype[m];
  Element.prototype[m] = function (...kids) {
    return native.apply(this, kids.flat(Infinity).filter((k) => k !== null && k !== undefined && k !== false));
  };
}

function h(tag, props, ...kids) {
  const el = document.createElement(tag);
  if (props) {
    for (const [k, v] of Object.entries(props)) {
      if (v === undefined || v === null || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
      else if (k === 'dataset') Object.assign(el.dataset, v);
      else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (['checked', 'disabled', 'selected', 'readOnly', 'multiple'].includes(k)) el[k] = v;
      else if (k === 'value') el.value = v;
      else el.setAttribute(k, v === true ? '' : v);
    }
  }
  for (const kid of kids.flat(Infinity)) {
    if (kid === null || kid === undefined || kid === false) continue;
    el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
  }
  if (props && props.value !== undefined && tag === 'select') el.value = props.value;
  return el;
}

const nf = new Intl.NumberFormat('cs-CZ');
const plural = (n, one, few, many) => (n === 1 ? one : n >= 2 && n <= 4 ? few : many);     // Czech: 1 řádek, 2–4 řádky, 5 řádků
const fmtN = (n) => nf.format(n ?? 0);
const fmtPct = (p) => (p === null || p === undefined ? '–' : p.toFixed(1).replace('.', ',') + ' %');
const clamp01 = (v) => Math.max(0, Math.min(100, v || 0));
const dateCz = (iso) => iso ? new Date(iso + 'T12:00:00').toLocaleDateString('cs-CZ', { day: 'numeric', month: 'long', year: 'numeric' }) : '';

