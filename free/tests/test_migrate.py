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
        notes = migrate.run(self.d)
        self.assertEqual(len(notes), 1)
        self.assertIn("proofread.csv", render(notes[0]))
        self.assertEqual((self.d / "assignments.csv").read_text(encoding="utf-8-sig").splitlines(),
                         ["file;group", "script.rpy;episode 3", "header.rpy;general"])
        self.assertEqual((self.d / "episode_states.csv").read_text(encoding="utf-8-sig").splitlines(),
                         ["episode;state", "1;wip", "2;proof", "3;tr", "4;done", "5;none"])
        self.assertTrue((self.d / "history.csv").read_text(encoding="utf-8-sig").startswith("date;translated;total;percent;proofread;proofread_percent"))

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

    def test_opening_a_project_with_the_old_folder_copies_it_once(self):
        self.basic_project()
        old = self.root / "_progress_data"
        (old / "snapshots").mkdir(parents=True, exist_ok=True)
        (old / "snapshots" / "snap-20260101-000000-manual.zip").write_bytes(b"zip")
        (old / "backups").mkdir(exist_ok=True)
        (old / "prirazeni.csv").write_text("soubor;zarazeni\nscript.rpy;Epizoda 1\n", encoding="utf-8")
        (old / "project.json").write_text('{"version": 1, "sections": [{"n": 1, "groups": [1]}]}', encoding="utf-8")
        (old / "historie_textu.jsonl").write_text("{}\n", encoding="utf-8")
        (old / "progress.csv").write_text("derived", encoding="utf-8")
        before = (old / "prirazeni.csv").read_bytes()
        app = self.new_app()
        shared, local = self.root / ".lexo", app.local_dir
        self.assertEqual(app.store.assign.get("script.rpy"), "ep:1")
        self.assertTrue((shared / "assignments.csv").is_file())
        self.assertTrue((shared / "lexo.json").is_file())
        self.assertFalse((shared / "progress.csv").exists())                       # derived files are not copied
        self.assertTrue((local / "snapshots" / "snap-20260101-000000-manual.zip").is_file())
        self.assertTrue((local / "text_history.jsonl").is_file())
        self.assertNotIn(self.root, local.parents)                                 # nothing bulky in the game folder
        self.assertEqual((old / "prirazeni.csv").read_bytes(), before)             # the old folder is untouched
        self.assertTrue(any("copied from" in render(n) for n in app.notices))
        again = self.new_app()                                                     # second open: nothing is copied again
        self.assertEqual(again.local_dir, local)
        self.assertFalse(any("copied from" in render(n) for n in again.notices))

    def test_a_project_without_old_data_gets_only_an_id_file(self):
        self.basic_project()
        app = self.new_app()
        self.assertTrue((self.root / ".lexo" / "project-id").is_file())
        self.assertEqual(app.local_dir.parent, self.cfg_dir / "projects")

    def test_nothing_to_do_without_old_files(self):
        self.assertEqual(migrate.run(self.d), [])
        self.assertEqual(list(self.d.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
