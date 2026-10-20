import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get, post } from './api.js';
import KeysPanel from './components/KeysPanel.vue';
import LicencePanel from './components/LicencePanel.vue';
import { ACTIONS, comboOf, findAction, handlers, keysOf, onKeydown, register, saveKeys } from './keys.js';

enableAutoUnmount(afterEach);
const ev = (key, extra = {}) => ({ key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, target: document.body, preventDefault: vi.fn(), ...extra });
const action = (id) => ACTIONS.find((a) => a.id === id);

beforeEach(() => { saveKeys({}); for (const k of Object.keys(handlers)) delete handlers[k]; });

describe('the keys', () => {
  it('writes a key the way the list does', () => {
    expect(comboOf(ev('a'))).toBe('A');
    expect(comboOf(ev('A', { shiftKey: true }))).toBe('Shift+A');
    expect(comboOf(ev('k', { ctrlKey: true }))).toBe('Ctrl+K');
    expect(comboOf(ev('3', { altKey: true }))).toBe('Alt+3');
    expect(comboOf(ev('ArrowDown'))).toBe('ArrowDown');
    expect(comboOf(ev('Shift'))).toBe('');
  });

  it('runs what the open screen registered, and only outside a field unless the action works while typing', () => {
    const next = vi.fn(), palette = vi.fn();
    const off = register({ next, palette });
    onKeydown(ev('ArrowDown'));
    expect(next).toHaveBeenCalledTimes(1);
    const field = document.createElement('textarea');
    onKeydown(ev('ArrowDown', { target: field }));
    expect(next).toHaveBeenCalledTimes(1);                       // typing: the arrow belongs to the field
    onKeydown(ev('k', { ctrlKey: true, target: field }));
    expect(palette).toHaveBeenCalledTimes(1);                    // the palette works while typing
    off();
    onKeydown(ev('ArrowDown'));
    expect(next).toHaveBeenCalledTimes(1);                       // the screen closed: nothing runs
  });

  it('uses the keys the user changed instead of the default ones', () => {
    register({ approve: vi.fn() });
    expect(findAction('A', false)?.id).toBe('approve');
    saveKeys({ approve: ['Ctrl+Y'] });
    expect(keysOf(action('approve'))).toEqual(['Ctrl+Y']);
    expect(findAction('A', false)).toBeUndefined();
    expect(findAction('Ctrl+Y', false)?.id).toBe('approve');
  });

  it('does nothing under an open dialog, and leaves Enter to a focused button', () => {
    const open = vi.fn();
    register({ open_editor: open });
    const dlg = document.createElement('div'); dlg.setAttribute('role', 'dialog'); document.body.append(dlg);
    onKeydown(ev('E'));
    expect(open).not.toHaveBeenCalled();
    dlg.remove();
    const b = document.createElement('button'); document.body.append(b);
    onKeydown(ev('Enter', { target: b }));
    expect(open).not.toHaveBeenCalled();
    onKeydown(ev('Enter'));
    expect(open).toHaveBeenCalledTimes(1);
    b.remove();
  });
});

describe('the shortcuts panel', () => {
  it('captures a new key and asks before it takes a key from another action', async () => {
    const w = mount(KeysPanel, { attachTo: document.body });
    const row = (label) => w.findAll('tbody tr').find((r) => r.text().startsWith(label));
    await row('Search in the current section').findAll('button')[0].trigger('click');          // Change
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true }));
    await flushPromises();
    expect(keysOf(action('search'))).toEqual(['Ctrl+P']);
    await row('Undo the last change').findAll('button')[0].trigger('click');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true }));
    await flushPromises();
    expect(document.body.textContent).toContain('The shortcut is already used');
    const move = [...document.querySelectorAll('[role="dialog"] button')].find((b) => b.textContent === 'Move');
    move.click();
    await flushPromises();
    expect(keysOf(action('undo'))).toEqual(['Ctrl+P']);
    expect(keysOf(action('search'))).toEqual([]);
    w.unmount();
  });

  it('refuses a bare key for an action that works while typing', async () => {
    const w = mount(KeysPanel, { attachTo: document.body });
    await w.findAll('tbody tr').find((r) => r.text().startsWith('Command palette')).findAll('button')[0].trigger('click');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'x', bubbles: true }));
    await flushPromises();
    expect(w.text()).toContain('needs Ctrl or Alt');
    expect(keysOf(action('palette'))).toEqual(['Ctrl+K']);
    w.unmount();
  });
});

describe('the licence panel', () => {
  beforeEach(() => { get.mockReset(); post.mockReset(); post.mockResolvedValue({ ok: true }); });

  it('says that nothing is licensed without the pack', async () => {
    get.mockResolvedValue({ ok: true, state: 'none' });
    const w = mount(LicencePanel);
    await flushPromises();
    expect(w.text()).toContain('The Pro pack is not installed');
  });

  it('activates a key with the name of the computer', async () => {
    get.mockResolvedValue({ ok: true, state: 'free' });
    const w = mount(LicencePanel);
    await flushPromises();
    const inputs = w.findAll('input[type="text"]');
    await inputs[0].setValue('ASIDE-AAAAA-BBBBB-CCCCC-DDDDD');
    await inputs[1].setValue('Home PC');
    await w.findAll('button').find((b) => b.text() === 'Activate').trigger('click');
    await flushPromises();
    expect(post).toHaveBeenCalledWith('licence/activate', { key: 'ASIDE-AAAAA-BBBBB-CCCCC-DDDDD', name: 'Home PC' });
  });

  it('lists the computers of a licence and frees another one after a confirmation', async () => {
    get.mockResolvedValue({ ok: true, state: 'licensed', id: 'L1', days_to_check: 20 });
    post.mockImplementation(async (path) => (path === 'licence/devices' ? { ok: true, max: 2, releases_left: 2, devices: [{ id: 'a', name: 'Home PC', current: true, last_seen: 1760000000 }, { id: 'b', name: 'Laptop', current: false, last_seen: 1750000000 }] } : { ok: true }));
    const w = mount(LicencePanel, { attachTo: document.body });
    await flushPromises();
    expect(w.text()).toContain('Places used: 2 of 2.');
    await w.findAll('button').find((b) => b.text() === 'Remove').trigger('click');
    expect(post).not.toHaveBeenCalledWith('licence/release', expect.anything());
    [...document.querySelectorAll('[role="dialog"] button')].find((b) => b.textContent === 'Remove').click();
    await flushPromises();
    expect(post).toHaveBeenCalledWith('licence/release', { target: 'b' });
    w.unmount();
  });
});
