import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api.js', () => ({ post: vi.fn(async () => ({ ok: true })), get: vi.fn(async () => ({ ok: true })) }));
import { post } from './api.js';
import GameCard from './components/GameCard.vue';
import Home from './views/Home.vue';
import { ago, app } from './state.js';

const game = (over = {}) => ({ path: 'C:\\g\\a', name: 'Alpha', version: '1.0', language: 'czech', total: 200, done: 50, corr: 10, pinned: false, opened: '2026-10-19T10:00:00Z', state: 'ready', ...over });

describe('a card of a game', () => {
  it('shows the name, the progress and the way to continue', () => {
    const w = mount(GameCard, { props: { game: game(), latest: true } });
    expect(w.text()).toContain('Alpha');
    expect(w.text()).toContain('25 %');
    expect(w.find('button.go').text()).toBe('Continue');
    expect(w.find('[role="progressbar"]').attributes('aria-valuenow')).toBe('25');
  });
  it('asks to prepare a game that has no translation, and says so when its folder is gone', async () => {
    const w = mount(GameCard, { props: { game: game({ state: 'needs_prepare' }) } });
    expect(w.text()).toContain('Not prepared yet');
    await w.find('button.go').trigger('click');
    expect(w.emitted('prepare')).toBeTruthy();
    const gone = mount(GameCard, { props: { game: game({ state: 'missing' }) } });
    expect(gone.text()).toContain('The folder is gone');
  });
  it('removes from the list only after a second click', async () => {
    const w = mount(GameCard, { props: { game: game() } });
    await w.find('.more').trigger('click');
    const remove = () => w.findAll('.menu-item').find((i) => i.text().startsWith('Remove'));
    await remove().trigger('click');
    expect(w.emitted('remove')).toBeFalsy();
    await remove().trigger('click');
    expect(w.emitted('remove')).toBeTruthy();
  });
});

describe('Home', () => {
  beforeEach(() => { app.games = null; app.licence = null; post.mockClear(); });

  it('says what to do when there is no game yet', () => {
    app.games = [];
    const w = mount(Home);
    expect(w.text()).toContain('Welcome to Aside Pro');
  });
  it('lists the games, the pinned first, and sorts by name or by progress', async () => {
    app.games = [game({ path: 'b', name: 'Bravo', done: 150 }), game({ path: 'a', name: 'Alpha', done: 20 }), game({ path: 'c', name: 'Charlie', done: 10, pinned: true })];
    const w = mount(Home);
    const names = () => w.findAll('.game .heading').map((h) => h.text());
    expect(names()).toEqual(['Charlie', 'Bravo', 'Alpha']);
    await w.find('.sort select').setValue('name');
    expect(names()).toEqual(['Charlie', 'Alpha', 'Bravo']);
    await w.find('.sort select').setValue('progress');
    expect(names()).toEqual(['Charlie', 'Bravo', 'Alpha']);
  });
  it('searches by name', async () => {
    app.games = [game({ path: 'b', name: 'Bravo' }), game({ path: 'a', name: 'Alpha' })];
    const w = mount(Home);
    await w.find('.search input').setValue('alp');
    expect(w.findAll('.game')).toHaveLength(1);
  });
  it('tells about the licence when its check is near, overdue or expired', () => {
    app.games = [game()];
    app.licence = { state: 'grace', days_left: 5 };
    expect(mount(Home).text()).toContain('overdue');
    app.licence = { state: 'licensed', days_to_check: 2 };
    expect(mount(Home).text()).toContain('due in 2 days');
    app.licence = { state: 'licensed', days_to_check: 12 };
    expect(mount(Home).find('.notice').exists()).toBe(false);
  });
  it('asks the application to open a game', async () => {
    app.games = [game()];
    const original = window.location;
    Object.defineProperty(window, 'location', { value: { href: '' }, writable: true });
    const w = mount(Home);
    await w.find('button.go').trigger('click');
    await new Promise((r) => setTimeout(r, 0));
    expect(post).toHaveBeenCalledWith('project/set', { path: 'C:\\g\\a' });
    Object.defineProperty(window, 'location', { value: original, writable: true });
  });
});

describe('the time of the last work', () => {
  it('is said in words, and nothing without a time', () => {
    expect(ago('', 'en-GB')).toBe('');
    const twoHours = new Date(Date.now() - 2 * 3600 * 1000).toISOString();
    expect(ago(twoHours, 'en-GB')).toBe('2 hours ago');
  });
});
