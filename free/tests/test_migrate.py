"""Copying the Czech-named data files of the BaDIK tool to the English names (migrate.py)."""
import unittest

from helpers import TempCase
from lexo import migrate
from lexo.messages import render
from lexo.store import Store


class MigrateTests(TempCase):
    def setUp(self):
        super().setUp()
        self.d = self.tmp / "data"
        self.d.mkdir()

    def old(self, name, text):
        (self.d / name).write_text("\ufeff" + text, encoding="utf-8")

    def test_every_file_is_copied_with_a_new_header_and_translated_values(self):
        self.old("prirazeni.csv", "soubor;zarazeni\nscript.rpy;Epizoda 3\nheader.rpy;Obecné\n")
        self.old("epizody.csv", "epizoda;stav\n1;Rozpracováno\n2;V korekci\n3;Přeloženo\n4;Hotovo\n5;Nezačato\n")
        self.old("korekce.csv", "soubor;zkontrolovano;prelozeno_rucne\nscript.rpy;5;\n")
        self.old("historie.csv", "datum;prelozeno;celkem;procenta;korekce;korekce_procenta\n2026-10-13 09:00;10;100;10.0;0;0.0\n")
        (self.d / "historie_textu.jsonl").write_text('{"t":"x"}\n', encoding="utf-8")
        notes = migrate.run(self.d)
        self.assertEqual(len(notes), 1)
        self.assertIn("proofread.csv", render(notes[0]))
        self.assertEqual((self.d / "assignments.csv").read_text(encoding="utf-8-sig").splitlines(),
                         ["file;group", "script.rpy;episode 3", "header.rpy;general"])
        self.assertEqual((self.d / "episode_states.csv").read_text(encoding="utf-8-sig").splitlines(),
                         ["episode;state", "1;wip", "2;proof", "3;tr", "4;done", "5;none"])
        self.assertTrue((self.d / "history.csv").read_text(encoding="utf-8-sig").startswith("date;translated;total;percent;proofread;proofread_percent"))
        self.assertEqual((self.d / "text_history.jsonl").read_text(encoding="utf-8"), '{"t":"x"}\n')

    def test_old_files_stay_untouched_and_a_second_run_does_nothing(self):
        self.old("korekce.csv", "soubor;zkontrolovano;prelozeno_rucne\nscript.rpy;5;\n")
        before = (self.d / "korekce.csv").read_bytes()
        migrate.run(self.d)
        self.assertEqual((self.d / "korekce.csv").read_bytes(), before)
        (self.d / "proofread.csv").write_text("file;proofread;translated_manual\nscript.rpy;9;\n", encoding="utf-8")
        self.assertEqual(migrate.run(self.d), [])
        self.assertIn(";9;", (self.d / "proofread.csv").read_text(encoding="utf-8"))

    def test_the_store_reads_what_the_migration_wrote(self):
        self.old("prirazeni.csv", "soubor;zarazeni\nscript.rpy;Epizoda 1\nheader.rpy;Obecné\n")
        self.old("epizody.csv", "epizoda;stav\n1;Rozpracováno\n")
        self.old("slovnik.csv", "kod;jmeno;typ;poznamka\nmc;Hlavní hrdina;speaker;\n")
        migrate.run(self.d)
        s = Store(self.d).load()
        self.assertEqual(s.assign, {"script.rpy": "ep:1", "header.rpy": "gen"})
        self.assertEqual(s.episodes, {1: "wip"})
        self.assertEqual(s.glossary["mc"]["n"], "Hlavní hrdina")

    def test_opening_a_project_with_old_data_migrates_it_and_keeps_the_layout(self):
        self.basic_project()
        data = self.root / "_progress_data"
        data.mkdir(exist_ok=True)
        (data / "prirazeni.csv").write_text("soubor;zarazeni\nscript.rpy;Epizoda 1\n", encoding="utf-8")
        app = self.new_app()
        self.assertEqual(app.store.assign.get("script.rpy"), "ep:1")
        self.assertTrue((data / "assignments.csv").is_file())
        self.assertTrue(any("new names" in render(n) for n in app.notices))
        self.assertEqual(self.new_app().store.assign.get("script.rpy"), "ep:1")      # second open: nothing to migrate

    def test_nothing_to_do_without_old_files(self):
        self.assertEqual(migrate.run(self.d), [])
        self.assertEqual(list(self.d.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
