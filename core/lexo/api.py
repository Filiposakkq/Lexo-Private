"""The interface of Core: every operation the application offers, as a JSON call.

    from lexo import api
    result = api.call(app, "entries", {"status": "todo", "limit": 10})        # a dict that can be written as JSON

This table is THE interface between a client and Core (roadmap step 5). The browser UI reaches it through the local server of Free (one GET or POST per
operation, the parameters in the query string or in the JSON body, the answer as JSON), a script calls `api.call` directly, and the Rust port of Core has to
offer the same operations with the same JSON. Nothing in this file knows about HTTP.

An operation has
    name         "entries", "entry/save" ... (the address after /api/ in the server)
    method       "GET" (a read: parameters in the query string) or "POST" (parameters in the JSON body); only a convention of the transport
    effect       "read"      changes nothing
                 "write"     changes the project, the settings or the data folder of the project
                 "external"  also reaches outside (starts a program, opens a window or a page, uses the network)
    params       what it takes: name, type (str, int, flag, list, any), default, required
    doc          one line
    run(app, a)  the work; `a` has the parameters, coerced by their types, as attributes
    example      parameters for the contract tests and the generated docs (None: the test cannot run it)

The answer is a dict with `"ok": true`; an error is raised as `AppError` (a message for the user), `KeyError` (a required parameter is missing), `WriteFailed`,
`ProjectBusy`; the transport turns them into `{"ok": false, "error": ...}`. Messages in answers are `Message` objects; `messages.wire` makes them JSON.
Binary answers (`kind == "binary"`: a font, a picture) return `(bytes, mime)`.

`describe()` gives the whole table as plain data (`docs/API.md` and `core/contract/api.json` are generated from it, and tests keep them in step).
"""
from . import __version__, lang
from .app_base import AppError
from .messages import msg

TYPES = ("str", "int", "flag", "list", "any")


class UnknownOperation(KeyError):
    pass


class Param:
    __slots__ = ("name", "type", "default", "required")

    def __init__(self, name, type="str", default=None, required=False):
        assert type in TYPES, type
        self.name, self.type, self.required = name, type, required
        self.default = default if default is not None else {"str": "", "int": 0, "flag": False, "list": [], "any": None}[type]

    def coerce(self, params):
        if self.name not in params:
            if self.required:
                raise KeyError(self.name)
            return list(self.default) if self.type == "list" else self.default
        v = params[self.name]
        if v is None or (v == "" and self.type in ("int", "list", "any")):        # an empty value means "not given"
            return list(self.default) if self.type == "list" else self.default
        if self.type == "str":
            return str(v)
        if self.type == "int":
            return int(v)
        if self.type == "flag":
            return v is True or v == 1 or str(v).lower() in ("1", "true")
        if self.type == "list":
            return list(v or [])
        return v

    def describe(self):
        return {"name": self.name, "type": self.type, "default": self.default, "required": self.required}


class Args:
    """The parameters of one call, as attributes (`a.file`); `a.all` is the whole dict as it came."""

    def __init__(self, values, raw):
        self.__dict__.update(values)
        self.all = raw


class Operation:
    def __init__(self, name, method, effect, params, doc, run, needs_project=True, kind="json", example=None, path=None):
        self.name, self.method, self.effect, self.params, self.doc, self.run = name, method, effect, params, doc, run
        self.needs_project, self.kind, self.example = needs_project, kind, example
        self.path = path or name                 # the address after /api/ (two operations may share one path, one GET and one POST)

    def bind(self, raw):
        raw = raw if isinstance(raw, dict) else {}
        return Args({p.name: p.coerce(raw) for p in self.params}, raw)

    def describe(self):
        return {"name": self.name, "path": self.path, "method": self.method, "effect": self.effect, "kind": self.kind, "needs_project": self.needs_project, "doc": self.doc,
                "params": [p.describe() for p in self.params], "example": self.example}


OPS = {}
ROUTES = {}                                    # (method, path) -> Operation, for a transport that works with addresses


def _op(name, method, effect, doc, *params, needs_project=True, kind="json", example=None, path=None):
    def deco(fn):
        op = OPS[name] = Operation(name, method, effect, list(params), doc, fn, needs_project, kind, example, path)
        ROUTES[(method, op.path)] = op
        return fn
    return deco


def S(name, default="", required=False):
    return Param(name, "str", default, required)


def I(name, default=0, required=False):          # noqa: E741
    return Param(name, "int", default, required)


def F(name, default=False):
    return Param(name, "flag", default)


def L(name):
    return Param(name, "list")


def A(name, default=None):
    return Param(name, "any", default)


def call(app, name, params=None):
    """Run the operation `name` with `params`. Returns the answer as a dict (a bare result is wrapped as {"ok": true, "result": ...})."""
    op = OPS.get(name)
    if op is None:
        raise UnknownOperation(name)
    if op.needs_project and not app.ready:
        raise AppError(msg("No project is open."))
    a = op.bind(params)
    with app.lock:
        result = op.run(app, a)
    if op.kind == "binary" or isinstance(result, dict):
        return result
    return {"ok": True, "result": result}


def describe():
    """The whole interface as plain data, sorted by name."""
    return [OPS[n].describe() for n in sorted(OPS)]


# =====================================================================================================================================================
# reads (GET)
# =====================================================================================================================================================
def _ok(d):
    return {"ok": True, **d}


@_op("state", "GET", "read", "What the client needs at start: is a project open, notices, config error, UI preferences, version.", needs_project=False, example={})
def _state(app, a):
    return {"ok": True, "ready": app.ready, "notices": list(app.notices), "config_error": app.config.load_error, "prefs": app.ui_prefs(), "version": __version__,
            "project_dir": app.config.get("project_dir", "")}


@_op("ui-langs", "GET", "read", "The interface languages (language packs that have UI texts).", needs_project=False, example={})
def _ui_langs(app, a):
    return {"ok": True, "langs": lang.ui_languages()}


@_op("ui-lang", "GET", "read", "The interface texts of one language: {code, name, plural, strings}; English is an empty table.", S("code"), needs_project=False, example={"code": "cs"})
def _ui_lang(app, a):
    try:
        res = lang.ui_language(a.code)
    except lang.PackError as e:
        raise AppError(str(e)) from e
    res["strings"] = {**res["strings"], **app.ext.ui_strings(res["code"])}      # plus the texts of the Pro pack, if there is one
    return {"ok": True, **res}


@_op("overview", "GET", "read", "Totals, groups, estimates and the data of the Overview tab.", example={})
def _overview(app, a):
    return _ok(app.overview())


@_op("files", "GET", "read", "The translation files with their counts and states.", example={})
def _files(app, a):
    return {"ok": True, "files": app.files_payload()}


@_op("entries", "GET", "read", "A page of translation lines, filtered and sorted on the server.", S("file"), S("status", "all"), S("kind"), S("q"), S("issues"), S("sort", "line"),
     I("offset"), I("limit", 200), I("around"), S("unit"), F("word"), example={"status": "all", "limit": 5})
def _entries(app, a):
    return _ok(app.entries(file=a.file, status=a.status, kind=a.kind, q=a.q, issues=a.issues, sort=a.sort, offset=max(0, a.offset), limit=a.limit,
                           around=a.around, unit=a.unit, word=a.word))


@_op("entry", "GET", "read", "The detail of one line (texts, state, findings, proofreading).", S("file", required=True), I("line", required=True), example={"file": "script.rpy", "line": 5})
def _entry(app, a):
    return _ok(app.entry_detail(a.file, a.line))


@_op("validation", "GET", "read", "The findings of the translation check, one item per line.", S("severity"), S("category"), S("file"), S("q"), I("offset"), I("limit", 200),
     S("ignored", "hide"), S("group"), F("word"), example={"group": "line", "limit": 5})
def _validation(app, a):
    return _ok(app.validation_list(severity=a.severity, category=a.category, file=a.file, q=a.q, offset=max(0, a.offset), limit=a.limit, ignored=a.ignored,
                                   group=a.group, word=a.word))


@_op("ignore/list", "GET", "read", "The findings the translator told the app to ignore.", example={})
def _ignore_list(app, a):
    return _ok(app.ignored_list())


@_op("settings", "GET", "read", "The settings (validation, text box, machine translation, publishing ...).", needs_project=True, example={})
def _settings_get(app, a):
    return _ok(app.settings())


@_op("fix/last", "GET", "read", "What the last bulk fix changed (for undo).", example={})
def _fix_last(app, a):
    return _ok(app.fix_last())


@_op("game/fontmetrics", "GET", "read", "Advance widths of a font of the game (key = file name).", S("k"), example=None)
def _fontmetrics(app, a):
    return _ok(app.game_font_metrics(a.k))


@_op("game/characters", "GET", "read", "The characters defined in the game scripts: code, name, colour.", example={})
def _characters(app, a):
    return _ok(app.game_characters())


@_op("entry/next", "GET", "read", "The next line of a kind (todo, review, fix, over) after this one, wrapping round the file.", S("file"), I("line"), S("kind"), example={"file": "script.rpy", "line": 5, "kind": "todo"})
def _entry_next(app, a):
    return _ok(app.entry_next(a.file, a.line, a.kind))


@_op("scenes", "GET", "read", "The scenes of a file (runs of lines after the same `scene` statement) with progress.", S("file"), I("line"), example={"file": "script.rpy", "line": 5})
def _scenes(app, a):
    return _ok(app.scene_list(a.file, a.line))


@_op("game/scene", "GET", "read", "The picture(s) behind a line and the text box metrics for the scene preview.", S("file"), I("line"), example={"file": "script.rpy", "line": 5})
def _game_scene(app, a):
    return _ok(app.scene_info(a.file, a.line))


@_op("entry/menu", "GET", "read", "The menu a line belongs to: question, choices with translations, what follows each.", S("file"), I("line"), example={"file": "script.rpy", "line": 23})
def _entry_menu(app, a):
    return _ok(app.entry_menu(a.file, a.line))


@_op("entry/scene", "GET", "read", "The lines around one line (original and translation).", S("file"), I("line"), I("radius", 6), example={"file": "script.rpy", "line": 5, "radius": 3})
def _entry_scene(app, a):
    return _ok(app.entry_scene(a.file, a.line, a.radius))


@_op("snapshots", "GET", "read", "The snapshots (zips of the .rpy files) of the project.", example={})
def _snapshots(app, a):
    return _ok(app.snapshot_list())


@_op("snapshots/files", "GET", "read", "The files inside one snapshot.", S("name"), example=None)
def _snapshots_files(app, a):
    return _ok(app.snapshot_files(a.name))


@_op("lint/status", "GET", "read", "State and result of the Ren'Py lint run.", example={})
def _lint_status(app, a):
    return _ok(app.lint_status())


@_op("entry/history", "GET", "read", "The versions of a line's text over time.", S("file"), I("line"), example={"file": "script.rpy", "line": 5})
def _entry_history(app, a):
    return _ok(app.entry_history(a.file, a.line))


@_op("entry/memory", "GET", "read", "Other translations of the same original (translation memory).", S("file"), I("line"), example={"file": "script.rpy", "line": 5})
def _entry_memory(app, a):
    return _ok(app.entry_memory(a.file, a.line))


@_op("consistency", "GET", "read", "Originals translated in several different ways.", F("loose"), F("by_speaker"), example={})
def _consistency(app, a):
    return _ok(app.consistency(a.loose, a.by_speaker))


@_op("entry/context", "GET", "read", "The original game script around the line this translation comes from.", S("file"), I("line"), example={"file": "script.rpy", "line": 5})
def _entry_context(app, a):
    return _ok(app.entry_context(a.file, a.line))


@_op("glossary/occurrences", "GET", "read", "Where a speaker code (or a missing name) occurs.", S("code"), F("missing"), example={"code": "friend"})
def _glossary_occ(app, a):
    return _ok(app.glossary_occurrences(a.code, a.missing))


@_op("glossary", "GET", "read", "The glossary: speakers and terms with counts and examples.", example={})
def _glossary(app, a):
    return _ok(app.glossary_payload())


@_op("proof/next", "GET", "read", "The next line still to proofread after a line number.", S("file"), I("after"), example={"file": "script.rpy", "after": 0})
def _proof_next(app, a):
    return _ok(app.proof_next(a.file, a.after))


@_op("export/preview", "GET", "read", "The public progress data and its checks (writes only the local progress.json).", example={})
def _export_preview(app, a):
    return _ok(app.export_preview())


@_op("missed", "GET", "read", "Texts the Ren'Py generator missed, found in the game scripts.", S("level", "likely"), example={})
def _missed(app, a):
    return _ok(app.missed_scan(a.level))


@_op("missed/functions", "GET", "read", "Functions of the game that are called again and again with a text.", example={})
def _missed_functions(app, a):
    return _ok(app.missed_functions())


@_op("groups/suggest", "GET", "read", "The groups the project has now and the groups its file names suggest.", example={})
def _groups_suggest(app, a):
    return _ok(app.groups_suggest())


@_op("game/font", "GET", "read", "A font of the game by file name (never a path): (bytes, mime).", S("k"), kind="binary", example=None)
def _game_font(app, a):
    return app.game_font(a.k)


@_op("game/image", "GET", "read", "A picture of the game by the key an answer gave (never a path): (bytes, mime).", S("g"), S("k"), kind="binary", example=None)
def _game_image(app, a):
    return app.game_image(a.k)


# =====================================================================================================================================================
# calls with a body (POST): previews and writes
# =====================================================================================================================================================
@_op("project/set", "POST", "write", "Open a translation folder (and remember it).", S("path"), F("force"), needs_project=False, example=None)
def _project_set(app, a):
    app.open_project(a.path.strip().strip('"'), force=a.force)
    app.maybe_auto_snapshot()
    return {"ok": True}


@_op("refresh", "POST", "write", "Read the files again (merging what changed on disk).", F("force"), F("dirty"), F("discard"), example={})
def _refresh(app, a):
    return _ok(app.refresh(force=a.force, client_dirty=a.dirty, discard=a.discard))


@_op("file/save", "POST", "write", "Save the counts of a file (the manual correction columns).", S("name", required=True), S("c"), S("t"), example={"name": "script.rpy", "c": "", "t": ""})
def _file_save(app, a):
    return app.save_file(a.name, a.c, a.t)


@_op("assign", "POST", "write", "Assign a file to a group.", S("name", required=True), S("v"), example={"name": "script.rpy", "v": ""})
def _assign(app, a):
    return app.set_assign(a.name, a.v)


@_op("assign/suggest", "POST", "read", "Suggested group for files (a Pro feature: the Pro pack suggests, the app shows the groups to choose from).", L("names"), example=None)
def _assign_suggest(app, a):
    return _ok(app.assign_suggest(a.names))


@_op("assign/apply", "POST", "write", "Apply assignments.", L("items"), example={"items": []})
def _assign_apply(app, a):
    return app.assign_apply(a.items)


@_op("features", "POST", "read", "Which gated (Pro) features are available, with which limit, and which Pro pack is installed.", example={})
def _features(app, a):
    return app.features_status()


@_op("episode", "POST", "write", "Set the state of a group (episode).", I("ep", required=True), S("state", required=True), example=None)
def _episode(app, a):
    return app.set_episode_state(a.ep, a.state)


@_op("approve/preview", "POST", "read", "Preview approving identical lines.", S("file", required=True), I("line", required=True), S("scope", "project"), Param("same_speaker", "flag", True),
     example={"file": "script.rpy", "line": 11})
def _approve_preview(app, a):
    return _ok(app.approval_preview(a.file, a.line, a.scope, a.same_speaker))


@_op("approve/apply", "POST", "write", "Approve (or un-approve) lines.", L("selections"), Param("on", "flag", True), example={"selections": [], "on": True})
def _approve_apply(app, a):
    return app.approval_apply(a.selections, a.on)


@_op("approve/rules", "POST", "read", "Preview approving by rules (names, interjections).", A("rules"), L("files"), example={"rules": [], "files": []})
def _approve_rules(app, a):
    return _ok(app.rule_preview(a.rules or [], a.files))


@_op("glossary/set", "POST", "write", "Set glossary items.", L("items"), example={"items": [{"code": "anna", "name": "Anna", "type": "speaker"}]})
def _glossary_set(app, a):
    return app.glossary_set(a.items)


@_op("glossary/import-preview", "POST", "read", "Preview importing character names from a game file.", S("path"), example=None)
def _glossary_import_preview(app, a):
    return _ok(app.glossary_import_preview(a.path))


@_op("glossary/import-apply", "POST", "write", "Import character names.", L("items"), example=None)
def _glossary_import_apply(app, a):
    return app.glossary_import_apply(a.items)


@_op("glossary/delete", "POST", "write", "Delete glossary items.", L("codes"), example={"codes": ["nobody"]})
def _glossary_delete(app, a):
    return app.glossary_delete(a.codes)


@_op("proof/set", "POST", "write", "Mark lines as proofread / to fix, with a note.", L("items"), S("state", "ok"), A("note"), example={"items": [{"file": "script.rpy", "line": 5}], "state": "ok", "note": ""})
def _proof_set(app, a):
    return app.proof_set(a.items, a.state, a.note)


@_op("proof/upto", "POST", "write", "Mark everything up to a line as proofread.", S("file", required=True), I("line", required=True), S("state", "ok"), example={"file": "script.rpy", "line": 5, "state": "ok"})
def _proof_upto(app, a):
    return app.proof_upto(a.file, a.line, a.state)


@_op("proof/clear-file", "POST", "write", "Clear the proofreading marks of a file.", S("file", required=True), example={"file": "script.rpy"})
def _proof_clear_file(app, a):
    return app.proof_clear_file(a.file)


@_op("fix/preview", "POST", "read", "Preview the bulk fixes (speaker, spaces, typos ...): a Pro feature, the Pro pack plans them.", L("items"), S("file"), L("kinds"), example=None)
def _fix_preview(app, a):
    return _ok(app.fix_preview(a.items, a.file, a.kinds))


@_op("fix/apply", "POST", "write", "Apply the chosen fixes.", L("items"), S("file"), L("kinds"), A("selections"), example=None)
def _fix_apply(app, a):
    sel = a.all.get("selections")
    return app.fix_apply(a.items, a.file, a.kinds, list(sel) if sel is not None else None)


@_op("fix/undo", "POST", "write", "Undo the last bulk fix.", example=None)
def _fix_undo(app, a):
    return app.fix_undo()


@_op("replace/preview", "POST", "read", "Preview a find and replace over the translations.", A("find", ""), A("replace", ""), S("mode", "plain"), F("case"), L("files"), Param("protect", "flag", True),
     example={"find": "Hello", "replace": "Hi", "mode": "plain"})
def _replace_preview(app, a):
    return _ok(app.replace_preview(a.find, a.replace, a.mode, a.case, a.files, a.protect))


@_op("replace/apply", "POST", "write", "Apply a find and replace to the chosen places.", L("selections"), A("find", ""), A("replace", ""), S("mode", "plain"), F("case"), L("files"),
     Param("protect", "flag", True), example={"selections": [{"file": "script.rpy", "line": 5}], "find": "Hi", "replace": "Hey", "mode": "plain"})
def _replace_apply(app, a):
    return app.replace_apply(a.selections, a.find, a.replace, a.mode, a.case, a.files, a.protect)


@_op("entry/mt", "POST", "external", "A machine translation suggestion for one line (uses the network).", S("file", required=True), I("line", required=True), example=None)
def _entry_mt(app, a):
    return _ok(app.mt_suggest(a.file, a.line))


@_op("mt/targets", "POST", "read", "The lines a batch machine translation would take (a Pro feature: the Pro pack cuts the list to its limit).", S("file"), A("lines"), Param("only_todo", "flag", True), example=None)
def _mt_targets(app, a):
    return _ok(app.mt_targets(a.file, a.lines or None, a.only_todo))


@_op("mt/batch", "POST", "external", "Machine translate many lines (a Pro feature; the pack translates, uses the network).", A("items"), example=None)
def _mt_batch(app, a):
    return _ok(app.mt_batch(a.items or []))


@_op("mt/apply", "POST", "write", "Write the chosen machine translations.", A("selections"), example={"selections": [{"file": "script.rpy", "line": 11, "text": "Kavu?"}]})
def _mt_apply(app, a):
    return app.mt_apply(a.selections or [])


@_op("missed/add", "POST", "write", "Add texts the generator missed to the translation files.", A("items"), F("into_files"), example={"items": [{"text": "A text the generator missed", "file": "", "line": 0}]})
def _missed_add(app, a):
    return _ok(app.missed_add(a.items, a.into_files))


@_op("missed/settings", "POST", "write", "Set the project's own functions that show text.", A("functions"), example={"functions": ["notify"]})
def _missed_settings(app, a):
    return _ok(app.missed_settings(a.functions))


@_op("missed/ignore", "POST", "write", "Leave texts out of the missed-texts list for good.", A("texts"), F("clear"), example={"texts": ["not a text of the game"]})
def _missed_ignore(app, a):
    return _ok(app.missed_ignore(a.texts, a.clear))


@_op("groups/apply", "POST", "write", "Define the groups of the project (written to lexo.json).", A("label"), A("groups"), S("series"), A("folders"), example={"label": "Episode", "groups": [1, 2]})
def _groups_apply(app, a):
    return _ok(app.groups_apply(a.label, a.groups, a.series, a.folders))


@_op("snapshots/create", "POST", "write", "Make a snapshot of the .rpy files now.", example={})
def _snapshots_create(app, a):
    return app.snapshot_create()


@_op("snapshots/restore", "POST", "write", "Restore files from a snapshot.", S("name"), A("files"), example=None)
def _snapshots_restore(app, a):
    return app.snapshot_restore(a.name, a.files)


@_op("snapshots/delete", "POST", "write", "Delete a snapshot.", S("name"), example=None)
def _snapshots_delete(app, a):
    return app.snapshot_delete(a.name)


@_op("lint/start", "POST", "external", "Start the Ren'Py lint of the whole game (needs the SDK).", example=None)
def _lint_start(app, a):
    return app.lint_start()


@_op("consistency/unify", "POST", "write", "Make all translations of one original the same.", S("key"), S("target"), A("lines"), F("loose"), F("by_speaker"), example=None)
def _consistency_unify(app, a):
    return app.consistency_unify(a.key, a.target, a.lines, a.loose, a.by_speaker)


@_op("ignore/set", "POST", "write", "Ignore (or stop ignoring) findings on lines.", L("items"), Param("on", "flag", True), S("note"), example={"items": [{"file": "script.rpy", "line": 5, "code": "typo-suspect"}], "on": True, "note": "example"})
def _ignore_set(app, a):
    return app.ignore_set(a.items, a.on, a.note)


@_op("entry/check", "POST", "read", "The check of a text the translator is typing (nothing is written).", S("file", required=True), I("line", required=True), S("text"),
     example={"file": "script.rpy", "line": 5, "text": "Ahoj."})
def _entry_check(app, a):
    return _ok(app.entry_check(a.file, a.line, a.text))


@_op("entry/save", "POST", "write", "Write a new translation of one line into the .rpy file (backup, atomic, undoable).", S("file", required=True), I("line", required=True), S("text"),
     example={"file": "script.rpy", "line": 5, "text": "Hi, world."})
def _entry_save(app, a):
    return app.entry_save(a.file, a.line, a.text)


@_op("undo", "POST", "write", "Undo the last change (approval, proofreading, glossary).", example={})
def _undo(app, a):
    return app.undo()


@_op("open-editor", "POST", "external", "Open a file at a line in the editor.", S("file", required=True), I("line", 1), example=None)
def _open_editor(app, a):
    return app.open_editor(a.file, a.line)


@_op("open-url", "POST", "external", "Open one of the known addresses in the browser (by id).", S("id"), needs_project=False, example=None)
def _open_url(app, a):
    return app.open_link(a.id)


@_op("settings/update", "POST", "write", "Change settings (the body is the patch).", needs_project=False, example={"web": {"title": "Example"}}, path="settings")
def _settings_update(app, a):
    return _ok(app.update_settings(a.all))


@_op("prefs", "POST", "write", "Remember UI preferences (the body is the object of preferences).", needs_project=False, example={"density": "compact"})
def _prefs(app, a):
    return app.set_ui_prefs(a.all)


@_op("publish/plan", "POST", "read", "Preview publishing the progress data (a Pro feature).", example=None)
def _publish_plan(app, a):
    return _ok(app.publish_plan())


@_op("publish/confirm", "POST", "external", "Publish the progress data after the typed confirmation (a Pro feature).", S("token"), S("branch"), example=None)
def _publish_confirm(app, a):
    return app.publish_confirm(a.token, a.branch)
