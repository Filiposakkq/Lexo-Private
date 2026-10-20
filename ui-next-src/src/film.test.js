import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get } from './api.js';
import FocusBar from './components/FocusBar.vue';
import LineDetail from './components/LineDetail.vue';
import ResultsBar from './components/ResultsBar.vue';
import { forgetFileData } from './gamewindow.js';
import { enterQuick, leaveQuick, setLayout, ui } from './layout.js';
import { project } from './project.js';

enableAutoUnmount(afterEach);

const detail = (over = {}) => ({ ok: true, file: 'a.rpy', line: 12, key: ['k'], status: 'translated', kind: '', unit: 'dialogue', who: 'mc', ref: 'game/s.rpy:3', source: 'Hello', target: 'Ahoj', issues: [], terms: [], path: 'a.rpy', block: 'b', proof: '', ...over });
const context = { ok: true, items: [{ line: 11, who: 'mc', status: 'translated', proof: 'ok', source: 'Before', target: 'Předtím' }, { line: 12, who: 'mc', status: 'translated', proof: '', source: 'Hello', target: 'Ahoj', current: true }, { line: 13, who: 'eva', status: 'todo', proof: '', source: 'Next', target: '', over: true }] };

beforeEach(() => {
  get.mockReset(); forgetFileData(); leaveQuick(); setLayout('auto');
  project.files = []; project.names = { mc: 'Alex' }; project.team = null;
  try { localStorage.clear(); } catch { /* none */ }
  get.mockImplementation(async (path) => {
    if (path === 'entry/scene') return context;
    if (path === 'entry/menu') return { ok: true, available: false, reason: 'x' };
    if (path === 'game/scene') return { ok: true, available: false, reason: 'No game folder.' };
    if (path === 'settings') return { ok: true, game_gui: { text_size: 33, dialogue_width: 1116, textbox_height: 278, dialogue_xpos: 402, dialogue_ypos: 75, name_xpos: 360, name_ypos: 0, name_text_size: 45, source: 'gui.rpy' }, validation: { textbox: { max_lines: 4 } } };
    return { ok: true, available: false, items: [] };
  });
});

describe('the scene and film strip layout', () => {
  it('puts the picture over the editor, the strip of lines under it and takes the Scene tab away', async () => {
    const w = mount(LineDetail, { props: { detail: detail(), layout: 'film' } });
    await flushPromises();
    expect(w.find('.ld').classes()).toContain('film');
    expect(w.find('.gwin').exists()).toBe(true);
    expect(w.findAll('.fcard')).toHaveLength(3);
    expect(w.find('.fcard.cur').text()).toContain('Ahoj');
    expect(w.findAll('.tab').some((b) => b.text() === 'Scene')).toBe(false);
    expect(w.find('.fcard.over').exists()).toBe(true);                           // the line that does not fit is marked
    await w.findAll('.fcard')[0].trigger('click');
    expect(w.emitted('goto')[0]).toEqual([{ file: 'a.rpy', line: 11 }]);
  });

  it('has no tabs in quick translating, and only the proofreading box in quick proofreading', async () => {
    const tr = mount(LineDetail, { props: { detail: detail({ status: 'todo' }), layout: 'film', quick: 'tr' } });
    await flushPromises();
    expect(tr.find('.tabs').exists()).toBe(false);
    expect(tr.find('.pane').exists()).toBe(false);
    const proof = mount(LineDetail, { props: { detail: detail(), layout: 'film', quick: 'proof' } });
    await flushPromises();
    expect(proof.find('.tabs').exists()).toBe(false);
    expect(proof.find('.pane').text()).toContain('Proofread');
  });

  it('keeps the usual detail (tabs, Scene tab) in the other layouts', async () => {
    const w = mount(LineDetail, { props: { detail: detail(), layout: '' } });
    await flushPromises();
    expect(w.find('.ld').classes()).not.toContain('film');
    expect(w.findAll('.tab').some((b) => b.text() === 'Scene')).toBe(true);
    expect(w.find('.fcard').exists()).toBe(false);
  });
});

describe('the quick modes, the focus bar and the results bar', () => {
  it('enters the quick modes: film layout, focus, and the tab the mode needs', () => {
    enterQuick('proof');
    expect(ui.layout).toBe('film'); expect(ui.focus).toBe(true); expect(ui.quick).toBe('proof');
    expect(localStorage.getItem('aside.detailTab')).toBe('proof');
    enterQuick('tr');
    expect(ui.quick).toBe('tr'); expect(localStorage.getItem('aside.detailTab')).toBe('check');
    leaveQuick();
    expect(ui.focus).toBe(false); expect(ui.quick).toBe('');
  });

  it('asks the screen to do what its buttons say, and shows what is left', async () => {
    enterQuick('proof');
    const w = mount(FocusBar, { props: { left: { review: 9 } } });
    expect(w.text()).toContain('left to proofread: 9');
    await w.findAll('button').find((b) => b.text() === '✓ Proofread').trigger('click');
    await w.findAll('button').find((b) => b.text() === '⚑ Needs a fix').trigger('click');
    await w.findAll('button').find((b) => b.text() === 'Translation').trigger('click');
    expect(w.emitted('act').map((a) => a[0])).toEqual(['proof', 'flag', 'quick']);
    await w.findAll('button').find((b) => b.text().includes('Show everything')).trigger('click');
    expect(ui.focus).toBe(false);
  });

  it('shows "Result n of m" and moves between the hits', async () => {
    const items = [1, 2, 3].map((n) => ({ file: 'a.rpy', line: n, status: 'translated', who: 'mc', source: 'S' + n, target: 'T' + n }));
    const w = mount(ResultsBar, { props: { items, total: 3, offset: 0, sel: 1, summary: 'Search: “o”' } });
    expect(w.text()).toContain('Result 2 of 3');
    await w.findAll('button.icon')[1].trigger('click');
    expect(w.emitted('select')[0]).toEqual([2]);
    await w.findAll('button').find((b) => b.text() === 'Show the list').trigger('click');
    expect(w.findAll('.resrow')).toHaveLength(3);
    await w.findAll('.resrow')[0].trigger('click');
    expect(w.emitted('select')[1]).toEqual([0]);
  });
});
