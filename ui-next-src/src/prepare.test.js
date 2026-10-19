import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const answers = {};
vi.mock('./api.js', () => ({ post: vi.fn(async (route, body) => { const a = answers[route]; if (a instanceof Error) throw a; return typeof a === 'function' ? a(body) : a; }), get: vi.fn() }));
import { post } from './api.js';
import Prepare from './views/Prepare.vue';

const info = (over = {}) => ({ ok: true, root: 'D:\\Games\\Fake', name: 'Fake', archives: 0, scripts: 3, bytecode_only: 0, translations: [], engine: true, unrpyc: null, needs_unrpyc: false, problems: [], ready: true, ...over });

describe('the guide to prepare a game', () => {
  beforeEach(() => { vi.useFakeTimers(); post.mockClear(); for (const k of Object.keys(answers)) delete answers[k]; });
  afterEach(() => vi.useRealTimers());

  it('looks at the game first and says what is wrong', async () => {
    answers['prepare/inspect'] = info({ engine: false, ready: false });
    const w = mount(Prepare, { props: { path: 'D:\\Games\\Fake' } });
    await flushPromises();
    expect(w.text()).toContain('no engine of its own');
    expect(w.find('.actions .primary').attributes('disabled')).toBeDefined();
  });

  it('asks for unrpyc only when the scripts need it', async () => {
    answers['prepare/inspect'] = info({ bytecode_only: 2, needs_unrpyc: true, ready: false, problems: [{}] });
    const w = mount(Prepare, { props: { path: 'D:\\Games\\Fake' } });
    await flushPromises();
    expect(w.text()).toContain('unrpyc is needed');
    answers['prepare/inspect'] = info();
    answers['prepare/plan'] = { ok: true, mode: 'copy', workspace: 'x', links: true, need: 1, free: 5e10, enough: true };
    const ok = mount(Prepare, { props: { path: 'D:\\Games\\Fake' } });
    await flushPromises();
    expect(ok.text()).not.toContain('unrpyc is needed');
    expect(ok.find('.actions .primary').attributes('disabled')).toBeUndefined();
  });

  it('runs the job, shows the steps as they go, and offers to open the project when it is done', async () => {
    answers['prepare/inspect'] = info();
    answers['prepare/start'] = { ok: true, job: 'j1' };
    answers['prepare/plan'] = { ok: true, mode: 'copy', workspace: 'D:\\Games\\Fake-aside', links: true, need: 4096, free: 5e10, enough: true };
    let n = 0;
    answers['prepare/status'] = () => (++n < 2 ? { ok: true, current: 'extract', steps: ['workspace'], done: false } : { ok: true, current: '', steps: ['workspace', 'extract', 'generate'], done: true, success: true, result: { folder: 'C:\\w\\game\\tl\\czech', files: 4, reused: false } });
    answers['library/add'] = { ok: true };
    const w = mount(Prepare, { props: { path: 'D:\\Games\\Fake' } });
    await flushPromises();
    await w.find('.actions .primary').trigger('click');
    await flushPromises();
    expect(post).toHaveBeenCalledWith('prepare/start', { path: 'D:\\Games\\Fake', language: 'czech', mode: 'copy', workspace: '' });
    await vi.advanceTimersByTimeAsync(700);
    expect(w.findAll('.steps li.done')).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(700);
    await flushPromises();
    expect(post).toHaveBeenCalledWith('library/add', { path: 'C:\\w\\game\\tl\\czech' });
    expect(w.text()).toContain('The translation is ready: 4 files.');
    expect(w.text()).toContain('not needed');             // no bytecode was read, so that step is marked as not needed
    expect(w.find('.actions .primary').text()).toBe('Open the project');
  });

  it('shows what went wrong and lets the user try again', async () => {
    answers['prepare/inspect'] = info();
    answers['prepare/start'] = { ok: true, job: 'j2' };
    answers['prepare/plan'] = { ok: true, mode: 'copy', workspace: 'D:\\Games\\Fake-aside', links: true, need: 4096, free: 5e10, enough: true };
    answers['prepare/status'] = { ok: true, current: 'generate', steps: ['workspace', 'extract'], done: true, success: false, error: 'The game did not write the translation.' };
    const w = mount(Prepare, { props: { path: 'D:\\Games\\Fake' } });
    await flushPromises();
    await w.find('.actions .primary').trigger('click');
    await flushPromises();
    await vi.advanceTimersByTimeAsync(700);
    await flushPromises();
    expect(w.text()).toContain('The game did not write the translation.');
    expect(w.find('.actions .primary').text()).toBe('Try again');
    expect(w.find('.actions .primary').attributes('disabled')).toBeUndefined();     // trying again must be possible
  });
});

describe('where to work', () => {
  beforeEach(() => { vi.useFakeTimers(); post.mockClear(); for (const k of Object.keys(answers)) delete answers[k]; });
  afterEach(() => vi.useRealTimers());

  it('tells when there is not enough room and does not start; working in the game needs no room', async () => {
    answers['prepare/inspect'] = info();
    answers['prepare/plan'] = (b) => (b.mode === 'inplace' ? { ok: true, mode: 'inplace', workspace: 'D:\Games\Fake', links: false, need: 0, free: 1e9, enough: true } : { ok: true, mode: 'copy', workspace: 'C:\w', links: false, need: 5e9, free: 1e9, enough: false });
    const w = mount(Prepare, { props: { path: 'D:\Games\Fake' } });
    await flushPromises();
    expect(w.text()).toContain('is not enough');
    expect(w.text()).toContain('The copy is on another disk');
    expect(w.find('.actions .primary').attributes('disabled')).toBeDefined();
    await w.find('input[value="inplace"]').setValue(true);
    await flushPromises();
    expect(w.text()).toContain('Keep a backup of the game');
    expect(w.find('.actions .primary').attributes('disabled')).toBeUndefined();
    expect(post).toHaveBeenCalledWith('prepare/plan', { path: 'D:\Games\Fake', mode: 'inplace', workspace: '' });
  });
});
