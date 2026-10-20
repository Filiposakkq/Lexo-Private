# UI parity checklist

The new interface (see the sketches discussed in the project: a sidebar with Overview, Files, Lines, Check, Dictionary, Tools, Team, Release, Settings) changes **where** things are and how they look, **never what the application can do**. This list is generated from the current interface: every button and menu action by its current label (Czech text in brackets). Each one must have a place in the new interface before the old one is removed. Fill the column *New place* while designing; the UI self-test and the contract tests must stay green; nothing is deleted without a line here saying where it went.

Generated from the sources of the current UI (`rust/resources/ui-classic/js`); 324 actions in 24 areas. It is a checklist to start from, not an exact inventory: a few actions are built without a plain button label and must be added by hand.

**Progress.** The *New place* column is filled: it names the screen or the dialog of the new interface (`ui-next-src/src`) that has the action. The state and what is left are in the section *State* below; the plan of the switch is in [UI-NEXT.md](UI-NEXT.md). Nothing is removed from the classic interface before its row is ticked.


## State (filled by comparing the texts of the classic interface with the new one, then by hand)

**314 of 324 actions have their place in the new interface.** The 10 that are left are the film layouts (quick translating and quick proofreading from Overview, the results bar with *Result n of m*, the list of hits), the side-by-side layout of Lines and the second window (*Scene and detail*, *Scene over the whole window*, *Show them here too*). The rows marked (#126) are in the pull request of the keyboard and the licence and count once it is merged. A row is ticked when the same text is used by the new interface or when its place was checked by hand; this is a check of the actions, not of how each one behaves, which the tests and the browser checks of each pull request cover.

## Přehled (`20-overview.js`)

| Current action | New place | Done |
|---|---|---|
| Continue proofreading (Pokračovat v korektuře) | Files, Overview | ☑ |
| No groups yet (Zatím žádné skupiny) | GroupsDialog, Overview | ☑ |
| Set up groups… (Nastavit skupiny…) | Overview | ☑ |
| Start translating (Začít překládat) | Overview | ☑ |
| Scene and editor over the whole screen, untranslated lines only (Scéna a editor přes celou obrazovku, jen nepřeložené řádky) | **not built:** film layout | ☐ |
| Translate quickly (Rychle překládat) | **not built:** film layout | ☐ |
| Proofread quickly (Rychle zkontrolovat) | **not built:** film layout | ☐ |

## Soubory (`21-files.js`)

| Current action | New place | Done |
|---|---|---|
| File (Soubor) | ApproveIdentical, GlossaryImport, ReleaseDialog | ☑ |
| Assignment (Zařazení) | Files | ☑ |
| Translation (Překlad) | GamePreview, LineDetail, OccurrencesDialog | ☑ |
| Proofreading (Korektura) | LineDetail, project, Files | ☑ |
| Left to translate (Zbývá přeložit) | Files, Overview | ☑ |
| Left to proofread (Zbývá zkontrolovat) | Files | ☑ |
| State (Stav) | GlossaryImport, OccurrencesDialog, ReleaseDialog | ☑ |
| Check (Kontrola) | App, LineDetail, ReleaseDialog | ☑ |
| Assign selected… (Zařadit vybrané…) | Files (`Assign selected (n)…`) | ☑ |
| Suggest assignment… (Navrhnout zařazení…) | Files | ☑ |
| What the groups (episodes, chapters…) are called and which numbers exist (Jak se skupiny (epizody, kapitoly…) jmenují a jaká čísla existují) | Files | ☑ |
| Groups… (Skupiny…) | Files | ☑ |
| Who is in the team of this project and who has which files (Kdo je v týmu tohoto projektu a kdo má které soubory) | Files | ☑ |
| Team… (Tým…) | Files | ☑ |
| Assigned to (Přiděleno) | Files | ☑ |
| Mark the whole file as proofread (Označit celý soubor jako zkontrolovaný) | Files | ☑ |
| Show lines (Zobrazit řádky) | Files | ☑ |
| Only untranslated (Jen nepřeložené) | Files | ☑ |
| Lines that changed after proofreading (Řádky, které se po korektuře změnily) | Files | ☑ |
| Continue proofreading (Pokračovat v korektuře) | Files, Overview | ☑ |
| Open in the editor (Otevřít v editoru) | Files | ☑ |
| Copy the path (Kopírovat cestu) | LineDetail, ReleaseDialog, Files | ☑ |
| To fix (K opravě) | Files, Lines | ☑ |
| Show the problems (Zobrazit problémy) | Files | ☑ |
| Set up groups… (Nastavit skupiny…) | Overview | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Řádky (`22-entries.js`)

| Current action | New place | Done |
|---|---|---|
| File (Soubor) | ApproveIdentical, GlossaryImport, ReleaseDialog | ☑ |
| Line (Řádek) | ApproveIdentical, LineDetail, Lines | ☑ |
| Line in the translation file (Řádek v překladovém souboru) | Lines | ☑ |
| Script (Skript) | LineDetail, Lines | ☑ |
| Type (Typ) | Glossary, Lines | ☑ |
| State (Stav) | GlossaryImport, OccurrencesDialog, ReleaseDialog | ☑ |
| Proofreading (Korektura) | LineDetail, project, Files | ☑ |
| Code (Kód) | GlossaryImport, MissedTextsDialog, ProfileDialog | ☑ |
| Speaker abbreviation (and image attributes) (Zkratka mluvčího (a atributy obrázku)) | Lines | ☑ |
| Name (Jméno) | ProfileDialog, Glossary, Lines | ☑ |
| Full name from the Glossary (Celé jméno ze Slovníku) | Lines | ☑ |
| Original (Originál) | GamePreview, LineDetail, MtDialog | ☑ |
| Translation (Překlad) | GamePreview, LineDetail, OccurrencesDialog | ☑ |
| Check (Kontrola) | App, LineDetail, ReleaseDialog | ☑ |
| Line type, kind, check, sorting, speaker display (Typ řádku, druh, kontrola, řazení, zobrazení mluvčích) | Lines | ☑ |
| Find and replace… (Najít a nahradit…) | Tools | ☑ |
| in the translations (v překladech) | Tools | ☑ |
| Approve by rule… (Schválit podle pravidla…) | Tools | ☑ |
| names, interjections (jména, citoslovce) | Tools | ☑ |
| Texts the generator missed… (Texty, které generátor vynechal…) | Tools | ☑ |
| show text, input prompts, titles (show text, výzvy k zadání, názvy) | Tools | ☑ |
| Carry over from the previous version… (Přenést překlad z předchozí verze…) | Tools | ☑ |
| Pro: translation of the old game version (Pro: překlad staré verze hry) | Tools | ☑ |
| Fill from similar translations… (Doplnit z podobných překladů…) | Tools | ☑ |
| Pro: similar originals of this project (Pro: podobné originály v tomto projektu) | Tools | ☑ |
| Fix… (Opravit…) | LineDetail, Tools, Check | ☑ |
| speaker, spaces, typos (mluvčí, mezery, překlepy) | Tools | ☑ |
| Translate with a machine… (Přeložit strojem…) | Tools, Lines | ☑ |
| selected lines, scene, file (vybrané řádky, scéna, soubor) | Tools | ☑ |
| Check the style… (Zkontrolovat styl…) | Tools | ☑ |
| Pro: address, gender, tone (language model) (Pro: oslovení, rod, tón (jazykový model)) | Tools | ☑ |
| Check the spelling… (Zkontrolovat pravopis…) | Tools | ☑ |
| Pro: with a dictionary of the language (Pro: se slovníkem jazyka) | Tools | ☑ |
| Build a release… (Sestavit vydání…) | Tools | ☑ |
| Pro: a zip for the players (Pro: zip pro hráče) | Tools | ☑ |
| Clear the selection (Zrušit výběr) | Lines | ☑ |
| Unapprove (A) (Zrušit schválení (A)) | LineDetail | ☑ |
| Approve (A) (Schválit (A)) | LineDetail | ☑ |
| Opens the file at this line in an external editor (Otevře soubor na tomhle řádku v externím editoru) | LineDetail | ☑ |
| Editor (E) (Editor (E)) | LineDetail | ☑ |
| Approve all identical… (Schválit všechny shodné…) | LineDetail | ☑ |
| Copy the path (Kopírovat cestu) | LineDetail, ReleaseDialog, Files | ☑ |
| Copies the original text to the clipboard (Zkopíruje text originálu do schránky) | LineDetail | ☑ |
| ⧉ Original (⧉ Originál) | LineDetail | ☑ |
| Copies the original and the translation one under the other (Zkopíruje originál a překlad pod sebe) | LineDetail | ☑ |
| ⧉ Both (⧉ Oboje) | LineDetail | ☑ |
| First line of the previous scene (První řádek předchozí scény) | LineDetail | ☑ |
| ◀ Scene (◀ Scéna) | LineDetail | ☑ |
| First line of the next scene (První řádek další scény) | LineDetail | ☑ |
| Scene ▶ (Scéna ▶) | LineDetail | ☑ |
| Next untranslated line in the file (Další nepřeložený řádek v souboru) | LineDetail | ☑ |
| Next untranslated (Další nepřeložený) | LineDetail | ☑ |
| Next translated but unchecked line (Další přeložený, ale nezkontrolovaný řádek) | LineDetail | ☑ |
| Next unchecked (Další nezkontrolovaný) | LineDetail | ☑ |
| Next line that does not fit the game window (Další řádek, který se nevejde do okna hry) | LineDetail | ☑ |
| Next one that does not fit (Další, co se nevejde) | LineDetail | ☑ |
| Load more (Načíst další) | Lines (`Load n more`) | ☑ |
| Hide the list (Skrýt seznam) | **not built:** results bar of the film layout | ☐ |
| Show the list (Zobrazit seznam) | **not built:** results bar of the film layout | ☐ |
| Previous line (←) (Předchozí řádek (←)) | **not built:** results bar of the film layout | ☐ |
| Next line (→) (Další řádek (→)) | **not built:** results bar of the film layout | ☐ |
| Show them here too (Ukázat je i tady) | **not built:** second window | ☐ |
| Scene and detail (Scéna a detail) | **not built:** second window | ☐ |
| Scene over the whole window (Scéna přes celé okno) | **not built:** second window | ☐ |
| Stop ignoring (Přestat ignorovat) | LineDetail, Check | ☑ |
| Hides this finding on this line (can be undone) (Schová tenhle nález u tohoto řádku (jde vrátit)) | LineDetail, Check | ☑ |
| Ignore… (Ignorovat…) | LineDetail, Check | ☑ |
| Fix from the original (Opravit podle originálu) | LineDetail | ☑ |
| Remove the mark (P) (Zrušit označení (P)) | LineDetail | ☑ |
| ✓ Proofread (P) (✓ Zkontrolováno (P)) | LineDetail | ☑ |
| Clear “to fix” (F) (Zrušit „k opravě“ (F)) | LineDetail | ☑ |
| ⚑ To fix (F) (⚑ K opravě (F)) | LineDetail | ☑ |
| ✓ Proofread everything up to here (Shift+P) (✓ Zkontrolovat vše až sem (Shift+P)) | LineDetail | ☑ |
| ↺ Remove the proofread mark up to here (Shift+U) (↺ Zrušit zkontrolování až sem (Shift+U)) | LineDetail | ☑ |
| Writes the text to the .rpy file (a backup is made before writing) (Zapíše text do souboru .rpy (před zápisem se udělá záloha)) | LineDetail | ☑ |
| Save to the file (Ctrl+Enter) (Uložit do souboru (Ctrl+Enter)) | LineDetail | ☑ |
| Restore the text (Vrátit text) | LineDetail | ☑ |
| Copies the text of the translation field to the clipboard (Zkopíruje text z pole překladu do schránky) | LineDetail | ☑ |
| ⧉ Translation (⧉ Překlad) | LineDetail | ☑ |
| Puts the original text into the field (handy as a base with the right tags already) (Vloží text originálu do pole (hodí se jako základ s už správnými značkami)) | LineDetail | ☑ |
| Original into the field (Originál do pole) | LineDetail | ☑ |
| Machine suggestion (Návrh strojem) | LineDetail | ☑ |
| Use the suggestion (Použít návrh) | LineDetail | ☑ |
| Insert (Vložit) | LineDetail | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| More (Více) | LineDetail | ☑ |
| as a base (jako základ) | LineDetail (`Original into the field`) | ☑ |
| ⧉ Copy the translation (⧉ Kopírovat překlad) | LineDetail (`⧉ Translation`) | ☑ |
| ↶ Undo the last file change (↶ Vrátit poslední změnu souboru) | LineDetail, Overview | ☑ |
| More actions (Další akce) | LineDetail (`More actions with the line`) | ☑ |
| Replaces the whole text in the field (Nahradí celý text v poli) | LineDetail | ☑ |
| Apply (Použít) | LineDetail | ☑ |
| Inserts the text at the cursor (Vloží text na místo kurzoru) | LineDetail | ☑ |
| Show the line (Zobrazit řádek) | LineDetail | ☑ |
| Restore into the field (Obnovit do pole) | LineDetail | ☑ |
| ⧉ Copy (⧉ Kopírovat) | LineDetail | ☑ |
| Show the context in the game script (Ukázat kontext ve skriptu hry) | LineDetail | ☑ |
| Table (Tabulka) | LineDetail | ☑ |
| Bubbles (Bubliny) | LineDetail | ☑ |
| Translate the scene with a machine… (Přeložit scénu strojem…) | LineDetail | ☑ |
| Approve identical lines (Schválit shodné řádky) | ApproveIdentical | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Opravy (`23-fixes.js`)

| Current action | New place | Done |
|---|---|---|
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Only what is safe (uncertain suggestions stay unticked) (Jen to, co je bezpečné (nejisté návrhy zůstanou odškrtnuté)) | FixDialog | ☑ |
| Only recommended (Jen doporučené) | CarryDialog, FixDialog | ☑ |
| Fix (Opravit) | FixDialog, Check | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Strojový překlad (`24-machine-translation.js`)

| Current action | New place | Done |
|---|---|---|
| Machine translation is not set up (Strojový překlad není nastavený) | MtDialog | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| Open Settings (Otevřít Nastavení) | MtDialog, StyleDialog, Publish | ☑ |
| Stop (Zastavit) | MtDialog, StyleDialog | ☑ |
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| Unticks the lines where the validator found something (Odškrtne řádky, u kterých validátor něco našel) | MtDialog | ☑ |
| Only without findings (Jen bez nálezů) | MtDialog | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Translate with a machine (Přeložit strojem) | MtDialog | ☑ |

## Najít a nahradit (`25-replace.js`)

| Current action | New place | Done |
|---|---|---|
| Replace (Nahradit) | ReplaceDialog (`Confirm: replace in n lines`) | ☑ |
| All (Vše) | Files, Glossary, Lines | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| Find and replace (Najít a nahradit) | ReplaceDialog | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |

## Schválit podle pravidla (`26-approve-rules.js`)

| Current action | New place | Done |
|---|---|---|
| All shown (Vše zobrazené) | ScopePicker | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Approve by rule (Schválit podle pravidla) | RuleApproveDialog | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Slovník (termíny) (`27-glossary-terms.js`)

| Current action | New place | Done |
|---|---|---|
| Show (Zobrazit) | OccurrencesDialog, Check | ☑ |
| Occurrences of the term (Výskyty termínu) | OccurrencesDialog | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |

## Zálohy (`29-snapshots.js`)

| Current action | New place | Done |
|---|---|---|
| Created (Vytvořeno) | Settings | ☑ |
| Kind (Druh) | MissedTextsDialog, Lines, Settings | ☑ |
| File count (Souborů) | Settings | ☑ |
| Size (Velikost) | Settings | ☑ |
| Restore… (Obnovit…) | Settings | ☑ |
| Delete the backup? (Smazat zálohu?) | Settings → Backups (a second click confirms) | ☑ |
| Delete (Smazat) | Glossary, Settings | ☑ |
| Create a backup now (Vytvořit zálohu teď) | Settings | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Sjednocení překladů (`30-unify.js`)

| Current action | New place | Done |
|---|---|---|
| Show (Zobrazit) | OccurrencesDialog, Check | ☑ |
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Only the lines of speakers who use this variant (Jen řádky mluvčích, kteří tuhle variantu používají) | UnifyDialog | ☑ |
| Only the same speaker (Jen stejný mluvčí) | UnifyDialog | ☑ |
| Unify the translation (Sjednotit překlad) | UnifyDialog | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Kontrola (`31-validation.js`)

| Current action | New place | Done |
|---|---|---|
| Findings of the translation check (Nálezy kontroly překladu) | Check | ☑ |
| Findings (Nálezy) | Check | ☑ |
| The same English line translated differently (Stejný anglický řádek přeložený různě) | Check | ☑ |
| Different translations (Rozdílné překlady) | Check | ☑ |
| Findings you let ignore on some lines (Nálezy, které jsi u některých řádků nechal ignorovat) | Check | ☑ |
| Ignored findings (Ignorované) | Check (`Ignored findings (n)`) | ☑ |
| Game lint (Lint hry) | Check | ☑ |
| Fix… (Opravit…) | LineDetail, Tools, Check | ☑ |
| Running… (Běží…) | Check | ☑ |
| Save the path and run lint (Uložit cestu a spustit lint) | Check | ☑ |
| Show (Zobrazit) | OccurrencesDialog, Check | ☑ |
| Original (Originál) | GamePreview, LineDetail, MtDialog | ☑ |
| Translation variants (Varianty překladu) | Check | ☑ |
| You choose lines and change them to this variant (Vybereš řádky a změníš je na tuhle variantu) | Check | ☑ |
| Unify… (Sjednotit…) | Check | ☑ |
| This group is fine, hide it from the list (Tahle skupina je v pořádku, schovat ji ze seznamu) | Check | ☑ |
| Watch again (Zase hlídat) | Check | ☑ |
| Leave as it is (Nechat jak je) | Check | ☑ |
| State (Stav) | GlossaryImport, OccurrencesDialog, ReleaseDialog | ☑ |
| File : line (Soubor : řádek) | Check | ☑ |
| Finding (Nález) | Check | ☑ |
| Note (Poznámka) | ApproveIdentical, IgnoreDialog, Check | ☑ |
| Translation (Překlad) | GamePreview, LineDetail, OccurrencesDialog | ☑ |
| The finding is reported again (Nález se znovu začne hlásit) | Check | ☑ |
| Stop ignoring (Přestat ignorovat) | LineDetail, Check | ☑ |
| Shows a preview of the fix (Ukáže náhled opravy) | LineDetail, Check | ☑ |
| Fix (Opravit) | FixDialog, Check | ☑ |
| Hides this finding on this line (can be undone) (Schová tenhle nález u tohoto řádku (jde vrátit)) | LineDetail, Check | ☑ |
| Ignore… (Ignorovat…) | LineDetail, Check | ☑ |
| Severity (Závažnost) | Check | ☑ |
| Findings on the line (Nálezy na řádku) | Check | ☑ |
| Hides all findings of this line at once (Schová všechny nálezy tohoto řádku najednou) | Check | ☑ |
| Show in the list of lines (where the text can be edited at once) (Zobrazit v seznamu řádků (tam jde text rovnou upravit)) | Check | ☑ |
| Open in an external editor (Otevřít v externím editoru) | Check | ☑ |
| Editor (Editor) | Check, Settings | ☑ |

## Slovník (glosář) (`32-glossary.js`)

| Current action | New place | Done |
|---|---|---|
| + Add an item (term / name) (+ Přidat položku (termín / jméno)) | Glossary | ☑ |
| Import from characters.rpy… (Importovat z characters.rpy…) | Glossary (`Import names from the game…`, worded for any game) | ☑ |
| Type (Typ) | Glossary, Lines | ☑ |
| Code / term (Kód / termín) | Glossary | ☑ |
| Full name / translation (Celé jméno / překlad) | Glossary | ☑ |
| Note (Poznámka) | ApproveIdentical, IgnoreDialog, Check | ☑ |
| Line count (Řádků) | Glossary | ☑ |
| Example (Příklad) | Glossary | ☑ |
| Occurrences (Výskyty) | Glossary | ☑ |
| Profile… (Profil…) | Glossary | ☑ |
| Show the lines of this speaker (Zobrazit řádky tohoto mluvčího) | Glossary | ☑ |
| Lines (Řádky) | App, Glossary, Publish | ☑ |
| Delete the entry from the glossary (Smazat záznam ze slovníku) | Glossary | ☑ |
| Write to the glossary (Zapsat do slovníku) | GlossaryImport | ☑ |
| Code (Kód) | GlossaryImport, MissedTextsDialog, ProfileDialog | ☑ |
| Name in characters.rpy (Jméno v characters.rpy) | GlossaryImport (`Name in the script`) | ☑ |
| In the glossary so far (Dosud ve slovníku) | GlossaryImport | ☑ |
| File (Soubor) | ApproveIdentical, GlossaryImport, ReleaseDialog | ☑ |
| State (Stav) | GlossaryImport, OccurrencesDialog, ReleaseDialog | ☑ |
| Load (Načíst) | GlossaryImport | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |
| Add (Přidat) | Glossary | ☑ |

## Publikace (`33-publish.js`)

| Current action | New place | Done |
|---|---|---|
| Generate a preview (Vygenerovat náhled) | Publish | ☑ |
| Prepare publishing… (Připravit publikování…) | Publish | ☑ |
| Open Settings (Otevřít Nastavení) | MtDialog, StyleDialog, Publish | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Nastavení (`34-settings.js`)

| Current action | New place | Done |
|---|---|---|
| Translation folder (Složka s překladem) | Settings | ☑ |
| Browse… (Procházet…) | AddGameDialog, CarryDialog, SpellDialog | ☑ |
| Open (Otevřít) | GameCard, Settings | ☑ |
| Save settings (Uložit nastavení) | Settings | ☑ |

## Klávesové zkratky (`35-keyboard.js`)

| Current action | New place | Done |
|---|---|---|
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| lines (řádky) | Overview | ☑ |
| New shortcut (Nová zkratka) | KeysPanel (#126) | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |
| Change (Změnit) | KeysPanel (#126) | ☑ |
| Add a second shortcut (Přidat druhou zkratku) | KeysPanel (#126) | ☑ |
| Remove all shortcuts of this action (Odebrat všechny zkratky této akce) | KeysPanel (#126) | ☑ |
| Remove (Odebrat) | ReleaseDialog, Team | ☑ |
| Restore the default (Vrátit výchozí) | KeysPanel (#126) | ☑ |
| Default (Výchozí) | KeysPanel (#126) | ☑ |
| Restore all shortcuts? (Vrátit všechny zkratky?) | KeysPanel (#126) | ☑ |
| All shortcuts return to the default. (Všechny zkratky se vrátí na výchozí.) | KeysPanel (#126) | ☑ |
| Restore (Vrátit) | KeysPanel (#126) | ☑ |
| Restore all defaults (Vrátit všechny výchozí) | KeysPanel (#126) | ☑ |

## Chybějící texty (`36-missed-texts.js`)

| Current action | New place | Done |
|---|---|---|
| Functions of this game that show text: (Funkce této hry, které zobrazují text:) | MissedTextsDialog | ☑ |
| Save (Uložit) | MissedTextsDialog, ProfileDialog | ☑ |
| Find functions… (Najít funkce…) | MissedTextsDialog | ☑ |
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| Select none (Zrušit výběr) | MissedTextsDialog | ☑ |
| Texts the generator missed (Texty, které generátor vynechal) | MissedTextsDialog | ☑ |
| Experimental (Experimentální) | MissedTextsDialog | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Skupiny (`37-groups.js`)

| Current action | New place | Done |
|---|---|---|
| Groups of the project (Skupiny projektu) | GroupsDialog | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Pro a licence (`38-pro.js`)

| Current action | New place | Done |
|---|---|---|
| Pro feature (Funkce Pro) | ProLocked | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| Name of this computer: (Název tohoto počítače:) | LicencePanel (#126) | ☑ |
| Computer renamed. (Počítač přejmenován.) | LicencePanel (#126) | ☑ |
| Rename (Přejmenovat) | LicencePanel (#126) | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |
| Continue (Pokračovat) | GameCard | ☑ |
| Your new key (Tvůj nový klíč) | LicencePanel (#126) | ☑ |
| Licence activated. (Licence aktivována.) | LicencePanel (#126) | ☑ |
| Activate (Aktivovat) | LicencePanel (#126) | ☑ |
| I lost my key (Ztratil jsem klíč) | LicencePanel (#126) | ☑ |
| Licence checked. (Licence zkontrolována.) | LicencePanel (#126) | ☑ |
| Check now (Zkontrolovat nyní) | Home | ☑ |
| Licence removed from this computer. (Licence odebrána z tohoto počítače.) | LicencePanel (#126) | ☑ |
| Remove from this computer (Odebrat z tohoto počítače) | LicencePanel (#126) | ☑ |

## Přenos překladu (`39-carry.js`)

| Current action | New place | Done |
|---|---|---|
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Only recommended (Jen doporučené) | CarryDialog, FixDialog | ☑ |
| Find (Najít) | CarryDialog | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |

## Profily postav (`40-profiles.js`)

| Current action | New place | Done |
|---|---|---|
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |
| Save (Uložit) | MissedTextsDialog, ProfileDialog | ☑ |

## Kontrola stylu (`41-style-check.js`)

| Current action | New place | Done |
|---|---|---|
| The style check needs a language model (Kontrola stylu potřebuje jazykový model) | StyleDialog | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| Open Settings (Otevřít Nastavení) | MtDialog, StyleDialog, Publish | ☑ |
| Stop (Zastavit) | MtDialog, StyleDialog | ☑ |
| Select all (Vybrat vše) | CarryDialog, FixDialog, MissedTextsDialog | ☑ |
| Unticks the lines where the validator found something in the correction (Odškrtne řádky, kde validátor v opravě něco našel) | StyleDialog | ☑ |
| Only safe ones (Jen bezpečné) | StyleDialog | ☑ |
| None (Nic) | CarryDialog, FixDialog, MtDialog | ☑ |
| Check the style (Zkontrolovat styl) | StyleDialog | ☑ |

## Vydání (`42-release.js`)

| Current action | New place | Done |
|---|---|---|
| Build the release (Sestavit vydání) | ReleaseDialog | ☑ |
| Check (Kontrola) | App, LineDetail, ReleaseDialog | ☑ |
| Remove (Odebrat) | ReleaseDialog, Team | ☑ |
| + Attach a file (+ Přiložit soubor) | ReleaseDialog | ☑ |
| The path is copied. (Cesta je zkopírovaná.) | ReleaseDialog (`Copy the path`) | ☑ |
| Copy the path (Kopírovat cestu) | LineDetail, ReleaseDialog, Files | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |

## Pravopis (`43-spell.js`)

| Current action | New place | Done |
|---|---|---|
| Check (Kontrola) | App, LineDetail, ReleaseDialog | ☑ |
| Enter the word to replace it with. (Zadej slovo, kterým se nahradí.) | SpellDialog | ☑ |
| Replace… (Nahradit…) | SpellDialog | ☑ |
| Do not report this word again in this project (Toto slovo v tomto projektu znovu nehlásit) | SpellDialog | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |
| Cancel (Zrušit) | AddGameDialog, ApproveIdentical, CarryDialog | ☑ |
| Add the dictionary (Přidat slovník) | SpellDialog | ☑ |

## Tým (`44-team.js`)

| Current action | New place | Done |
|---|---|---|
| Yes (Ano) | SureButton | ☑ |
| No (Ne) | SureButton | ☑ |
| Nothing new since your last look. (Od tvého posledního pohledu nic nového.) | Team | ☑ |
| Mark as seen (Označit jako viděné) | Team | ☑ |
| Changes received. (Změny staženy.) | Team | ☑ |
| Get changes (Stáhnout změny) | Team | ☑ |
| Changes sent. (Změny odeslány.) | Team | ☑ |
| Send changes (Odeslat změny) | Team | ☑ |
| Team (Tým) | App, LineDetail, Team | ☑ |
| Close (Zavřít) | Dialog, GlossaryImport, LineDetail | ☑ |

