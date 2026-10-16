"use strict";
/* ================================================================ SETTINGS */
async function viewSettings() {
  const s = await get('settings');
  const root = h('div', { style: { maxWidth: '1100px' } });
  const val = (id) => $('#' + id);
  const field = (label, node, hint) => [h('label', { for: node.id }, label), node, hint ? h('div', { class: 'hint' }, hint) : null];
  const ln = s.validation.length;
  const tb = s.validation.textbox;
  const projectInput = h('input', { type: 'text', id: 'set_project', value: S.projectDir });
  root.append(
    h('fieldset', null, h('legend', null, 'Projekt'), h('div', { class: 'form' },
      ...field('Složka s překladem', h('div', { class: 'row nowrap' }, projectInput, h('button', { class: 'btn', onclick: pickProject }, 'Procházet…'), h('button', { class: 'btn', onclick: () => openProject(projectInput.value) }, 'Otevřít')),
        'Pamatuje se mezi spuštěními. Data (CSV, progress.json, zálohy) leží v podsložce _progress_data.'),
      ...field('Přeskočené soubory', h('input', { type: 'text', id: 'set_skip', value: s.skip_files.join(', ') }), 'Názvy souborů oddělené čárkou (ve všech podsložkách). Změna se projeví po „Obnovit soubory“.'))),
    h('fieldset', null, h('legend', null, 'Editor (otevření souboru na řádku)'), h('div', { class: 'form' },
      ...field('Editor', h('select', { id: 'set_editor', value: s.editor.preferred }, [['auto', 'Automaticky (VS Code, jinak Notepad++)'], ['vscode', 'Visual Studio Code'], ['notepadpp', 'Notepad++'], ['custom', 'Vlastní příkaz'], ['system', 'Výchozí program (bez skoku na řádek)']].map(([k, l]) => h('option', { value: k }, l)))),
      ...field('Vlastní příkaz', h('input', { type: 'text', id: 'set_custom', value: s.editor.custom, placeholder: '"C:\\Tools\\editor.exe" --line {line} {file}' }), 'Zástupné {file} a {line}. Spouští se bez shellu, takže cesty s mezerami a speciálními znaky jsou bezpečné.'))),
    h('fieldset', null, h('legend', null, 'Kontrola překladu'), h('div', { class: 'form' },
      ...field('Kontrola zapnutá', h('input', { type: 'checkbox', id: 'set_val', checked: s.validation.enabled })),
      ...field('Varování na délku textu', h('input', { type: 'checkbox', id: 'set_len', checked: ln.enabled }), 'Jen pomůcka při kontrole, nikdy chyba.'),
      ...field('Delší než (× originál)', h('input', { type: 'text', id: 'set_max', value: String(ln.max_ratio) }), 'Čeština bývá delší než angličtina, výchozí 2,2×.'),
      ...field('Kratší než (× originál)', h('input', { type: 'text', id: 'set_min', value: String(ln.min_ratio) }), 'Výchozí 0,45×.'),
      ...field('Nejmenší originál (znaků)', h('input', { type: 'text', id: 'set_minsrc', value: String(ln.min_source_chars) }), 'Kratší texty se na délku nekontrolují.'),
      ...field('Min. rozdíl – delší (znaků)', h('input', { type: 'text', id: 'set_extra', value: String(ln.min_extra_chars) })),
      ...field('Min. rozdíl – kratší (znaků)', h('input', { type: 'text', id: 'set_missing', value: String(ln.min_missing_chars) })),
      ...field('Hlídat překlepy', h('input', { type: 'checkbox', id: 'set_spell', checked: s.validation.spelling }), 'Chybějící diakritika a prohozená písmena podle slovníku z tvého vlastního překladu. Jen nápověda.'),
      ...field('Kontrola okna dialogu', h('input', { type: 'checkbox', id: 'set_tb', checked: tb.enabled }), 'Varuje, když se překlad nevejde do okna dialogu a originál se vešel.'),
      ...field('Znaků na řádek', h('input', { type: 'text', id: 'set_tbcpl', value: String(tb.chars_per_line) }), 'Výchozí 65 vychází z gui.rpy hry (šířka textu 1116 px, písmo 33 px).'),
      ...field('Nejvíc řádků v okně', h('input', { type: 'text', id: 'set_tblines', value: String(tb.max_lines) }), 'Výchozí 4.'))),
    h('fieldset', null, h('legend', null, 'Strojový překlad (nepovinné, experimentální)'), h('div', { class: 'form' },
      ...field('Služba', h('select', { id: 'set_mt_provider' }, [['', 'Vypnuto'], ['deepl', 'DeepL'], ['openai', 'OpenAI-kompatibilní (i místní)'], ['libretranslate', 'LibreTranslate']].map(([k, l]) => h('option', { value: k, selected: (s.mt?.provider || '') === k }, l))), 'Dává jen NÁVRH k jednomu řádku po kliknutí na „Návrh strojem“ v editoru; nikdy se nic samo nezapíše.'),
      ...field('Klíč k API', h('input', { type: 'password', id: 'set_mt_key', value: '', placeholder: s.mt?.has_key ? '(uložen – nech prázdné, pokud ho nechceš měnit)' : '', autocomplete: 'off' }), 'Ukládá se jako prostý text v config.json v %APPDATA%\\Lexo. Do aplikace se zpět neposílá.'),
      ...(s.mt?.has_key ? [h('span'), h('label', { class: 'check' }, h('input', { type: 'checkbox', id: 'set_mt_clear' }), ' Smazat uložený klíč')] : []),
      ...field('Adresa služby', h('input', { type: 'text', id: 'set_mt_url', value: s.mt?.base_url || '', placeholder: 'jen pro vlastní server, např. http://localhost:1234/v1' })),
      ...field('Model', h('input', { type: 'text', id: 'set_mt_model', value: s.mt?.model || '', placeholder: 'jen pro OpenAI-kompatibilní, např. gpt-4o-mini' })),
      ...field('Jazyky', h('span', { class: 'row' }, h('input', { type: 'text', id: 'set_mt_src', value: s.mt?.source_lang || 'EN', style: { width: '70px' } }), ' → ', h('input', { type: 'text', id: 'set_mt_dst', value: s.mt?.target_lang || 'CS', style: { width: '70px' } }))),
      ...field('Souhlas', h('label', { class: 'check' }, h('input', { type: 'checkbox', id: 'set_mt_consent', checked: !!s.mt?.consent }), ' Souhlasím, aby se anglický text řádku, na který klikneš, odeslal vybrané externí službě'), 'Bez souhlasu se nic neodesílá. Odesílá se jen text jednoho řádku (a u OpenAI-kompatibilní služby 2 předchozí řádky jako kontext).'))),
    h('fieldset', null, h('legend', null, 'Klávesové zkratky'), keysPanel()),
    h('fieldset', null, h('legend', null, 'Zálohy souborů překladu'), snapshotsPanel(s)),
    h('fieldset', null, h('legend', null, 'Kontrola hry (Ren\'Py lint)'), h('div', { class: 'form' },
      ...field('Složka Ren\'Py SDK', h('input', { type: 'text', id: 'set_sdk', value: s.lint?.sdk || '', placeholder: 'C:\\renpy-8.1.1-sdk' }), 'Složka s renpy.py (rozbalené SDK ze stránky renpy.org). Lint se spouští v Kontrola → Lint hry; potřebuje zdrojové soubory hry nad složkou překladu.'))),
    h('fieldset', null, h('legend', null, 'Týdenní cíle'), h('div', { class: 'form' },
      ...field('Překlad (řádků týdně)', h('input', { type: 'text', id: 'set_goal_t', value: String(s.goals?.translate_per_week ?? 0) }), '0 = automaticky podle tvého tempa za poslední týdny (o 10 % víc).'),
      ...field('Korektura (řádků týdně)', h('input', { type: 'text', id: 'set_goal_p', value: String(s.goals?.proofread_per_week ?? 0) }), '0 = automaticky.'))),
    h('fieldset', null, h('legend', null, 'Web (publikování)'), h('div', { class: 'form' },
      ...field('Repozitář', h('input', { type: 'text', id: 'set_repo', value: s.web.repo, placeholder: 'Filiposakkq/Being-A-DIK-CZ' }), 'Složka s klonem, adresa nebo uživatel/repozitář. Přihlášení řeší tvůj git (Credential Manager) – žádné tokeny se tu neukládají.'),
      ...field('Soubor v repozitáři', h('input', { type: 'text', id: 'set_path', value: s.web.path }), 'Výchozí web/progress.json.'),
      ...field('Název hry na webu', h('input', { type: 'text', id: 'set_title', value: s.web.title })),
      s.web.migrated_from ? h('div', { class: 'hint' }, `Převzato z ${s.web.migrated_from}. Samostatný web.json už aplikace nepotřebuje.`) : null)),
    h('fieldset', null, h('legend', null, 'O aplikaci'), h('p', null, h('b', null, `Lexo ${S.version || ''}`),
      ' – nástroj pro český překlad hry Being a DIK. Licence: viz soubor ', h('span', { class: 'mono' }, 'tools/translation-manager/LICENSE'),
      ' (osobní nekomerční použití; další šíření, úpravy a komerční použití jen s mým souhlasem).'),
      h('p', { class: 'notice' }, h('b', null, 'Rozsah nástroje: '), 'vzniká hlavně pro mě. Funguje jen pro Being a DIK (1. a 2. sezóna) a má pouze české rozhraní. ',
        'V plánu je nejdřív 3. sezóna, možná později přepracování pro víc Ren\'Py her a angličtinu (bez slibů a termínů).'),
      h('div', { class: 'row' }, ...[['kofi', 'Podpořit na Ko-fi'], ['github', 'GitHub'], ['repo', 'Repozitář překladu'], ['web', 'Web překladu']].map(([id, l]) =>
        h('button', { class: 'btn small', onclick: () => post('open-url', { id }).catch(fail) }, l)))),
    h('div', { class: 'row' }, h('button', { class: 'btn primary', onclick: save }, 'Uložit nastavení'),
      h('span', { class: 'muted' }, 'Nastavení: ', h('span', { class: 'mono' }, 'config.json'), ' v %APPDATA%\\Lexo')));
  /* Group the fieldsets into sections with a side navigation. Everything stays in the DOM (only hidden), so Save sees every field. */
  const fsets = [...root.querySelectorAll(':scope > fieldset')];
  const findSet = (t) => fsets.find((f) => f.querySelector('legend').textContent.startsWith(t));
  const SECS = [['general', 'Obecné', ['Projekt', 'Editor', 'Týdenní cíle']], ['check', 'Kontrola', ['Kontrola překladu']], ['mt', 'Strojový překlad', ['Strojový']],
    ['backup', 'Zálohy a lint', ['Zálohy', 'Kontrola hry']], ['keys', 'Zkratky', ['Klávesové']], ['web', 'Web', ['Web']], ['about', 'O aplikaci', ['O aplikaci']]];
  const saveRow = root.lastElementChild;
  const content = h('div', { class: 'setcontent' });
  const secs = SECS.map(([id, title, legends]) => { const sec = h('section', { class: 'setsec' }, ...legends.map(findSet).filter(Boolean)); content.append(sec); return [id, title, sec]; });
  const nav = h('nav', { class: 'setnav', 'aria-label': 'Sekce nastavení' });
  function showSec(id) {
    setPref('ui.settingsSection', id);
    secs.forEach(([k, , sec]) => { sec.hidden = k !== id; });
    nav.replaceChildren(...secs.map(([k, t]) => h('button', { class: 'setnavbtn' + (k === id ? ' on' : ''), 'aria-current': k === id ? 'page' : 'false', onclick: () => showSec(k) }, t)));
  }
  root.replaceChildren(h('div', { class: 'settings' }, nav, content), h('div', { class: 'savebar' }, saveRow));
  const want = pref('ui.settingsSection', 'general');
  showSec(secs.some(([k]) => k === want) ? want : 'general');
  async function save() {
    try {
      await post('settings', {
        skip_files: val('set_skip').value.split(',').map((x) => x.trim()).filter(Boolean),
        editor: { preferred: val('set_editor').value, custom: val('set_custom').value },
        validation: { enabled: val('set_val').checked, spelling: val('set_spell').checked,
          textbox: { enabled: val('set_tb').checked, chars_per_line: num(val('set_tbcpl').value), max_lines: num(val('set_tblines').value) }, length: { enabled: val('set_len').checked, max_ratio: num(val('set_max').value), min_ratio: num(val('set_min').value),
          min_source_chars: num(val('set_minsrc').value), min_extra_chars: num(val('set_extra').value), min_missing_chars: num(val('set_missing').value) } },
        goals: { translate_per_week: num(val('set_goal_t').value), proofread_per_week: num(val('set_goal_p').value) },
        snapshots: { enabled: val('set_snap_on').checked, interval_hours: num(val('set_snap_h').value), keep: num(val('set_snap_keep').value) },
        lint: { sdk: val('set_sdk').value },
        mt: { provider: val('set_mt_provider').value, api_key: val('set_mt_key').value, clear_key: !!val('set_mt_clear')?.checked, base_url: val('set_mt_url').value, model: val('set_mt_model').value,
          source_lang: val('set_mt_src').value, target_lang: val('set_mt_dst').value, consent: val('set_mt_consent').checked },
        web: { repo: val('set_repo').value, path: val('set_path').value, title: val('set_title').value },
      });
      S.mtOn = undefined; toast('Nastavení uloženo ✓'); refreshHeader();
    } catch (e) { fail(e); }
  }
  const num = (v) => Number(String(v).replace(',', '.'));
  return root;
}

