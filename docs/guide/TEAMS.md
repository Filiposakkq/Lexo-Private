# Teams: several people on one translation

This is the full guide to working as a team. It covers what a team is, how to set one up (step by step), what everybody sees and does, what the lead can do with a Pro licence, where the data is kept, and what to do when something goes wrong. For the short version of every Pro feature see [PRO-FEATURES.md](PRO-FEATURES.md); for the design and the reasons see [TEAMWORK.md](../product/TEAMWORK.md); for the file formats see [DATA-LAYOUT.md](../technical/DATA-LAYOUT.md).

## 1. What a team is (and what it is not)

**A team is the people who share one translation folder.** There is no server, no account and no registration. You share the folder with a tool you already have (git, Dropbox, Syncthing, a network drive), and every person runs Aside on their own copy. Aside reads what the others wrote into the shared folder and shows it.

The idea that keeps it simple: **every file is written by one person only.** Each person writes their own name, their own notes, their own log of changes. Nobody edits a file somebody else owns, so the sharing tool never has to merge anything and nothing conflicts. The only file that holds common decisions (roles, who has which files) is written by the **lead** alone.

**It is a team of trust, not of rights.** Whoever has the folder can write in it; Aside does not stop anybody. Roles and assignments tell people (and Aside) who does what, they do not lock files. This is on purpose: a team of translators who share a folder trust each other already, and anything stricter would need a server.

**What it is not:** it is not live co-editing (two people typing in the same line at the same moment), not a chat, not a notification service, and not a merge tool for the `.rpy` files. People work, the sharing tool brings the changes, and Aside shows what changed. Section 12 lists what is not built yet.

## 2. Who needs a licence

| | Free | Pro licence |
|---|---|---|
| Say who you are in the project (join) | yes | yes |
| See the members, their roles and their files | yes | yes |
| Column **Assigned to** and the filter **Person** in the table of files | yes | yes |
| **News** since your last look | yes | yes |
| **Notes to the team** on lines | yes | yes |
| **Make a team** (become the lead) | no | **yes** |
| Give **roles**, give **files** to people | no | **yes** |
| **Statistics** per person | no | **yes** |

**Only the lead needs a licence.** The translators and proofreaders work in the free application; they join, see their files, write notes and read the news. One licence is enough for a whole team. The demo (no licence) cannot make or manage a team (`team.manage` is a locked feature).

If the lead's licence runs out, nothing is lost: the team, the roles and the assignments stay in the folder and everybody still sees them. The lead only cannot change them or see the statistics until the licence works again.

## 3. Before you start: share the folder

The shared folder is the **translation folder**, the one you open in Aside (`game/tl/<language>/`). It contains the `.rpy` files of the translation and a small folder `.aside/` with the shared project data. Aside keeps everything bulky or private (backups, snapshots, the history of texts, the news pointer) **outside** the folder, in the settings of the application on each computer, so it is not shared and does not grow your repository.

Pick one way to share it:

- **git** (good for programmers and for a history of changes): everybody clones the same repository and runs *pull* before work and *commit and push* after. Commit `.aside/` together with the `.rpy` files (the local data of each person is outside the folder, so there is nothing to exclude).
- **Dropbox, OneDrive, Google Drive, Syncthing**: share the folder and let the tool sync. Wait for the sync before you start and after you stop. If two people change the **same** `.rpy` file at once, these tools make a "conflicted copy"; avoid it by giving the files to different people (section 7).
- **A network drive**: works if everybody has write access; never open the same file from two computers at the same time.

Two rules make any of them safe:
1. **Give every file to one person at a time** (section 7). Most conflicts then never happen.
2. **Sync before you start and after you stop.** Aside reads the folder when you press *Refresh files* or open the project.

A reminder for the game build: hidden folders may reach a built game. The shared part (`.aside/`) holds no secrets, only names you typed and notes you wrote. If you do not want it in a release, see the rule shown in [DATA-LAYOUT.md](../technical/DATA-LAYOUT.md); the release builder (Pro) never packs `.aside/`.

## 4. Setting up the team, step by step

### The lead (a person with a Pro licence)

1. Open the translation folder in Aside. Everybody must open the **same** folder (the one that is shared).
2. Go to the **Files** page and press **Team…**.
3. Type **your name** (the name the others will see) and press **Join**. Aside writes it to `.aside/team/members/<your id>.json`. (Your id is made once from your user name and kept in the application settings; it is how Aside tells people apart, so two people on one computer account would look like one person.)
4. Press **Make this a team (I am the lead)**. Aside writes `.aside/team/lead.json`. The project now has a team and you are its lead.
5. Share the folder (section 3). The members will see the team as soon as the change reaches them.

### Everybody else (no licence needed)

1. Get the shared folder (clone it, accept the share) and open it in Aside.
2. **Files › Team…**. If you have not joined, type your name and press **Join**.
3. You see the members, their roles and the files that are yours. Your name appears in the lead's list as soon as your file reaches the lead.

### Giving roles and files (the lead)

- **Roles:** in the Team window the lead picks for every member **translator** or **proofreader** and presses **Save roles**. (The lead is the lead; the lead has no other role.)
- **Files:** in the table of files, the column **Assigned to** has a list for every file. Pick the person; pick **–** to take the file back. One person per file in the table; the file format allows up to five people on a file if you ever edit it by hand (section 11).

Everybody sees roles and assignments after the next sync.

### One team per project, not per computer

A team lives **in the translation folder of one game** (`.aside/team/`). If you work on several games, each has its own team (or none): switching to another game shows *its* team, and a game without a team shows "This project has no team yet". Your **id** is the same in all your games (it belongs to this computer account), and the **name** you typed last is offered again when you join another project, but you join each project separately.

## 5. Leaving, removing, handing over, ending

Everything is in the **Team…** window.

| What | Who | How | What happens |
|---|---|---|---|
| **Leave the team** | any member (not the lead) | *Leave the team* (asks "Leave this team?") | Your own file in `members/` is deleted; you can join again later. Your notes and your activity log stay. |
| **Remove a person** | the lead (Pro) | *Remove* next to the name (asks first) | The person is listed as **removed** in `lead.json`, loses their role and their files, and **cannot join again by themselves** (they see "The lead removed you…"). Their notes and logs stay. |
| **Take a person back** | the lead (Pro) | *Removed: … Take back* | The person can join again with their name. |
| **Hand the team over** | the lead (Pro) | *Make lead* next to the new lead's name (asks first) | The other person becomes the lead and you become a translator. The new lead needs a Pro licence to manage the team. |
| **End the team** | the lead (Pro) | *End the team* at the bottom | `lead.json` is deleted. The people's files, the notes and the activity logs stay in the folder (they are not the lead's to delete); a new team can be made later. |

The lead cannot simply leave: hand the team over or end it first. A removed person keeps their copy of the folder and their notes; only the team stops counting them. Because the file of a removed person is not deleted (a sharing tool could bring it back), removal is a list in the lead's file that every copy respects.

## 6. The roles

| Role | What the person does | What Aside does with it |
|---|---|---|
| **lead** | Makes the team, gives roles and files, sees the statistics. | Needs a Pro licence for these. The lead can also translate and proofread like everybody else. |
| **translator** | Translates the files they have. | Shown in the list; nothing is locked. |
| **proofreader** | Reads translated lines and marks them proofread or "to fix". | Shown in the list; nothing is locked. |

The roles are information for people and for the statistics (they label the rows). They do not change what a person can click. A member who has no role set is shown as a translator.

## 7. Giving files to people (assignments)

A file given to a person is the best protection against conflicts: two people do not edit one file. In the table of files:

- the column **Assigned to** shows the person (or **–**);
- the filter **Person** next to the other filters has **Person: all**, **My files** (the files that are yours), **Nobody** (files nobody has) and one entry per member;
- the lead sees a list in each cell and changes it there; the others see text.

This is separate from the **Assignment** column (the group of a file: general, episode N). Groups say *which part of the game* a file belongs to; **Assigned to** says *who works on it*.

Giving a file does not lock it. If somebody else changes it anyway, Aside does not stop them. Use the news (section 9) and talk.

A file given to a person who is not a member (for example because the member's file did not arrive yet) is cleaned away when the lead saves; the pack drops what refers to unknown people.

## 8. Everyday work

A good day for a translator:

1. **Sync** the folder (pull, or wait for the sharing tool).
2. Open Aside, go to **Files**, set the filter to **My files**.
3. Press **Team…** and read the **news**: what others did since your last look, and new notes to the team. Press **Mark as seen** when you have read it.
4. Translate your files in **Lines**. When a line needs a word to the team, write a **note** (section 10).
5. **Sync** again when you stop, so the others get your work.

A proofreader does the same, but works on translated lines: marks them proofread or "to fix" (with a note for the translator), approves what is fine. The marks are written under their name in the shared log.

The lead also opens **Team…** to see the **statistics** (section 11) and to give files to people.

## 9. News: what changed since your last look

In the Team window (and as a number you can check there) Aside lists what **other people** did since **your last look**:

- **changed N lines** in a file (texts written in the app by the others),
- **approved N lines**, **marked N lines** (proofreading marks and "to fix" marks),
- **moved N files to a group** (their assignment to a group changed),
- and the number of **new notes to the team**.

The lines are grouped by person and file, the newest first, at most 40. Your own work is never news for you.

**Mark as seen** moves the point of your last look to now. This point is kept **only on your computer** (in the local data of the project), it is not shared. **The first time** you open a team, nothing is news (a new member does not get the whole history as news); from then on it counts from that moment.

News depends on the sync: you see what has reached your folder. If you want a quiet start of the day, sync first.

## 10. Notes to the team on lines

When a line needs a decision or a question ("is this too informal?", "check the joke"), write a note on the line instead of a message somewhere else.

- In **Lines**, open a line; in its detail there is a tab **Team** (it appears when the project has a team).
- Write the text and press **Add the note**. Everybody in the team sees it after the next sync, with your name and the time.
- Anybody can press **Mark as dealt with** on a note; it is then struck through with a chip "dealt with". (This is written as a line in your own file, so one person's "dealt with" never conflicts with another's.)
- Notes belong to a line by its **block and original text** (the same identity the history of texts uses). If the original text of the line changes in a new game version, the old notes no longer show on it. They stay in the files.
- A note is at most 2000 characters. Notes are never deleted by Aside.

## 11. Statistics (the lead, Pro)

In **Team…**, below the members, the lead sees **Statistics** with a period (last 7 days, last 30 days, all time):

| Column | Meaning |
|---|---|
| **Files** | How many files are given to the person. |
| **Translated** | How much of **their files** is translated (lines translated of all lines), as a percentage. This is the current state, not limited by the period. |
| **Proofread** | How much of their files is proofread (the same way). |
| **Changed lines** | How many lines the person changed **in the application** in the period (typing, fixes, replacements, machine translation, carrying over, filling from similar translations). Undo and restoring a backup are not counted. |
| **Approved** | How many lines the person approved in the period (from the shared log). |
| **Marked** | How many proofreading marks (proofread / "to fix") the person wrote in the period. |
| **Last activity** | The time of the person's last change, approval or mark (in any period). |

Under the table: the files **nobody has** (how many and how far they are translated).

What to know before you rely on the numbers:

- **"Changed lines" counts only what was written in the application, from the day the team was made.** A text edited in an external editor and noticed at a refresh is not counted (it is in the history of texts of that computer, but not in the team log). A line changed three times counts three times.
- **A file that belongs to two people counts for both.** Their progress figures overlap.
- **Progress is of the files given to the person**, not of what the person wrote. If Ann translates a file that is given to Bob, the file's progress is Bob's.
- **Approved and marked come from the shared log** the whole project uses, with the name of whoever wrote the event. A line approved twice counts twice.
- The clock is the clock of each computer. Two people with clocks that differ by minutes can swap who "was last" in a conflict; this only affects the last-activity column.
- **Only the lead sees the statistics**, and only with a working Pro licence. Statistics are for coordination, not for judging people: a translator who writes few lines may be fixing hard ones.

## 12. Where the data is, and what is private

Everything shared is in `.aside/team/` (and the existing shared project data in `.aside/`). **No other computer, no server and no company sees it** unless you share the folder with them. Aside itself sends nothing for the team over the network.

```
<translation folder>/.aside/team/
  members/<author>.json      each person's own name: {"author": id, "name": "..."}         written by that person
  lead.json                  {"v":1, "lead": id, "roles": {id: role}, "assign": {file: [id]}, "removed": [id]}  written by the lead only
  activity/<author>.jsonl    what the person changed: {t, a, file, n, via}, one line per file and edit   (only in a team)
  notes/<author>.jsonl       notes {id, t, a, file, k, text} and resolutions {resolve, t, a}
```

Local, **not shared**: `team-seen.json` (in the application's data for the project), the time of your last look at the news; and `team-name.txt` (in the settings folder of the application), the last name you typed, offered again when you join another project.

What the files contain about a person: **the id** (made from the user name of the computer account and four random characters) and **the name the person typed**. Notes contain the text the person wrote. Nothing else. If the project is public (for example on GitHub), these names are public too; type a nickname if you prefer.

The formats are plain text, one file per author, so a git history shows who did what and a conflict is practically impossible. Details and the rules for the `.aside/` folder are in [DATA-LAYOUT.md](../technical/DATA-LAYOUT.md).

## 13. Limits, and what is not built

Limits (the application keeps them even if a file is edited by hand):

- at most **100 members**, at most **5 people on one file**, a name at most **40 characters**, a note at most **2000 characters**;
- the news lists at most **40 lines**;
- the statistics read the whole shared log each time (fine for projects of tens of thousands of lines).

**Not built (yet):**
- merging the `.rpy` files of two people **by block** (today a file is given to one person; see section 3 for the sharing tools' conflicts);
- buttons that run git for you ("get changes", "send changes");
- notifications (e-mail, pop-ups) and live co-editing;
- statistics of who **translated** a line in an external editor.

## 14. Questions and problems

**A member does not see the team.** The folder has not reached them yet (sync), or they opened a different folder. The team exists when `.aside/team/lead.json` is in the folder they opened.

**A member joined but is not in the lead's list.** Their file `.aside/team/members/<id>.json` has not reached the lead (sync). Nothing is wrong with the application.

**The "Team…" window says there is no team and I am the lead.** You are looking at a copy that was made before the team was. Sync, or check that `.aside/team/lead.json` exists.

**Someone made a team by mistake / two leads.** Only one `lead.json` exists in a folder. If two people made one at the same time, the sharing tool may keep one copy and drop the other (or create a "conflicted copy"); keep the one you want. The lead of a team is the `lead` field of that file.

**I need to remove a member.** The lead presses *Remove* next to the name in **Team…** (section 5); the person's files become free to give to someone else.

**The lead changes.** The lead presses *Make lead* next to the new lead's name (section 5). **If the lead's computer is lost** and nobody can do that, open `.aside/team/lead.json`, change the `lead` field to the **id** of the new lead (the name of that person's file in `members/` and the `author` value inside it), and share it. The new lead needs a Pro licence to manage the team.

**A member changed their name.** Press **Join** again and type the new name; it replaces the old one.

**A member reinstalled and now appears twice.** The application made a new id for the new installation. The lead removes the old entry (*Remove*) and gives the files to the new one; or the person deletes their old file in `members/` by hand.

**The statistics show 0 changed lines for someone who clearly worked.** Either the work was done outside the application (an external editor), before the team was made, or the person's activity file has not reached the lead yet.

**Do the members need the same version of Aside?** Use the same version when you can. Files are written in a versioned format and unknown lines are skipped, so a different version may miss a feature but does not break the folder.

**Is anything sent to Aside's servers?** No. A team uses only your shared folder. (A licence is checked with the licence server every 14 days as for any Pro user; that does not include the team.)
