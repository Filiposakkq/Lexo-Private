import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get } from './api.js';
import LineDetail from './components/LineDetail.vue';
import { forgetFileData, gameText } from './gamewindow.js';
import { project } from './project.js';

enableAutoUnmount(afterEach);

describe('the text of the game window', () => {
  it('draws the tags the game draws and hides the others', () => {
    const el = gameText('Hi {i}there{/i}\\n[name] {w}{color=#f00}red{/color}');
    expect(el.querySelector('i').textContent).toBe('there');
    expect(el.querySelector('br')).not.toBeNull();
    expect(el.textContent).toContain('XXXXX');
    expect(el.textContent).not.toContain('{w}');
    expect(el.querySelector('span').style.color).not.toBe('');
  });
});

const detail = (over = {}) => ({ ok: true, file: 'a.rpy', line: 12, key: ['k'], status: 'translated', kind: '', unit: 'dialogue', who: 'mc', ref: 'game/s.rpy:3', source: 'Hello', target: 'Ahoj', issues: [], terms: [], path: 'a.rpy', block: 'b', ...over });
const menu = { ok: true, available: true, script: 'game/s.rpy', question: [], before: [{ entry: { file: 'a.rpy', line: 12, who: 'mc', status: 'translated', target: 'Ahoj', source: 'Hello' }, here: true }], options: [{ n: 1, text: 'Yes', cond: '', current: false, entry: { file: 'a.rpy', line: 20, who: '', status: 'todo', target: '', source: 'Yes' }, after: [], body: [] }] };

describe('the line detail: scene and menu', () => {
  beforeEach(() => {
    get.mockReset(); forgetFileData();
    project.files = []; project.names = { mc: 'Alex' }; project.team = null;
    get.mockImplementation(async (path) => {
      if (path === 'entry/menu') return menu;
      if (path === 'entry/scene') return { ok: true, items: [{ line: 11, who: 'mc', status: 'translated', source: 'Before', target: 'Předtím' }, { line: 12, who: 'mc', status: 'translated', source: 'Hello', target: 'Ahoj', current: true }] };
      if (path === 'game/scene') return { ok: true, available: false, reason: 'No game folder.' };
      if (path === 'settings') return { ok: true, game_gui: { text_size: 33, dialogue_width: 1116, textbox_height: 278, dialogue_xpos: 402, dialogue_ypos: 75, name_xpos: 360, name_ypos: 0, name_text_size: 45, source: 'gui.rpy' }, validation: { textbox: { max_lines: 4 } } };
      if (path === 'scenes') return { ok: true, available: false, items: [] };
      return { ok: true };
    });
    try { localStorage.clear(); } catch { /* none */ }
  });

  it('gives a line of dialogue that belongs to a menu the Menu tab, with the speaker named from the glossary', async () => {
    const w = mount(LineDetail, { props: { detail: detail() } });
    await flushPromises();
    const tab = w.findAll('.tab').find((b) => b.text() === 'Menu');
    expect(tab).toBeTruthy();
    await tab.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('Alex · mc');
    expect(w.text()).toContain('this line');
  });

  it('has no Menu tab for a line of dialogue outside a menu', async () => {
    get.mockImplementation(async (path) => (path === 'entry/menu' ? { ok: true, available: false, reason: 'x' } : { ok: true, items: [], available: false }));
    const w = mount(LineDetail, { props: { detail: detail() } });
    await flushPromises();
    expect(w.findAll('.tab').some((b) => b.text() === 'Menu')).toBe(false);
  });

  it('shows the game window and the lines around, as a table or as bubbles', async () => {
    const w = mount(LineDetail, { props: { detail: detail() } });
    await flushPromises();
    await w.findAll('.tab').find((b) => b.text() === 'Scene').trigger('click');
    await flushPromises();
    expect(w.find('.gwin').exists()).toBe(true);
    expect(w.text()).toContain('No game folder.');
    expect(w.find('table.scene').exists()).toBe(true);
    await w.findAll('.seg button').find((b) => b.text() === 'Bubbles').trigger('click');
    expect(w.findAll('.bubble')).toHaveLength(2);
    expect(w.find('.bubble.cur').text()).toContain('Ahoj');
  });
});

describe('sections that collapse', () => {
  it('remembers whether a section was left open, by its id', async () => {
    const Fold = (await import('./components/Fold.vue')).default;
    try { localStorage.clear(); } catch { /* none */ }
    const make = () => mount(Fold, { props: { id: 'demo' }, slots: { title: 'Title', default: '<p class="in">inside</p>' } });
    const a = make();
    expect(a.find('.in').exists()).toBe(true);
    a.find('details').element.open = false;
    await a.find('details').trigger('toggle');
    expect(a.find('.in').exists()).toBe(false);
    const b = make();                                  // the next line: the same section starts closed
    expect(b.find('details').element.open).toBe(false);
    expect(b.find('.in').exists()).toBe(false);
  });
});
