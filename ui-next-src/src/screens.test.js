import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get, post } from './api.js';
import Files from './views/Files.vue';
import Lines from './views/Lines.vue';
import Overview from './views/Overview.vue';
import { openLines, project } from './project.js';

const file = (name, over = {}) => ({ name, total: 100, done: 40, corr: 10, todo: 60, auto: 40, approved: 0, proof_marked: 0, proof_fix: 0, proof_stale: 0, assign: '', status: 'wip', order: 1, errors: 0, warnings: 0, ...over });
const overview = () => ({
  project: { name: 'Eternum', files: 2 }, totals: { total: 200, done: 80, approved: 0, percent: 40, remaining: 120, proofread: 20, proofread_percent: 10, proof_marked: 0, proof_fix: 3, proof_stale: 0 },
  file_counts: { done: 0, rev: 0, tr: 0, wip: 2, none: 0 }, validation: { error: 0, warning: 0, length: 0, ignored: 0 }, estimate: {}, groups_defined: false, episodes: [],
  general: { translation: { total: 0, done: 0 } }, unassigned: { translation: { total: 200, done: 80 } }, recent: [], unsaved: [], worst_files: [],
});

enableAutoUnmount(afterEach);
beforeEach(() => { get.mockReset(); post.mockReset(); post.mockResolvedValue({ ok: true }); project.files = null; project.overview = null; });

describe('Overview', () => {
  it('shows where you are and opens the lines to continue with', async () => {
    get.mockImplementation(async (path) => (path === 'overview' ? { ok: true, ...overview() } : { ok: true, files: [file('a.rpy'), file('b.rpy', { todo: 10, done: 90 })] }));
    const w = mount(Overview);
    await flushPromises();
    expect(w.text()).toContain('120 lines left to translate');
    expect(w.text()).toContain('40,0 %'.replace(',', w.text().includes('40,0') ? ',' : '.'));
    await w.findAll('button').find((b) => b.text().startsWith('Closest to done')).trigger('click');
    expect(project.lines.file).toBe('b.rpy');
    expect(w.emitted('go')[0]).toEqual(['lines']);
  });
});

describe('Files', () => {
  const load = async () => {
    get.mockImplementation(async (path) => (path === 'overview' ? { ok: true, ...overview() } : { ok: true, files: [file('script2.rpy', { todo: 5, done: 95, status: 'tr' }), file('script10.rpy'), file('chat.rpy', { assign: 'gen' })] }));
    const w = mount(Files);
    await flushPromises();
    return w;
  };
  const names = (w) => w.findAll('tbody tr .name span:first-child').map((s) => s.text());
  it('lists the files, sorts them by a column and filters by the search text', async () => {
    const w = await load();
    expect(names(w)).toEqual(['chat.rpy', 'script2.rpy', 'script10.rpy']);
    await w.findAll('thead .th').find((b) => b.text().startsWith('Left to translate')).trigger('click');
    expect(names(w)[0]).toBe('script2.rpy');
    await w.find('input[type="search"]').setValue('chat');
    expect(names(w)).toEqual(['chat.rpy']);
  });
  it('saves an assignment and opens the lines of a file with a double click', async () => {
    const w = await load();
    await w.find('tbody tr select').setValue('gen');
    expect(post).toHaveBeenCalledWith('assign', { name: 'chat.rpy', v: 'gen' });
    await w.find('tbody tr').trigger('dblclick');
    expect(project.lines.file).toBe('chat.rpy');
    expect(w.emitted('go').pop()).toEqual(['lines']);
  });
});

describe('Lines', () => {
  const item = (line, over = {}) => ({ file: 'chat.rpy', line, key: 'k' + line, status: 'todo', proof: '', kind: 'english', unit: 'dialogue', who: 'e', ref: 'game/script.rpy:' + line, source: 'Hello ' + line, target: 'Hello ' + line, issues: [], ...over });
  const setup = async () => {
    project.files = [file('chat.rpy')]; project.names = { e: 'Eileen' };
    openLines({ file: '', status: 'todo' });
    get.mockImplementation(async (path, q) => {
      if (path === 'entries') return { ok: true, items: [item(1), item(2)], total: 2, offset: 0, counts: { all: 2, todo: 2 } };
      if (path === 'entry') return { ok: true, ...item(q.line), path: 'game/tl/czech/chat.rpy' };
      if (path === 'glossary') return { ok: true, names: { e: 'Eileen' } };
      return { ok: true };
    });
    const w = mount(Lines);
    await flushPromises(); await new Promise((r) => setTimeout(r, 0)); await flushPromises();
    return w;
  };
  it('shows every column of a line, including the speaker name from the glossary', async () => {
    const w = await setup();
    const cells = w.find('tbody tr').findAll('td').map((c) => c.text());
    expect(cells).toContain('1');
    expect(cells).toContain('script.rpy:1');
    expect(cells).toContain('e');
    expect(cells).toContain('Eileen');
    expect(w.findAll('thead th').map((h) => h.text())).toEqual(expect.arrayContaining(['File', 'Line', 'Script', 'Type', 'State', 'Proofreading', 'Code', 'Name', 'Original', 'Translation', 'Check']));
  });
  it('saves the edited translation to the file', async () => {
    const w = await setup();
    await w.find('tbody tr').trigger('click');
    await flushPromises();
    await w.find('textarea').setValue('Ahoj 1');
    post.mockImplementation(async (path) => (path === 'entry/check' ? { ok: true, issues: [], literal_error: '' } : { ok: true }));
    await new Promise((r) => setTimeout(r, 400));
    await flushPromises();
    await w.findAll('button').find((b) => b.text().startsWith('Save to the file')).trigger('click');
    await flushPromises();
    expect(post).toHaveBeenCalledWith('entry/save', { file: 'chat.rpy', line: 1, text: 'Ahoj 1' });
  });
  it('approves optimistically: the next line opens before the mark is written, and a failed write puts the row back', async () => {
    const w = await setup();
    await w.find('tbody tr').trigger('click');
    await flushPromises();
    const order = [];
    get.mockImplementation(async (path, q) => {
      if (path === 'entry') { order.push('get entry ' + q.line); return { ok: true, ...item(q.line), path: 'game/tl/czech/chat.rpy' }; }
      return { ok: true };
    });
    let fail = false;
    post.mockImplementation(async (path) => { if (path === 'approve/apply') { order.push('write'); if (fail) throw new Error('disk is full'); } return { ok: true }; });
    await w.findAll('button').find((b) => b.text().startsWith('Approve (A)')).trigger('click');
    await flushPromises(); await new Promise((r) => setTimeout(r, 300)); await flushPromises();
    expect(order.slice(0, 2)).toEqual(['get entry 2', 'write']);               // the next line first, the write after it
    expect(w.findAll('tbody tr')[0].text()).toContain('Approved');              // the row shows it at once
    fail = true; order.length = 0;
    await w.findAll('button').find((b) => b.text().startsWith('Approve (A)')).trigger('click');
    await flushPromises(); await new Promise((r) => setTimeout(r, 300)); await flushPromises();
    expect(w.text()).toContain('disk is full');
    expect(w.findAll('tbody tr')[1].text()).not.toContain('Approved');          // put back
  });
  it('moving fast shows the last line you chose, not a slower answer for an earlier one', async () => {
    const w = await setup();
    let release;
    const slow = new Promise((r) => { release = r; });
    get.mockImplementation(async (path, q) => {
      if (path === 'entry' && q.line === 1) { await slow; return { ok: true, ...item(1), path: 'game/tl/czech/chat.rpy' }; }
      if (path === 'entry') return { ok: true, ...item(q.line), path: 'game/tl/czech/chat.rpy' };
      return { ok: true };
    });
    const rows = w.findAll('tbody tr');
    await rows[0].trigger('click');                 // the answer for line 1 is slow
    await rows[1].trigger('click');                 // line 2 is chosen after it
    await flushPromises(); await new Promise((r) => setTimeout(r, 300)); await flushPromises();
    expect(w.find('.path').text()).toContain(':2');
    release();                                      // the late answer for line 1 arrives
    await flushPromises(); await new Promise((r) => setTimeout(r, 100)); await flushPromises();
    expect(w.find('.path').text()).toContain(':2');
    expect(w.findAll('tbody tr')[1].classes()).toContain('sel');
  });
});
