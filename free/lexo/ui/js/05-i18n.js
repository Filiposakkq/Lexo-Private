/* ---------------------------------------------------------------- interface texts
   The English text IS the key: t('Overview'). English needs no table at all; another language is a pack with a ui.json
   ({"strings": {"Overview": "Přehled", "{n} lines": ["{n} řádek", ...]}}) that the server hands out (/api/ui-lang).
   A text without a translation is shown in English. Placeholders are {name}: t('Saved {file}', { file }).
   Counted texts: tn(n, 'one line', '{n} lines') - the key is "one line|{n} lines", the pack gives one text per plural form.
   This module has no imports, so every other module can use it. */

// number -> index of the plural form; the same table is in lang.py (PLURAL_RULES): keep them equal
const RULES = {
  en: [2, (n) => (n === 1 ? 0 : 1)],
  cs: [3, (n) => (n === 1 ? 0 : n >= 2 && n <= 4 ? 1 : 2)],
  sk: [3, (n) => (n === 1 ? 0 : n >= 2 && n <= 4 ? 1 : 2)],
  pl: [3, (n) => (n === 1 ? 0 : n % 10 >= 2 && n % 10 <= 4 && !(n % 100 >= 12 && n % 100 <= 14) ? 1 : 2)],
  ru: [3, (n) => (n % 10 === 1 && n % 100 !== 11 ? 0 : n % 10 >= 2 && n % 10 <= 4 && !(n % 100 >= 12 && n % 100 <= 14) ? 1 : 2)],
  fr: [2, (n) => (n === 0 || n === 1 ? 0 : 1)],
};

let strings = {};
let ruleName = 'en';
let current = 'en';

export const pluralIndex = (rule, n) => {
  const [forms, fn] = RULES[rule] || RULES.en;
  return Math.min(fn(Math.abs(Math.trunc(n))), forms - 1);
};

function fill(text, params) {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
}

export function t(text, params) {
  let s = strings[text];
  if (Array.isArray(s)) s = s[0];
  return fill(typeof s === 'string' ? s : text, params);
}

/* Marks a text for translation where it is written down but shown later (tables built at load time): the text is returned as it is
   and translated with t() where it is displayed. */
export const N_ = (text) => text;

export function tn(n, one, many, params) {
  const all = { n, ...params };
  const forms = strings[one + '|' + many];
  if (Array.isArray(forms) && forms.length) return fill(forms[Math.min(pluralIndex(ruleName, n), forms.length - 1)], all);
  return fill(n === 1 ? one : many, all);
}

/* Like t(), but the values may be DOM nodes: returns an array of text and nodes, for h(...) children.
   tx('Folder {path} is used', { path: h('b', null, p) }) */
export function tx(text, params = {}) {
  const out = [];
  let last = 0;
  const src = t(text);
  src.replace(/\{(\w+)\}/g, (m, k, at) => {
    if (at > last) out.push(src.slice(last, at));
    out.push(k in params ? params[k] : m);
    last = at + m.length;
    return m;
  });
  if (last < src.length) out.push(src.slice(last));
  return out;
}

/* A table of fixed labels that is translated when it is READ, not when the module is loaded (the language is chosen later):
   const STATUS = labels({ done: 'Done' }); STATUS.done -> t('Done') */
export function labels(table) {
  return new Proxy(table, { get: (target, key) => (typeof target[key] === 'string' ? t(target[key]) : target[key]) });
}

export const uiLanguage = () => current;
/* BCP 47 tag for dates and numbers */
export const locale = () => ({ en: 'en-GB', cs: 'cs-CZ', sk: 'sk-SK', pl: 'pl-PL', ru: 'ru-RU', fr: 'fr-FR' }[current] || current);

export function setLanguage(pack) {
  strings = (pack && pack.strings) || {};
  ruleName = (pack && pack.plural) || 'en';
  current = (pack && pack.code) || 'en';
  document.documentElement.lang = current;
  applyStatic();
}

/* Static texts of index.html: data-i18n="English text" (the text), data-i18n-title / data-i18n-aria (attributes). */
export function applyStatic(root = document) {
  for (const el of root.querySelectorAll('[data-i18n]')) el.textContent = t(el.dataset.i18n);
  for (const el of root.querySelectorAll('[data-i18n-title]')) el.title = t(el.dataset.i18nTitle);
  for (const el of root.querySelectorAll('[data-i18n-aria]')) el.setAttribute('aria-label', t(el.dataset.i18nAria));
}

/* The language to start with: ?lang= in the address (not remembered), then the saved choice, then the browser's language. */
export function chooseLanguage(saved, available) {
  const codes = new Set(available.map((l) => l.code));
  const asked = new URLSearchParams(location.search).get('lang');
  if (asked && codes.has(asked)) return asked;
  if (saved && codes.has(saved)) return saved;
  for (const l of navigator.languages || [navigator.language || 'en']) {
    const base = String(l).toLowerCase().split('-')[0];
    if (codes.has(base)) return base;
  }
  return 'en';
}

export async function loadLanguage(code) {
  if (!code || code === 'en') return { code: 'en', plural: 'en', strings: {} };
  const res = await fetch('/api/ui-lang?code=' + encodeURIComponent(code), { cache: 'no-store' });
  const body = await res.json();
  if (!res.ok || !body.ok) throw new Error(body.error || 'language not available');
  return body;
}

export async function availableLanguages() {
  try {
    const res = await fetch('/api/ui-langs', { cache: 'no-store' });
    const body = await res.json();
    return body.langs || [{ code: 'en', name: 'English' }];
  } catch {
    return [{ code: 'en', name: 'English' }];
  }
}
