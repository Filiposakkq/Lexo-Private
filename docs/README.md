# Documents

A map of the documents. **Current** = describes how things are now. **Plan** = a design or a plan, partly done. **History** = how it came to be; read it for the reasons, not for the state.

| Document | Question it answers | State |
|---|---|---|
| [HANDOFF.md](HANDOFF.md) | What is this project, what was decided, what are the rules, how is a step done, what is the state now (start here) | current |
| [ROADMAP.md](ROADMAP.md) | The plan in order, what is done (the table "Where we are" and the log) | current (the early steps are history) |
| [FEATURES.md](FEATURES.md) | Which features are Free and which are Pro, and the status of each | current |
| [PRO-FEATURES.md](PRO-FEATURES.md) | What each Pro feature does, where it is, the demo limits, what it sends over the network, how to undo it | current |
| [PRO-DESIGN.md](PRO-DESIGN.md) | How Pro is kept apart, delivered, licensed, limited; the licence server; what is done and open | current |
| [PRIVACY.md](PRIVACY.md) | Everything that leaves the user's computer, and what the servers store | current (changes with the code, in the same pull request) |
| [REPO-CLEANUP.md](REPO-CLEANUP.md) | What is untidy in the repository and the plan to tidy it | plan |
| [RUST-PORT.md](RUST-PORT.md) | The port of Core to Rust: milestones, the progress table, how the Pro pack connects | plan, nearly all done |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Layers, the interface of Core, how Free and Pro attach | current for the ideas, Python-era for the file names |
| [API.md](API.md) | The 80 operations of Core (generated from `testdata/contract`) | current (generated) |
| [GOLDEN.md](GOLDEN.md) | The golden data: inputs, recorded answers, how the Rust port is checked | current |
| [DATA-LAYOUT.md](DATA-LAYOUT.md) | Where the data of a project lives (`.aside/`, the local data, the event log) | current |
| [PROJECT-FILE.md](PROJECT-FILE.md) | `.aside/aside.json`: groups, language | current |
| [UI-TEXTS.md](UI-TEXTS.md) | How the texts of the UI are translated (`t`, `msg`, packs) and the tests that guard it | current |
| [GAME-TOOLS.md](GAME-TOOLS.md) | `.rpa` extraction, unrpyc, generating translation files, scene preview, layered images, menus | current |
| [TEAMWORK.md](TEAMWORK.md) | How several translators would work together | plan (not built) |
| [GAME-TESTS.md](GAME-TESTS.md) | The eight games the application was tried on and what they showed | history |
| [HARDCODED-AUDIT.md](HARDCODED-AUDIT.md) | What was tied to one game or one language, and what is left on purpose | history |

The documents of the parts live next to them: [`../server/README.md`](../server/README.md) (licence server and its admin page), [`../web/README.md`](../web/README.md) (website),
[`../free/assets/brand/README.md`](../free/assets/brand/README.md) (brand).

**Rules for the documents:** English; one fact in one place (link instead of copying); a document that describes behaviour is changed in the same pull request as the behaviour.
