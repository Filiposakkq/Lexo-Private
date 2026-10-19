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
| `POST release/build` | `{version, notes, force?}` | write the zip, `.sha256` and the release notes, record the release; `force` builds despite the errors of the check of the translation | `release.build` |

A selection is `[{file, line}]`. The corrections of the style check are written with the operation `mt/apply`. A locked feature answers with the message "“…” is a Pro feature."
