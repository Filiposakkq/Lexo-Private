# Documents

The documents are in five folders by who needs them. **Current** = describes how things are now. **Plan** = a design or a plan, partly done. **History** = how it came to be; read it for the reasons, not for the state.

## `guide/`: how to use the application
| Document | Question it answers | State |
|---|---|---|
| [USER-GUIDE.md](guide/USER-GUIDE.md) | Everything the application can do, by tab: the ideas, Overview, Files, Lines (the editor), the tools, Check, Glossary, Publishing, Settings, shortcuts, what can be undone | current |
| [TEAMS.md](guide/TEAMS.md) | Teams in full: set-up step by step, roles, files, news, notes, statistics, the data, problems | current |
| [PRO-FEATURES.md](guide/PRO-FEATURES.md) | What each Pro feature does, where it is, the demo limits, what it sends over the network, how to undo it | current |

The first walk-through for a new user (install, open a game, the first translation, troubleshooting) is the documentation page of the website: [`../web/public/docs.html`](../web/public/docs.html).

## `product/`: what the product is, what it costs, what it promises
| Document | Question it answers | State |
|---|---|---|
| [FEATURES.md](product/FEATURES.md) | Which features are Free and which are Pro, and the status of each | current |
| [PRO-DESIGN.md](product/PRO-DESIGN.md) | How Pro is kept apart, delivered, licensed and limited; the licence server; what is done and open | current |
| [PRIVACY.md](product/PRIVACY.md) | Everything that leaves the user's computer, and what the servers store | current (changes with the code, in the same pull request) |
| [TEAMWORK.md](product/TEAMWORK.md) | Design of team work: why no server, what is built, what is planned | version 1 built |

## `technical/`: how it is built
| Document | Question it answers | State |
|---|---|---|
| [ARCHITECTURE.md](technical/ARCHITECTURE.md) | Layers, the interface of Core, how Free and Pro attach | current for the ideas, Python-era for the file names |
| [API.md](technical/API.md) | The 80 operations of Core (generated from `testdata/contract`) | current (generated) |
| [APP-ROUTES.md](technical/APP-ROUTES.md) | What the application serves outside the contract of Core: windows, the licence, the Pro features | current |
| [DATA-LAYOUT.md](technical/DATA-LAYOUT.md) | Where the data of a project lives (`.aside/`, the local data, the event log) | current |
| [PROJECT-FILE.md](technical/PROJECT-FILE.md) | `.aside/aside.json`: groups, language | current |
| [UI-TEXTS.md](technical/UI-TEXTS.md) | How the texts of the UI are translated (`t`, `msg`, packs) and the tests that guard it | current |
| [GAME-TOOLS.md](technical/GAME-TOOLS.md) | `.rpa` extraction, unrpyc, generating translation files, scene preview, layered images, menus | current |
| [GOLDEN.md](technical/GOLDEN.md) | The golden data: inputs, recorded answers, how the Rust port is checked | current |
| [RUST-PORT.md](technical/RUST-PORT.md) | The port of Core to Rust: milestones, the progress table, how the Pro pack connects | plan, nearly all done |

## `project/`: how the work is done
| Document | Question it answers | State |
|---|---|---|
| [HANDOFF.md](project/HANDOFF.md) | What is this project, what was decided, what are the rules, how is a step done, what is the state now (start here) | current |
| [ROADMAP.md](project/ROADMAP.md) | The plan in order, what is done (the table "Where we are" and the log) | current (the early steps are history) |
| [REPO-CLEANUP.md](project/REPO-CLEANUP.md) | What was untidy in the repository, what was moved, what is open | current |
| [UI-PARITY.md](project/UI-PARITY.md) | Checklist of every action of the current interface: each must have a place in the new interface (nothing is lost) | working list |

## `history/`: how it came to be
| Document | Question it answers | State |
|---|---|---|
| [GAME-TESTS.md](history/GAME-TESTS.md) | The eight games the application was tried on and what they showed | history |
| [HARDCODED-AUDIT.md](history/HARDCODED-AUDIT.md) | What was tied to one game or one language, and what is left on purpose | history |

The documents of the parts live next to them: [`../server/README.md`](../server/README.md) (licence server and its admin page), [`../web/README.md`](../web/README.md) (website),
[`../rust/resources/assets/brand/README.md`](../rust/resources/assets/brand/README.md) (brand).

**Rules for the documents:** English; one fact in one place (link instead of copying); a document that describes behaviour is changed in the same pull request as the behaviour; a new document goes into the folder of the question it answers and into this index.
