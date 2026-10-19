# What the application serves outside the contract of Core

The 80 operations of Core ([API.md](API.md), generated from `testdata/contract/api.json`) are the same in every client. The routes below belong to **the application around Core** (`rust/crates/lexo-server/src/dispatch.rs`; the Tauri app serves the same routes through its own protocol): windows and the folder dialog, the licence of the Pro pack, and the Pro features that are not in Core's contract. They are listed in `reference/python/free/tests/test_contract_ui.py` (`SESSION`) so that the UI test knows they are allowed.

Every `POST` needs the session token (the header the page sends); the answers are JSON, `{"ok": true, ...}` or an error with a message that is already translated.

## The window and the session
| Route | What |
|---|---|
| `GET ping?w=&k=` | a window says it is alive (`k=1` marks a companion window); the server ends when no window pinged for a while |
| `POST bye?t=&w=` | a window closes (sent with `sendBeacon`) |
| `GET focus` / `POST focus {file, line, text?}` | the line open in the main window, which the companion windows follow (a long poll) |
| `POST window/open` | open one more window of the application |
| `POST window/fullscreen` | toggle full screen of the window in front (`{ok: false, fallback: true}` when the page has to use the browser's way) |
| `POST project/pick` | the folder dialog, then open the folder as the project |

## The licence of the Pro pack (without the pack: `{state: "none"}`)
| Route | Body | What |
|---|---|---|
| `GET licence` | | the state: `none`, `demo`, `licensed` (id, days to the next check), `grace`, `expired`, `refused` (reason) |
| `POST licence/activate` | `{key, name?}` | take a place for this computer; the optional name of the computer |
| `POST licence/refresh` | | the regular check now |
| `POST licence/deactivate` | | free the place of this computer and remove the file |
| `POST licence/devices` | | the computers that hold the licence: id, name, last seen, current; places; how many releases are left |
| `POST licence/release` | `{target}` | free another computer (twice for the whole life of the licence) |
| `POST licence/rename` | `{name}` | the name of this computer |
| `POST licence/recover` | `{email}` | a lost key, step one: a code is mailed to the address of the purchase |
| `POST licence/recover-confirm` | `{email, code}` | step two: the key is replaced, the new key is returned |

## Pro features
| Route | Body | What | Feature |
|---|---|---|---|
| `POST carry/pick` | | the folder dialog for the old version | `carry.over` |
| `POST carry/preview` | `{old, min_ratio?}` | the matches of the untranslated lines in the old folder (`items`, `counts`, `limited?`) | `carry.over` |
| `POST carry/apply` | `{old, selections, mark?, min_ratio?}` | write the ticked lines; `mark` (default true) marks changed originals "to fix"; answers `changed`, `files`, `marked` | `carry.over` |
| `POST memory/fill-preview` | `{min_ratio?}` | the same, from the project's own translations (ratio from 0.85) | `memory.fuzzy` |
| `POST memory/fill-apply` | `{selections, mark?, min_ratio?}` | write them | `memory.fuzzy` |
| `POST profile/list` | | the speakers with their profiles | `mt.profiles` |
| `POST profile/set` | `{code, gender, style, to}` | write the profile of a speaker (empty = remove) | `mt.profiles` |
| `POST style/targets` | `{file?, lines?}` | the translated lines a check would look at (cut to the limit of the licence) | `style.check` |
| `POST style/batch` | `{items}` | verdicts for up to 5 lines `{file, line, ok, remarks, fix, issues}` or an error that stops the batch | `style.check` |

| `POST release/state` | | the settings, the releases so far, the game (name and version from `config.*`), the language | `release.build` |
| `POST release/settings` | `{settings}` | save the attached files, the options, credits, the output folder (`.aside/release.json`) | `release.build` |
| `POST release/preview` | `{version, notes}` | the checks, the changes since the last release, the texts; nothing is written | `release.build` |
| `POST team/state` | | `{exists, me: {author, name, joined, lead}, members: [{author, name, role, me, files}], assign: {file: [author]}, my_files, can_manage}` | |
| `POST team/join` | `{name}` | write your own file `.aside/team/members/<author>.json` | |
| `POST team/create` | | make the project a team with you as the lead (`.aside/team/lead.json`) | `team.manage` |
| `POST team/set` | `{roles, assign}` | the lead's roles and assignments; the pack cleans them; `dropped` counts what it removed | `team.manage` |
| `POST team/leave` | | leave the team: your own file in `members/` is deleted (not for the lead) | |
| `POST team/remove` | `{author}` | the lead removes a person (listed in `removed` of `lead.json`; their role and files are dropped) | `team.manage` |
| `POST team/restore` | `{author}` | the lead takes a removed person back | `team.manage` |
| `POST team/transfer` | `{author}` | hand the team over: the person becomes the lead, you become a translator | `team.manage` |
| `POST team/disband` | | end the team: `lead.json` is deleted | `team.manage` |
| `POST team/stats` | `{days?}` | statistics per person for the lead: `{members: [{author, name, role, files, lines, done_pct, proof_pct, edited, approved, marked, proofread_updates, last}], unassigned}` | `team.manage` |
| `POST team/notes` | `{file, line}` | the notes of a line, oldest first: `{exists, notes: [{id, author, name, t, text, resolved}]}` | |
| `POST team/note_add` | `{file, line, text}` | a note to the team on a line (own file `.aside/team/notes/<author>.jsonl`) | |
| `POST team/note_resolve` | `{id, file, line}` | mark a note as dealt with (a line of your own file) | |
| `POST team/changes` | | what the others did since your last look: `{since, items: [{author, name, what, file, n}], notes, total}` | |
| `POST team/seen` | | move the point of your last look to now (local file `team-seen.json`) | |
| `POST spell/state` | | the language of the project, whether a dictionary is added (and its name), how many words are ignored | |
| `POST spell/add` | `{path}` | copy the dictionary (a `.dic` file or a folder with one; the `.aff` file next to it) to the settings of the application; the pack reads it once to prove it works | `spell.hunspell` |
| `POST spell/remove` | | remove the dictionary of this language | |
| `POST spell/ignore` | `{words}` | remember words to ignore (`.aside/pro/spelling.json`) | |
| `POST spell/check` | `{file?, lines?}` | the words the dictionary does not know: `{items: [{word, count, places, suggestions}], words, truncated}` | `spell.hunspell` |
| `POST spell/replace` | `{word, to}` | the lines that would change (the word whole, not in tags or variables): `{items: [{file, line, before, after, approved}], total}`; written with `mt/apply` | `spell.hunspell` |
| `POST release/build` | `{version, notes, force?}` | write the zip, `.sha256` and the release notes, record the release; `force` builds despite the errors of the check of the translation | `release.build` |

A selection is `[{file, line}]`. The corrections of the style check are written with the operation `mt/apply`. A locked feature answers with the message "“…” is a Pro feature."
