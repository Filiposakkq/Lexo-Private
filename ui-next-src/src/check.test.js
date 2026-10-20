import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get, post } from './api.js';
import { openCheck, project } from './project.js';
import Check from './views/Check.vue';

enableAutoUnmount(afterEach);

const finding = { file: 'script.rpy', line: 29, severity: 'error', src_who: 'mc', source: 'Bye, {name}.', target: 'Měj se, {jmeno}.',
  issues: [{ severity: 'error', code: 'var-missing', message: 'Variable missing', extra: {} }, { severity: 'warning', code: 'tag-missing', message: 'A tag is missing', extra: { fixable: true, kind: 'speaker' } }] };
const group = { key: 'k1', source: 'Yes.', total: 3, sources: [], variants: [
  { target: 'Ano.', count: 2, lines: [{ file: 'a.rpy', line: 1, who: 'mc' }, { file: 'a.rpy', line: 5, who: 'mc' }] },
  { target: 'Jo.', count: 1, lines: [{ file: 'a.rpy', line: 9, who: 'eva' }] }] };

beforeEach(() => {
  get.mockReset(); post.mockReset(); post.mockResolvedValue({ ok: true, changed: 1 });
  project.files = [{ name: 'script.rpy' }]; project.overview = { validation: { ignored: 0 } }; openCheck({});
  get.mockImplementation(async (path) => {
    if (path === 'validation') return { ok: true, items: [finding], total: 1, findings: 2, sev_counts: { error: 1, warning: 1, info: 0 } };
    if (path === 'consistency') return { ok: true, groups: [group], lines: 3 };
    if (path === 'ignore/list') return { ok: true, items: [{ file: 'script.rpy', line: 29, code: 'tag-missing', state: 'active', message: 'A tag is missing', note: 'ok', target: 'Měj se' }], counts: {} };
    if (path === 'fix/last') return { ok: true, last: null };
    if (path === 'settings') return { ok: true, validation: { show_info: false }, lint: { sdk: '' } };
    if (path === 'lint/status') return { ok: true, state: 'idle' };
    return { ok: true };
  });
});

const btn = (w, text) => w.findAll('button').find((b) => b.text().startsWith(text));

describe('Check', () => {
  it('lists the findings of a line together, with the counts, and opens the line', async () => {
    const w = mount(Check);
    await flushPromises();
    expect(w.text()).toContain('Variable missing');
    expect(w.text()).toContain('A tag is missing');
    expect(w.text()).toContain('1 line with a finding');
    await btn(w, 'Show').trigger('click');
    expect(project.lines.file).toBe('script.rpy');
    expect(project.lines.line).toBe(29);
    expect(w.emitted('go')[0]).toEqual(['lines']);
  });

  it('opens on the file it was sent for and asks the server for that file only', async () => {
    openCheck({ file: 'script.rpy' });
    mount(Check);
    await flushPromises();
    expect(get.mock.calls.find((c) => c[0] === 'validation')[1].file).toBe('script.rpy');
  });

  it('ignores one finding with a note, through a dialog', async () => {
    const w = mount(Check);
    await flushPromises();
    await btn(w, 'Ignore…').trigger('click');
    await w.find('[role="dialog"] input[type="text"]').setValue('fine here');
    await w.findAll('[role="dialog"] button').find((b) => b.text() === 'Ignore').trigger('click');
    await flushPromises();
    expect(post).toHaveBeenCalledWith('ignore/set', { items: [{ file: 'script.rpy', line: 29, code: 'var-missing' }], on: true, note: 'fine here' });
    expect(w.text()).toContain('Ignored 1 finding');
  });

  it('shows the originals translated differently and unifies only the lines you tick', async () => {
    const w = mount(Check);
    await flushPromises();
    await btn(w, 'Different translations').trigger('click');
    await flushPromises();
    expect(w.text()).toContain('Ano.');
    await btn(w, 'Unify…').trigger('click');          // the first variant: "Ano."
    await flushPromises();
    const boxes = w.findAll('[role="dialog"] input[type="checkbox"]');
    expect(boxes).toHaveLength(3);
    expect(boxes.filter((b) => b.element.checked)).toHaveLength(1);       // only the line that differs ("Jo.") is ticked
    await w.findAll('[role="dialog"] button').find((b) => b.text().startsWith('Unify 1')).trigger('click');
    await flushPromises();
    expect(post.mock.calls.find((c) => c[0] === 'consistency/unify')[1].lines).toEqual([{ file: 'a.rpy', line: 9 }]);
  });

  it('restores an ignored finding', async () => {
    const w = mount(Check);
    await flushPromises();
    await btn(w, 'Ignored findings').trigger('click');
    await flushPromises();
    await btn(w, 'Stop ignoring').trigger('click');
    expect(post).toHaveBeenCalledWith('ignore/set', { items: [{ file: 'script.rpy', line: 29, key: undefined, code: 'tag-missing' }], on: false });
  });
});
