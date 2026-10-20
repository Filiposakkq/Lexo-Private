import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })), TOKEN: 't' }));
import { get, post } from './api.js';
import LineDetail from './components/LineDetail.vue';
import { forgetFileData } from './gamewindow.js';
import { project } from './project.js';
import { announceDraft, announceFocus, openDetailWindow, startFollowing, startPings, win } from './win.js';

enableAutoUnmount(afterEach);
const detail = (over = {}) => ({ ok: true, file: 'a.rpy', line: 12, key: ['k'], status: 'translated', kind: '', unit: 'dialogue', who: 'mc', ref: 'game/s.rpy:3', source: 'Hello', target: 'Ahoj', issues: [], terms: [], path: 'a.rpy', block: 'b', proof: '', ...over });

beforeEach(() => {
  get.mockReset(); post.mockReset(); post.mockResolvedValue({ ok: true }); forgetFileData();
  win.companionOpen = false; win.editor = false; win.full = false; win.slim = true; win.follow = true; win.lost = false;
  project.files = []; project.names = {}; project.team = null;
  try { localStorage.clear(); } catch { /* none */ }
  get.mockImplementation(async (path) => (path === 'entry/scene' ? { ok: true, items: [] } : path === 'game/scene' ? { ok: true, available: false, reason: 'x' } : { ok: true, available: false, items: [] }));
});

describe('the detail window (the second window)', () => {
  it('has the scene and the checks but no editor, no approving and no memory or history', async () => {
    const w = mount(LineDetail, { props: { detail: detail(), companion: true } });
    await flushPromises();
    expect(w.find('.pair').exists()).toBe(false);
    expect(w.find('.actions.top').exists()).toBe(false);
    expect(w.find('.filestrip').exists()).toBe(false);
    const tabs = w.findAll('.tab').map((b) => b.text());
    expect(tabs).toContain('Scene');
    expect(tabs).not.toContain('Memory');
    expect(tabs).not.toContain('History');
  });

  it('gets the editor, and the memory and history, when you ask for the editor here too', async () => {
    win.editor = true;
    const w = mount(LineDetail, { props: { detail: detail(), companion: true } });
    await flushPromises();
    expect(w.find('.pair textarea').exists()).toBe(true);
    expect(w.findAll('.tab').map((b) => b.text())).toEqual(expect.arrayContaining(['Memory', 'History']));
  });

  it('shows the picture over the whole window and nothing else when asked', async () => {
    win.full = true;
    const w = mount(LineDetail, { props: { detail: detail(), companion: true } });
    await flushPromises();
    expect(w.find('.gwin').exists()).toBe(true);
    expect(w.find('.tabs').exists()).toBe(false);
    expect(w.find('.pair').exists()).toBe(false);
  });

  it('shows what is typed in the main window in its preview', async () => {
    const w = mount(LineDetail, { props: { detail: detail(), companion: true } });
    await flushPromises();
    window.dispatchEvent(new CustomEvent('aside:draft', { detail: 'Napsáno jinde' }));
    await flushPromises();
    await w.findAll('.tab').find((b) => b.text() === 'Scene').trigger('click');
    await flushPromises(); await new Promise((r) => setTimeout(r, 250)); await flushPromises();
    expect(w.find('.gbox').text()).toContain('Napsáno jinde');
  });
});

describe('the main window while a detail window is open', () => {
  it('slims down to the original and the editor, says where the rest is, and gives the tabs back on request', async () => {
    const w = mount(LineDetail, { props: { detail: detail(), slim: true } });
    await flushPromises();
    expect(w.find('.slimnote').exists()).toBe(true);
    expect(w.find('.tabs').exists()).toBe(false);
    expect(w.find('.pair textarea').exists()).toBe(true);
    await w.find('.slimnote button').trigger('click');
    expect(win.slim).toBe(false);
  });

  it('announces what is typed, only while a second window is open', async () => {
    vi.useFakeTimers();
    announceDraft('a.rpy', 12, 'no one listens');
    vi.advanceTimersByTime(300);
    expect(post).not.toHaveBeenCalled();
    win.companionOpen = true;
    announceDraft('a.rpy', 12, 'Ahoj světe');
    vi.advanceTimersByTime(300);
    expect(post).toHaveBeenCalledWith('focus', { file: 'a.rpy', line: 12, text: 'Ahoj světe' });
    vi.useRealTimers();
  });
});

describe('the windows and the server', () => {
  it('announces only the last line of a run, after a pause', async () => {
    vi.useFakeTimers();
    announceFocus('a.rpy', 1); announceFocus('a.rpy', 2); announceFocus('a.rpy', 3);
    vi.advanceTimersByTime(300);
    expect(post.mock.calls.filter((c) => c[0] === 'focus')).toEqual([['focus', { file: 'a.rpy', line: 3 }]]);
    vi.useRealTimers();
  });

  it('follows the line the main window announces through the server', async () => {
    const seen = [];
    let n = 0;
    get.mockImplementation(async (path) => {
      if (path !== 'focus') return { ok: true };
      if (n++ === 0) return { ok: true, seq: 1, file: 'a.rpy', line: 7, text: null };
      await new Promise((r) => setTimeout(r, 20));
      return { ok: true, seq: 1, file: 'a.rpy', line: 7, text: null };
    });
    const stop = startFollowing((f) => seen.push(f));
    await new Promise((r) => setTimeout(r, 60));
    stop();
    expect(seen).toHaveLength(1);
    expect(seen[0]).toMatchObject({ file: 'a.rpy', line: 7 });
  });

  it('learns from the ping that a second window is open, and opens one in the new interface', async () => {
    get.mockResolvedValue({ ok: true, companions: 1 });
    const stop = startPings();
    await flushPromises();
    expect(win.companionOpen).toBe(true);
    stop();
    await openDetailWindow({ file: 'a.rpy', line: 5 });
    expect(post).toHaveBeenCalledWith('window/open', expect.objectContaining({ companion: '1', ui: 'next', file: 'a.rpy', line: 5 }));
  });
});
