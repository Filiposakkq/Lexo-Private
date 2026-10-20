import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { get, post } from './api.js';
import { project } from './project.js';
import Glossary from './views/Glossary.vue';

enableAutoUnmount(afterEach);

const items = () => [
  { code: 'mc', name: 'Alex', note: '', type: 'speaker', in_project: true, count: 40, example: 'Hello' },
  { code: 'eva', name: '', note: '', type: 'speaker', in_project: true, count: 7, example: 'Hi' },
  { code: 'Delta Iota Kappa', name: 'Delta Iota Kappa', note: 'the frat', type: 'term', in_project: true, count: 3, example: 'DIK' },
];

beforeEach(() => {
  get.mockReset(); post.mockReset();
  try { localStorage.clear(); } catch { /* none */ }
  get.mockImplementation(async (path) => (path === 'glossary' ? { ok: true, items: items(), names: { mc: 'Alex' }, unnamed: 1 }
    : path === 'glossary/occurrences' ? { ok: true, czech: 'Delta Iota Kappa', found: 1, missing: 1, untranslated: 0, truncated: false, items: [{ file: 'a.rpy', line: 3, used: false, translated: true, source: 'DIK party', target: 'Večírek' }] }
      : { ok: true }));
  post.mockImplementation(async (path, body) => (path === 'glossary/set' ? { ok: true, items: items().map((i) => (i.code === body.items[0].code ? { ...i, ...body.items[0] } : i)), names: {}, unnamed: 0 } : { ok: true, items: items(), names: {}, unnamed: 1 }));
  project.names = {};
});

describe('Glossary', () => {
  it('lists the speakers and the terms, and filters the speakers without a name', async () => {
    const w = mount(Glossary);
    await flushPromises();
    expect(w.findAll('tbody tr')).toHaveLength(3);
    expect(w.text()).toContain('3 items · without a name: 1');
    await w.findAll('button').find((b) => b.text() === 'Without a name').trigger('click');
    expect(w.findAll('tbody tr')).toHaveLength(1);
    expect(w.find('tbody tr').text()).toContain('eva');
  });

  it('saves a name when the field is left, with the type and the note', async () => {
    const w = mount(Glossary);
    await flushPromises();
    const row = w.findAll('tbody tr').find((r) => r.text().includes('eva'));
    const input = row.findAll('input')[0];
    await input.setValue('  Eva Stone ');
    await input.trigger('change');
    await flushPromises();
    expect(post).toHaveBeenCalledWith('glossary/set', { items: [{ code: 'eva', name: 'Eva Stone', type: 'speaker', note: '' }] });
  });

  it('adds an item only with a code, and deletes an entry', async () => {
    const w = mount(Glossary);
    await flushPromises();
    await w.findAll('button').find((b) => b.text().startsWith('+ Add')).trigger('click');
    await w.findAll('[role="dialog"] button').find((b) => b.text() === 'Add').trigger('click');
    expect(w.text()).toContain('Fill in the code or the term.');
    expect(post).not.toHaveBeenCalled();
    const row = w.findAll('tbody tr').find((r) => r.text().includes('mc'));
    await row.findAll('button').find((b) => b.text() === 'Delete').trigger('click');
    expect(post).toHaveBeenCalledWith('glossary/delete', { codes: ['mc'] });
  });

  it('shows where a term occurs and whether the translated term is used', async () => {
    const w = mount(Glossary);
    await flushPromises();
    const row = w.findAll('tbody tr').find((r) => r.text().includes('Delta Iota Kappa'));
    await row.findAll('button').find((b) => b.text() === 'Occurrences').trigger('click');
    await flushPromises();
    expect(w.find('[role="dialog"]').text()).toContain('DIK party');
    expect(w.find('[role="dialog"]').text()).toContain('missing');
  });
});
