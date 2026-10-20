import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get, post } from './api.js';
import { project } from './project.js';
import Publish from './views/Publish.vue';
import Settings from './views/Settings.vue';

enableAutoUnmount(afterEach);

const settings = (over = {}) => ({
  ok: true, project_dir: 'C:\\g\\tl\\czech', skip_files: ['common.rpy'], editor: { preferred: 'auto', custom: '' },
  validation: { enabled: true, spelling: true, textbox: { enabled: true, chars_per_line: 65, max_lines: 4 }, length: { enabled: true, max_ratio: 2.2, min_ratio: 0.45, min_source_chars: 12, min_extra_chars: 25, min_missing_chars: 18 } },
  goals: { translate_per_week: 0, proofread_per_week: 0 }, snapshots: { enabled: true, interval_hours: 12, keep: 20 }, lint: { sdk: '' },
  mt: { provider: '', has_key: false, base_url: '', model: '', source_lang: 'EN', target_lang: '', consent: false }, web: { repo: '', path: 'web/progress.json', title: '' }, ...over,
});

beforeEach(() => {
  get.mockReset(); post.mockReset(); post.mockResolvedValue({ ok: true });
  try { localStorage.clear(); } catch { /* none */ }
  project.overview = { project: { name: 'x' } }; project.files = [];
  get.mockImplementation(async (path) => (path === 'settings' ? settings() : path === 'snapshots' ? { ok: true, items: [{ name: 's1', time: '2026-10-20T10:00:00', reason: 'auto', files: 12, size: 4096 }] } : path === 'ui-langs' ? { ok: true } : { ok: true }));
});

describe('Settings', () => {
  it('saves what you changed, in the shape the application takes', async () => {
    const w = mount(Settings);
    await flushPromises();
    await w.find('input[placeholder^="\\"C:"]').exists();
    const skip = w.findAll('input[type="text"]').find((i) => i.element.value === 'common.rpy');
    await skip.setValue('common.rpy, extra.rpy');
    await w.findAll('button').find((b) => b.text() === 'Save settings').trigger('click');
    await flushPromises();
    const body = post.mock.calls.find((c) => c[0] === 'settings')[1];
    expect(body.skip_files).toEqual(['common.rpy', 'extra.rpy']);
    expect(body.validation.length.max_ratio).toBe(2.2);
    expect(body.snapshots).toEqual({ enabled: true, interval_hours: 12, keep: 20 });
    expect(body.mt.api_key).toBe('');
    expect(w.text()).toContain('Settings saved ✓');
  });

  it('shows the sections and the backups with a restore and a delete that needs a second click', async () => {
    const w = mount(Settings);
    await flushPromises();
    await w.findAll('.setnavbtn').find((b) => b.text() === 'Backups and lint').trigger('click');
    await flushPromises();
    expect(w.text()).toContain('2026-10-20 10:00');
    const del = w.findAll('button').find((b) => b.text() === 'Delete');
    await del.trigger('click');
    expect(post).not.toHaveBeenCalledWith('snapshots/delete', expect.anything());
    await w.findAll('button').find((b) => b.text() === 'Delete?').trigger('click');
    expect(post).toHaveBeenCalledWith('snapshots/delete', { name: 's1' });
  });
});

describe('Publish', () => {
  it('sends you to Settings when the website is not set up, and shows the preview of the data', async () => {
    get.mockImplementation(async (path) => (path === 'settings' ? settings() : path === 'export/preview'
      ? { ok: true, problems: [], path: 'p.json', data: { translation: { percent: 40, done: 4, total: 10 }, proofreading: { percent: 0 }, files: { total: 2 }, estimate: { status: 'none' }, seasons: [], episodes: [] } } : { ok: true }));
    const w = mount(Publish);
    await flushPromises();
    expect(w.text()).toContain('The connection to the website is not set up yet.');
    await w.findAll('button').find((b) => b.text() === 'Open Settings').trigger('click');
    expect(w.emitted('go')[0]).toEqual(['settings']);
    await w.findAll('button').find((b) => b.text() === 'Generate a preview').trigger('click');
    await flushPromises();
    expect(w.text()).toContain('The data passed the check.');
  });
});
