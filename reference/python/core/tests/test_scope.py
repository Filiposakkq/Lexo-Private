"""A project only sees its own game: other games next to it, source scripts and game folders must not leak in."""
import unittest

from helpers import TempCase, dialog
from lexo.app import AppError

TRANSLATION = dialog("a1", "mc", "Hello there, friend.", "Ahoj tam, příteli.", 5)
SOURCE = 'label start:\n    mc "Hello there, friend."\n    show text "Only in this game"\n'
OTHER = 'label start:\n    show text "A text of another game"\n    mc "Another line."\n'


class ScopeTests(TempCase):
    def make_games(self):
        """workspace/GameA/game/{script.rpy, tl/czech}, workspace/GameB/game/script.rpy next to it."""
        ws = self.tmp / "workspace"
        self.a = ws / "GameA"
        self.b = ws / "GameB"
        for g, text in ((self.a, SOURCE), (self.b, OTHER)):
            (g / "game").mkdir(parents=True)
            (g / "game" / "script.rpy").write_text(text, encoding="utf-8")
        self.tl = self.a / "game" / "tl" / "czech"
        self.tl.mkdir(parents=True)
        (self.tl / "script.rpy").write_text(TRANSLATION, encoding="utf-8")

    def test_missed_texts_stay_inside_the_game_of_the_translation(self):
        self.make_games()
        app = self.new_app(open_root=False)
        app.open_project(self.tl)
        texts = {i["text"] for i in app.missed_scan("all")["items"]}
        self.assertIn("Only in this game", texts)
        self.assertNotIn("A text of another game", texts)
        self.assertNotIn("Another line.", texts)

    def test_characters_stay_inside_the_game_too(self):
        self.make_games()
        (self.a / "game" / "characters.rpy").write_text('define a = Character("Anna")\n', encoding="utf-8")
        (self.b / "game" / "characters.rpy").write_text('define b = Character("Bob")\n', encoding="utf-8")
        app = self.new_app(open_root=False)
        app.open_project(self.tl)
        self.assertEqual([p.parent.parent.name for p in app.find_characters_files()], ["GameA"])

    def test_a_translation_outside_a_game_folder_searches_nothing(self):
        elsewhere = self.tmp / "loose" / "czech"
        elsewhere.mkdir(parents=True)
        (elsewhere / "script.rpy").write_text(TRANSLATION, encoding="utf-8")
        (self.tmp / "loose" / "other.rpy").write_text(OTHER, encoding="utf-8")
        app = self.new_app(open_root=False)
        app.open_project(elsewhere)
        self.assertEqual(app.missed_scan()["items"], [])
        self.assertEqual(app.find_characters_files(), [])

    def test_source_scripts_are_not_translation_files(self):
        self.make_games()
        (self.tl / "plain_source.rpy").write_text(SOURCE, encoding="utf-8")        # dialogue but no translate block
        app = self.new_app(open_root=False)
        app.open_project(self.tl)
        self.assertEqual(sorted(app.project.files), ["script.rpy"])

    def test_choosing_the_game_folder_opens_its_only_translation(self):
        self.make_games()
        for chosen in (self.a, self.a / "game", self.a / "game" / "tl"):
            app = self.new_app(open_root=False)
            app.open_project(chosen)
            self.assertEqual(app.project.root, self.tl.resolve(), chosen)
            self.assertEqual(sorted(app.project.files), ["script.rpy"])
            self.assertTrue(any("game folder" in str(n) for n in app.notices))

    def test_several_translations_ask_for_a_choice(self):
        self.make_games()
        other = self.a / "game" / "tl" / "german"
        other.mkdir()
        (other / "script.rpy").write_text(TRANSLATION.replace("czech", "german"), encoding="utf-8")
        app = self.new_app(open_root=False)
        with self.assertRaises(AppError) as cm:
            app.open_project(self.a)
        self.assertIn("czech", str(cm.exception))
        self.assertIn("german", str(cm.exception))


if __name__ == "__main__":
    unittest.main()
