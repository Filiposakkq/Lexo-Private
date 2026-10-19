# What Aside sends, and where

This is the list of everything that leaves the computer of the user, written from the code (Rust application, Pro pack, licence server). It is the base for the privacy page of the website and for the answers of the support. When the code changes what it sends, this file changes in the same pull request.

**Short version:** Aside has **no telemetry, no analytics, no crash reports and no check for updates** (an update check is not built yet; when it comes it is added here). The translation, the game files and the project data never go to the Aside server. Aside talks to three kinds of places, and only for the reasons below.

## 1. The licence server (only with the Pro pack)

Address: `https://api.aside.filiposakkq.dev` (Cloudflare Worker). Contacted when the user enters a key, at the start of the Pro pack and then when the licence file is older than six hours (at most once in about six hours), when the user presses "Check now", opens the list of computers, frees a computer or removes the licence from this computer.

| Call | What is sent |
|---|---|
| activate | the **licence key** (once), the **device id**, the optional **name** of the computer (typed by the user), the **version** of the Pro pack |
| regular check (refresh) | the signed **licence file** (licence id, features, version, device id, time of issue, check interval), the device id, the name, the pack version |
| lost key (Pro, "I lost my key") | the **e-mail address** the user types (to get a code), and then the address and the **code** (to get a new key). The server never confirms whether an address belongs to a customer |
| list of computers, freeing a computer, removing the licence | the licence file, the device id, for freeing another computer also its device id |

- The **device id** is a hash of a random number that Aside makes once and keeps next to the licence file (`device.id`). It is made from that random number and the identifier of the installation of the operating system (the Windows `MachineGuid`, `/etc/machine-id`), mixed into one hash, so that the licence file cannot simply be copied to another computer; neither of the two is sent, only the hash. It is not made from the name of the computer, the user name or the network. It cannot be traced back to a person.
- The **name** of a computer is only what the user types ("Home PC"). Empty is allowed.
- Nothing about the projects is sent: no file names, no paths, no texts, no glossary, no settings, no list of games.

What the server **stores** (D1 database at Cloudflare): the licence id, a **hash** of the key (the key itself is not stored), the e-mail address of the buyer (from the payment provider, to send the key and to help the buyer), the number of computers allowed, the device ids with the names, the version, the time the computer was added and last seen, and a **log** of events (issued, activated, checked, refused, freed, cancelled, e-mail sent). The owner can delete the e-mail and the note of a licence (and the e-mail in its log) on request.

Who else handles it: **Cloudflare** hosts the server and sees the connections (it may log IP addresses under its own terms; our code does not store them, a rate-limit rule uses the IP address only to block floods); **Resend** sends the e-mail with the key (it gets the address of the buyer and the key); the **payment provider** takes the payment and keeps the payment data (**Aside never sees card numbers**).

## 2. Machine translation (only when the user turns it on)

Off by default. It needs the user's own account at a translation service and an explicit **consent** in Settings, Machine translation ("Missing consent to send text to an external service" is the message without it). The requests go **straight from the computer of the user to the service**, not through the Aside server.

| Service | What is sent |
|---|---|
| DeepL | the line to translate (tags and variables protected), source and target language, the user's API key (in the header) |
| LibreTranslate | the same, the key if there is one |
| OpenAI-compatible (any address the user sets) | the line, the previous lines as context, the glossary terms that occur in it, the languages, the model name, the key |

The API key is kept in `config.json` in the settings folder of the user (plain text, only on that computer; it is never sent to the Aside server). What the service does with the text is governed by its own terms.

## 3. Publishing progress to a website (Pro, only when the user sets it up)

`git clone / pull / push` of a repository **chosen by the user**: the file with the progress of the translation (percentages, episodes, dates) is committed and pushed with the git of the user and the credentials of the user (Credential Manager). Nothing goes to the Aside server.

## What stays on the computer

The settings (`%APPDATA%\Aside\config.json`, no password of any account), the data of the projects (`%APPDATA%\Aside\projects\...`, backups, snapshots, history), the licence file and the device id (`licence.json`, `device.id`, `device.name` in the same folder), language packs. In the game folder Aside writes only the translation (`game/tl/<language>/`) and a small folder of shared project data.

## How to check it

- The only code that opens an Internet connection is `rust/crates/lexo-core/src/net.rs` (one function `post`), used by the machine translation (`mt.rs`) and by the licence client of the Pro pack (`pro/rust/lexo-pro/src/licence_client.rs`). The git commands are in `pro/rust/lexo-pro/src/publish.rs`.
- The user interface is local: it loads no script, font or picture from the Internet.
- Carrying a translation over from the previous version (Pro) reads two local folders and writes into the open project; it sends nothing anywhere (the matching is done in the pack on the computer).
- A copy without the Pro pack (the Free installer) contains no licence client and never contacts the licence server.
