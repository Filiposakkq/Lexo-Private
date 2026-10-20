# UI parity checklist

The new interface (see the sketches discussed in the project: a sidebar with Overview, Files, Lines, Check, Dictionary, Tools, Team, Release, Settings) changes **where** things are and how they look, **never what the application can do**. This list is generated from the current interface: every button and menu action by its current label (Czech text in brackets). Each one must have a place in the new interface before the old one is removed. Fill the column *New place* while designing; the UI self-test and the contract tests must stay green; nothing is deleted without a line here saying where it went.

Generated from the sources of the current UI (`rust/resources/ui-classic/js`); 324 actions in 24 areas. It is a checklist to start from, not an exact inventory: a few actions are built without a plain button label and must be added by hand.

**Progress.** Built in the new interface (`ui-next`): Home, the guide to prepare a game, **Overview, Files and Lines** with the actions listed under those headings *except* the ones named in the "Not yet" part of [UI-NEXT.md](UI-NEXT.md). Their *New place* cells are still to be filled in one by one; nothing is removed from the classic interface.

## Přehled (`20-overview.js`)

| Current action | New place | Done |
|---|---|---|
| Continue proofreading (Pokračovat v korektuře) |  | ☐ |
| No groups yet (Zatím žádné skupiny) |  | ☐ |
| Set up groups… (Nastavit skupiny…) |  | ☐ |
| Start translating (Začít překládat) |  | ☐ |
| Scene and editor over the whole screen, untranslated lines only (Scéna a editor přes celou obrazovku, jen nepřeložené řádky) |  | ☐ |
| Translate quickly (Rychle překládat) |  | ☐ |
| Proofread quickly (Rychle zkontrolovat) |  | ☐ |

## Soubory (`21-files.js`)

| Current action | New place | Done |
|---|---|---|
| File (Soubor) |  | ☐ |
| Assignment (Zařazení) |  | ☐ |
| Translation (Překlad) |  | ☐ |
| Proofreading (Korektura) |  | ☐ |
| Left to translate (Zbývá přeložit) |  | ☐ |
| Left to proofread (Zbývá zkontrolovat) |  | ☐ |
| State (Stav) |  | ☐ |
| Check (Kontrola) |  | ☐ |
| Assign selected… (Zařadit vybrané…) |  | ☐ |
| Suggest assignment… (Navrhnout zařazení…) |  | ☐ |
| What the groups (episodes, chapters…) are called and which numbers exist (Jak se skupiny (epizody, kapitoly…) jmenují a jaká čísla existují) |  | ☐ |
| Groups… (Skupiny…) |  | ☐ |
| Who is in the team of this project and who has which files (Kdo je v týmu tohoto projektu a kdo má které soubory) |  | ☐ |
| Team… (Tým…) |  | ☐ |
| Assigned to (Přiděleno) |  | ☐ |
| Mark the whole file as proofread (Označit celý soubor jako zkontrolovaný) |  | ☐ |
| Show lines (Zobrazit řádky) |  | ☐ |
| Only untranslated (Jen nepřeložené) |  | ☐ |
| Lines that changed after proofreading (Řádky, které se po korektuře změnily) |  | ☐ |
| Continue proofreading (Pokračovat v korektuře) |  | ☐ |
| Open in the editor (Otevřít v editoru) |  | ☐ |
| Copy the path (Kopírovat cestu) |  | ☐ |
| To fix (K opravě) |  | ☐ |
| Show the problems (Zobrazit problémy) |  | ☐ |
| Set up groups… (Nastavit skupiny…) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Řádky (`22-entries.js`)

| Current action | New place | Done |
|---|---|---|
| File (Soubor) |  | ☐ |
| Line (Řádek) |  | ☐ |
| Line in the translation file (Řádek v překladovém souboru) |  | ☐ |
| Script (Skript) |  | ☐ |
| Type (Typ) |  | ☐ |
| State (Stav) |  | ☐ |
| Proofreading (Korektura) |  | ☐ |
| Code (Kód) |  | ☐ |
| Speaker abbreviation (and image attributes) (Zkratka mluvčího (a atributy obrázku)) |  | ☐ |
| Name (Jméno) |  | ☐ |
| Full name from the Glossary (Celé jméno ze Slovníku) |  | ☐ |
| Original (Originál) |  | ☐ |
| Translation (Překlad) |  | ☐ |
| Check (Kontrola) |  | ☐ |
| Line type, kind, check, sorting, speaker display (Typ řádku, druh, kontrola, řazení, zobrazení mluvčích) |  | ☐ |
| Find and replace… (Najít a nahradit…) |  | ☐ |
| in the translations (v překladech) |  | ☐ |
| Approve by rule… (Schválit podle pravidla…) |  | ☐ |
| names, interjections (jména, citoslovce) |  | ☐ |
| Texts the generator missed… (Texty, které generátor vynechal…) |  | ☐ |
| show text, input prompts, titles (show text, výzvy k zadání, názvy) |  | ☐ |
| Carry over from the previous version… (Přenést překlad z předchozí verze…) |  | ☐ |
| Pro: translation of the old game version (Pro: překlad staré verze hry) |  | ☐ |
| Fill from similar translations… (Doplnit z podobných překladů…) |  | ☐ |
| Pro: similar originals of this project (Pro: podobné originály v tomto projektu) |  | ☐ |
| Fix… (Opravit…) |  | ☐ |
| speaker, spaces, typos (mluvčí, mezery, překlepy) |  | ☐ |
| Translate with a machine… (Přeložit strojem…) |  | ☐ |
| selected lines, scene, file (vybrané řádky, scéna, soubor) |  | ☐ |
| Check the style… (Zkontrolovat styl…) |  | ☐ |
| Pro: address, gender, tone (language model) (Pro: oslovení, rod, tón (jazykový model)) |  | ☐ |
| Check the spelling… (Zkontrolovat pravopis…) |  | ☐ |
| Pro: with a dictionary of the language (Pro: se slovníkem jazyka) |  | ☐ |
| Build a release… (Sestavit vydání…) |  | ☐ |
| Pro: a zip for the players (Pro: zip pro hráče) |  | ☐ |
| Clear the selection (Zrušit výběr) |  | ☐ |
| Unapprove (A) (Zrušit schválení (A)) |  | ☐ |
| Approve (A) (Schválit (A)) |  | ☐ |
| Opens the file at this line in an external editor (Otevře soubor na tomhle řádku v externím editoru) |  | ☐ |
| Editor (E) (Editor (E)) |  | ☐ |
| Approve all identical… (Schválit všechny shodné…) |  | ☐ |
| Copy the path (Kopírovat cestu) |  | ☐ |
| Copies the original text to the clipboard (Zkopíruje text originálu do schránky) |  | ☐ |
| ⧉ Original (⧉ Originál) |  | ☐ |
| Copies the original and the translation one under the other (Zkopíruje originál a překlad pod sebe) |  | ☐ |
| ⧉ Both (⧉ Oboje) |  | ☐ |
| First line of the previous scene (První řádek předchozí scény) |  | ☐ |
| ◀ Scene (◀ Scéna) |  | ☐ |
| First line of the next scene (První řádek další scény) |  | ☐ |
| Scene ▶ (Scéna ▶) |  | ☐ |
| Next untranslated line in the file (Další nepřeložený řádek v souboru) |  | ☐ |
| Next untranslated (Další nepřeložený) |  | ☐ |
| Next translated but unchecked line (Další přeložený, ale nezkontrolovaný řádek) |  | ☐ |
| Next unchecked (Další nezkontrolovaný) |  | ☐ |
| Next line that does not fit the game window (Další řádek, který se nevejde do okna hry) |  | ☐ |
| Next one that does not fit (Další, co se nevejde) |  | ☐ |
| Load more (Načíst další) |  | ☐ |
| Hide the list (Skrýt seznam) |  | ☐ |
| Show the list (Zobrazit seznam) |  | ☐ |
| Previous line (←) (Předchozí řádek (←)) |  | ☐ |
| Next line (→) (Další řádek (→)) |  | ☐ |
| Show them here too (Ukázat je i tady) |  | ☐ |
| Scene and detail (Scéna a detail) |  | ☐ |
| Scene over the whole window (Scéna přes celé okno) |  | ☐ |
| Stop ignoring (Přestat ignorovat) |  | ☐ |
| Hides this finding on this line (can be undone) (Schová tenhle nález u tohoto řádku (jde vrátit)) |  | ☐ |
| Ignore… (Ignorovat…) |  | ☐ |
| Fix from the original (Opravit podle originálu) |  | ☐ |
| Remove the mark (P) (Zrušit označení (P)) |  | ☐ |
| ✓ Proofread (P) (✓ Zkontrolováno (P)) |  | ☐ |
| Clear “to fix” (F) (Zrušit „k opravě“ (F)) |  | ☐ |
| ⚑ To fix (F) (⚑ K opravě (F)) |  | ☐ |
| ✓ Proofread everything up to here (Shift+P) (✓ Zkontrolovat vše až sem (Shift+P)) |  | ☐ |
| ↺ Remove the proofread mark up to here (Shift+U) (↺ Zrušit zkontrolování až sem (Shift+U)) |  | ☐ |
| Writes the text to the .rpy file (a backup is made before writing) (Zapíše text do souboru .rpy (před zápisem se udělá záloha)) |  | ☐ |
| Save to the file (Ctrl+Enter) (Uložit do souboru (Ctrl+Enter)) |  | ☐ |
| Restore the text (Vrátit text) |  | ☐ |
| Copies the text of the translation field to the clipboard (Zkopíruje text z pole překladu do schránky) |  | ☐ |
| ⧉ Translation (⧉ Překlad) |  | ☐ |
| Puts the original text into the field (handy as a base with the right tags already) (Vloží text originálu do pole (hodí se jako základ s už správnými značkami)) |  | ☐ |
| Original into the field (Originál do pole) |  | ☐ |
| Machine suggestion (Návrh strojem) |  | ☐ |
| Use the suggestion (Použít návrh) |  | ☐ |
| Insert (Vložit) |  | ☐ |
| Close (Zavřít) |  | ☐ |
| More (Více) |  | ☐ |
| as a base (jako základ) |  | ☐ |
| ⧉ Copy the translation (⧉ Kopírovat překlad) |  | ☐ |
| ↶ Undo the last file change (↶ Vrátit poslední změnu souboru) |  | ☐ |
| More actions (Další akce) |  | ☐ |
| Replaces the whole text in the field (Nahradí celý text v poli) |  | ☐ |
| Apply (Použít) |  | ☐ |
| Inserts the text at the cursor (Vloží text na místo kurzoru) |  | ☐ |
| Show the line (Zobrazit řádek) |  | ☐ |
| Restore into the field (Obnovit do pole) |  | ☐ |
| ⧉ Copy (⧉ Kopírovat) |  | ☐ |
| Show the context in the game script (Ukázat kontext ve skriptu hry) |  | ☐ |
| Table (Tabulka) |  | ☐ |
| Bubbles (Bubliny) |  | ☐ |
| Translate the scene with a machine… (Přeložit scénu strojem…) |  | ☐ |
| Approve identical lines (Schválit shodné řádky) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Opravy (`23-fixes.js`)

| Current action | New place | Done |
|---|---|---|
| Select all (Vybrat vše) |  | ☐ |
| None (Nic) |  | ☐ |
| Only what is safe (uncertain suggestions stay unticked) (Jen to, co je bezpečné (nejisté návrhy zůstanou odškrtnuté)) |  | ☐ |
| Only recommended (Jen doporučené) |  | ☐ |
| Fix (Opravit) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Strojový překlad (`24-machine-translation.js`)

| Current action | New place | Done |
|---|---|---|
| Machine translation is not set up (Strojový překlad není nastavený) |  | ☐ |
| Close (Zavřít) |  | ☐ |
| Open Settings (Otevřít Nastavení) |  | ☐ |
| Stop (Zastavit) |  | ☐ |
| Select all (Vybrat vše) |  | ☐ |
| Unticks the lines where the validator found something (Odškrtne řádky, u kterých validátor něco našel) |  | ☐ |
| Only without findings (Jen bez nálezů) |  | ☐ |
| None (Nic) |  | ☐ |
| Translate with a machine (Přeložit strojem) |  | ☐ |

## Najít a nahradit (`25-replace.js`)

| Current action | New place | Done |
|---|---|---|
| Replace (Nahradit) |  | ☐ |
| All (Vše) |  | ☐ |
| None (Nic) |  | ☐ |
| Select all (Vybrat vše) |  | ☐ |
| Find and replace (Najít a nahradit) |  | ☐ |
| Close (Zavřít) |  | ☐ |

## Schválit podle pravidla (`26-approve-rules.js`)

| Current action | New place | Done |
|---|---|---|
| All shown (Vše zobrazené) |  | ☐ |
| None (Nic) |  | ☐ |
| Approve by rule (Schválit podle pravidla) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Slovník (termíny) (`27-glossary-terms.js`)

| Current action | New place | Done |
|---|---|---|
| Show (Zobrazit) |  | ☐ |
| Occurrences of the term (Výskyty termínu) |  | ☐ |
| Close (Zavřít) |  | ☐ |

## Zálohy (`29-snapshots.js`)

| Current action | New place | Done |
|---|---|---|
| Created (Vytvořeno) |  | ☐ |
| Kind (Druh) |  | ☐ |
| File count (Souborů) |  | ☐ |
| Size (Velikost) |  | ☐ |
| Restore… (Obnovit…) |  | ☐ |
| Delete the backup? (Smazat zálohu?) |  | ☐ |
| Delete (Smazat) |  | ☐ |
| Create a backup now (Vytvořit zálohu teď) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Sjednocení překladů (`30-unify.js`)

| Current action | New place | Done |
|---|---|---|
| Show (Zobrazit) |  | ☐ |
| Select all (Vybrat vše) |  | ☐ |
| None (Nic) |  | ☐ |
| Only the lines of speakers who use this variant (Jen řádky mluvčích, kteří tuhle variantu používají) |  | ☐ |
| Only the same speaker (Jen stejný mluvčí) |  | ☐ |
| Unify the translation (Sjednotit překlad) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Kontrola (`31-validation.js`)

| Current action | New place | Done |
|---|---|---|
| Findings of the translation check (Nálezy kontroly překladu) |  | ☐ |
| Findings (Nálezy) |  | ☐ |
| The same English line translated differently (Stejný anglický řádek přeložený různě) |  | ☐ |
| Different translations (Rozdílné překlady) |  | ☐ |
| Findings you let ignore on some lines (Nálezy, které jsi u některých řádků nechal ignorovat) |  | ☐ |
| Ignored findings (Ignorované) |  | ☐ |
| Game lint (Lint hry) |  | ☐ |
| Fix… (Opravit…) |  | ☐ |
| Running… (Běží…) |  | ☐ |
| Save the path and run lint (Uložit cestu a spustit lint) |  | ☐ |
| Show (Zobrazit) |  | ☐ |
| Original (Originál) |  | ☐ |
| Translation variants (Varianty překladu) |  | ☐ |
| You choose lines and change them to this variant (Vybereš řádky a změníš je na tuhle variantu) |  | ☐ |
| Unify… (Sjednotit…) |  | ☐ |
| This group is fine, hide it from the list (Tahle skupina je v pořádku, schovat ji ze seznamu) |  | ☐ |
| Watch again (Zase hlídat) |  | ☐ |
| Leave as it is (Nechat jak je) |  | ☐ |
| State (Stav) |  | ☐ |
| File : line (Soubor : řádek) |  | ☐ |
| Finding (Nález) |  | ☐ |
| Note (Poznámka) |  | ☐ |
| Translation (Překlad) |  | ☐ |
| The finding is reported again (Nález se znovu začne hlásit) |  | ☐ |
| Stop ignoring (Přestat ignorovat) |  | ☐ |
| Shows a preview of the fix (Ukáže náhled opravy) |  | ☐ |
| Fix (Opravit) |  | ☐ |
| Hides this finding on this line (can be undone) (Schová tenhle nález u tohoto řádku (jde vrátit)) |  | ☐ |
| Ignore… (Ignorovat…) |  | ☐ |
| Severity (Závažnost) |  | ☐ |
| Findings on the line (Nálezy na řádku) |  | ☐ |
| Hides all findings of this line at once (Schová všechny nálezy tohoto řádku najednou) |  | ☐ |
| Show in the list of lines (where the text can be edited at once) (Zobrazit v seznamu řádků (tam jde text rovnou upravit)) |  | ☐ |
| Open in an external editor (Otevřít v externím editoru) |  | ☐ |
| Editor (Editor) |  | ☐ |

## Slovník (glosář) (`32-glossary.js`)

| Current action | New place | Done |
|---|---|---|
| + Add an item (term / name) (+ Přidat položku (termín / jméno)) |  | ☐ |
| Import from characters.rpy… (Importovat z characters.rpy…) |  | ☐ |
| Type (Typ) |  | ☐ |
| Code / term (Kód / termín) |  | ☐ |
| Full name / translation (Celé jméno / překlad) |  | ☐ |
| Note (Poznámka) |  | ☐ |
| Line count (Řádků) |  | ☐ |
| Example (Příklad) |  | ☐ |
| Occurrences (Výskyty) |  | ☐ |
| Profile… (Profil…) |  | ☐ |
| Show the lines of this speaker (Zobrazit řádky tohoto mluvčího) |  | ☐ |
| Lines (Řádky) |  | ☐ |
| Delete the entry from the glossary (Smazat záznam ze slovníku) |  | ☐ |
| Write to the glossary (Zapsat do slovníku) |  | ☐ |
| Code (Kód) |  | ☐ |
| Name in characters.rpy (Jméno v characters.rpy) |  | ☐ |
| In the glossary so far (Dosud ve slovníku) |  | ☐ |
| File (Soubor) |  | ☐ |
| State (Stav) |  | ☐ |
| Load (Načíst) |  | ☐ |
| Close (Zavřít) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |
| Add (Přidat) |  | ☐ |

## Publikace (`33-publish.js`)

| Current action | New place | Done |
|---|---|---|
| Generate a preview (Vygenerovat náhled) |  | ☐ |
| Prepare publishing… (Připravit publikování…) |  | ☐ |
| Open Settings (Otevřít Nastavení) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Nastavení (`34-settings.js`)

| Current action | New place | Done |
|---|---|---|
| Translation folder (Složka s překladem) |  | ☐ |
| Browse… (Procházet…) |  | ☐ |
| Open (Otevřít) |  | ☐ |
| Save settings (Uložit nastavení) |  | ☐ |

## Klávesové zkratky (`35-keyboard.js`)

| Current action | New place | Done |
|---|---|---|
| Close (Zavřít) |  | ☐ |
| lines (řádky) |  | ☐ |
| New shortcut (Nová zkratka) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |
| Change (Změnit) |  | ☐ |
| Add a second shortcut (Přidat druhou zkratku) |  | ☐ |
| Remove all shortcuts of this action (Odebrat všechny zkratky této akce) |  | ☐ |
| Remove (Odebrat) |  | ☐ |
| Restore the default (Vrátit výchozí) |  | ☐ |
| Default (Výchozí) |  | ☐ |
| Restore all shortcuts? (Vrátit všechny zkratky?) |  | ☐ |
| All shortcuts return to the default. (Všechny zkratky se vrátí na výchozí.) |  | ☐ |
| Restore (Vrátit) |  | ☐ |
| Restore all defaults (Vrátit všechny výchozí) |  | ☐ |

## Chybějící texty (`36-missed-texts.js`)

| Current action | New place | Done |
|---|---|---|
| Functions of this game that show text: (Funkce této hry, které zobrazují text:) |  | ☐ |
| Save (Uložit) |  | ☐ |
| Find functions… (Najít funkce…) |  | ☐ |
| Select all (Vybrat vše) |  | ☐ |
| Select none (Zrušit výběr) |  | ☐ |
| Texts the generator missed (Texty, které generátor vynechal) |  | ☐ |
| Experimental (Experimentální) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Skupiny (`37-groups.js`)

| Current action | New place | Done |
|---|---|---|
| Groups of the project (Skupiny projektu) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Pro a licence (`38-pro.js`)

| Current action | New place | Done |
|---|---|---|
| Pro feature (Funkce Pro) |  | ☐ |
| Close (Zavřít) |  | ☐ |
| Name of this computer: (Název tohoto počítače:) |  | ☐ |
| Computer renamed. (Počítač přejmenován.) |  | ☐ |
| Rename (Přejmenovat) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |
| Continue (Pokračovat) |  | ☐ |
| Your new key (Tvůj nový klíč) |  | ☐ |
| Licence activated. (Licence aktivována.) |  | ☐ |
| Activate (Aktivovat) |  | ☐ |
| I lost my key (Ztratil jsem klíč) |  | ☐ |
| Licence checked. (Licence zkontrolována.) |  | ☐ |
| Check now (Zkontrolovat nyní) |  | ☐ |
| Licence removed from this computer. (Licence odebrána z tohoto počítače.) |  | ☐ |
| Remove from this computer (Odebrat z tohoto počítače) |  | ☐ |

## Přenos překladu (`39-carry.js`)

| Current action | New place | Done |
|---|---|---|
| Select all (Vybrat vše) |  | ☐ |
| None (Nic) |  | ☐ |
| Only recommended (Jen doporučené) |  | ☐ |
| Find (Najít) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |

## Profily postav (`40-profiles.js`)

| Current action | New place | Done |
|---|---|---|
| Cancel (Zrušit) |  | ☐ |
| Save (Uložit) |  | ☐ |

## Kontrola stylu (`41-style-check.js`)

| Current action | New place | Done |
|---|---|---|
| The style check needs a language model (Kontrola stylu potřebuje jazykový model) |  | ☐ |
| Close (Zavřít) |  | ☐ |
| Open Settings (Otevřít Nastavení) |  | ☐ |
| Stop (Zastavit) |  | ☐ |
| Select all (Vybrat vše) |  | ☐ |
| Unticks the lines where the validator found something in the correction (Odškrtne řádky, kde validátor v opravě něco našel) |  | ☐ |
| Only safe ones (Jen bezpečné) |  | ☐ |
| None (Nic) |  | ☐ |
| Check the style (Zkontrolovat styl) |  | ☐ |

## Vydání (`42-release.js`)

| Current action | New place | Done |
|---|---|---|
| Build the release (Sestavit vydání) |  | ☐ |
| Check (Kontrola) |  | ☐ |
| Remove (Odebrat) |  | ☐ |
| + Attach a file (+ Přiložit soubor) |  | ☐ |
| The path is copied. (Cesta je zkopírovaná.) |  | ☐ |
| Copy the path (Kopírovat cestu) |  | ☐ |
| Close (Zavřít) |  | ☐ |

## Pravopis (`43-spell.js`)

| Current action | New place | Done |
|---|---|---|
| Check (Kontrola) |  | ☐ |
| Enter the word to replace it with. (Zadej slovo, kterým se nahradí.) |  | ☐ |
| Replace… (Nahradit…) |  | ☐ |
| Do not report this word again in this project (Toto slovo v tomto projektu znovu nehlásit) |  | ☐ |
| Close (Zavřít) |  | ☐ |
| Cancel (Zrušit) |  | ☐ |
| Add the dictionary (Přidat slovník) |  | ☐ |

## Tým (`44-team.js`)

| Current action | New place | Done |
|---|---|---|
| Yes (Ano) |  | ☐ |
| No (Ne) |  | ☐ |
| Nothing new since your last look. (Od tvého posledního pohledu nic nového.) |  | ☐ |
| Mark as seen (Označit jako viděné) |  | ☐ |
| Changes received. (Změny staženy.) |  | ☐ |
| Get changes (Stáhnout změny) |  | ☐ |
| Changes sent. (Změny odeslány.) |  | ☐ |
| Send changes (Odeslat změny) |  | ☐ |
| Team (Tým) |  | ☐ |
| Close (Zavřít) |  | ☐ |

