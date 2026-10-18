/* Aside website: the numbers you change by hand. Edit this file and publish; every page reads it.
   - program: the current round of the beta programs and how many places are taken
   - roadmap: the cards of roadmap.html; `votes` is the count shown on the card (add the votes that came by e-mail)
   Columns: now, next, later, ideas. Tiers: free, pro, web. */
window.ASIDE_DATA = {
  contact: "aside@filiposakkq.dev",
  author: { name: "filiposakkq", url: "https://filiposakkq.dev" },

  program: {
    round: "Round 1",
    open: true,                       // false closes both applications
    closes: "",                       // optional, e.g. "2026-11-30"; shown as "applications close on …"
    test: { places: 4, taken: 0 },    // testers
    licence: { places: 3, taken: 0 }  // sponsored licences
  },

  roadmap: [
    { col: "now", title: "Installer and switch-over", text: "Per-user installer, tested from the installed copy on every change.", tier: "free" },
    { col: "now", title: "Code signing", text: "So Windows stops warning about the installer.", tier: "free" },
    { col: "now", title: "Updates", text: "Aside tells you about a new version and installs it.", tier: "free" },
    { col: "now", title: "Licence server", text: "Activation, two computers, freeing a computer.", tier: "pro" },

    { col: "next", title: "Game picker", text: "A start screen with your games and translations.", tier: "free", votes: 0 },
    { col: "next", title: "\"Prepare a game\" window", text: "Unpack, decompile and generate translation files with a few clicks.", tier: "free", votes: 0 },
    { col: "next", title: "Refreshed interface", text: "A cleaner look for the editor and the tabs.", tier: "free", votes: 0 },
    { col: "next", title: "User guide in the app", text: "Help where you need it, not only on the website.", tier: "free", votes: 0 },

    { col: "later", title: "Carry over to a new game version", text: "Old translations matched to changed and moved lines, marked for a check.", tier: "pro", votes: 0 },
    { col: "later", title: "Layered images in the scene preview", text: "Draw characters and scenes built from layers.", tier: "free", votes: 0 },
    { col: "later", title: "Fuzzy translation memory", text: "Suggestions from similar lines, not only identical ones.", tier: "pro", votes: 0 },
    { col: "later", title: "Style check", text: "Formal and informal address, gender, character voice, using the language pack.", tier: "pro", votes: 0 },
    { col: "later", title: "Character profiles", text: "Who speaks how, passed to batch machine translation.", tier: "pro", votes: 0 },
    { col: "later", title: "Release builder", text: "From translation to an installable patch with version and changelog.", tier: "pro", votes: 0 },
    { col: "later", title: "Teamwork", text: "Assignments, translator and proofreader roles, merging edits.", tier: "pro", votes: 0 },

    { col: "ideas", title: "More language packs", text: "Rules for more languages, downloadable in the app.", tier: "free", votes: 0 },
    { col: "ideas", title: "Reports and saved views", text: "CSV or PDF reports, saved filter views.", tier: "pro", votes: 0 },
    { col: "ideas", title: "Linux and macOS", text: "Possible with the new core; not promised yet.", tier: "free", votes: 0 },
    { col: "ideas", title: "Czech version of the website", text: "This site, in Czech.", tier: "web", votes: 0 }
  ]
};
