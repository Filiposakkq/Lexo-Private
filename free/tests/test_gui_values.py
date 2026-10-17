"""gui.rpy values for the scene preview: negative numbers and fractions (a decimal point means a fraction of the container in Ren'Py)."""
import unittest

from helpers import TempCase, dialog

GUI = '''gui.init(1920, 1080)
define gui.text_size = 32
define gui.textbox_height = 270
define gui.name_xpos = 132
define gui.name_ypos = 0.1
define gui.dialogue_ypos = 0.33
define gui.dialogue_xpos = -20
define gui.dialogue_width = 0.5
define gui.name_text_size = 50
'''


class GuiValuesTests(TempCase):
    def open(self, gui):
        game = self.tmp / "gamefolder" / "game"
        tl = game / "tl" / "czech"
        tl.mkdir(parents=True)
        (game / "gui.rpy").write_text(gui, encoding="utf-8")
        (tl / "script.rpy").write_text(dialog("a1", "mc", "Hello there, friend.", None, 5), encoding="utf-8")
        app = self.new_app(open_root=False)
        app.open_project(tl)
        return app.game_gui()

    def test_fractions_are_relative_to_the_box_and_negative_numbers_are_read(self):
        g = self.open(GUI)
        self.assertEqual((g["name_ypos"], g["dialogue_ypos"], g["dialogue_xpos"], g["dialogue_width"]), (27, 89, -20, 960))
        self.assertEqual((g["text_size"], g["textbox_height"], g["name_xpos"], g["name_text_size"]), (32, 270, 132, 50))

    def test_a_fractional_box_height_is_a_part_of_the_screen(self):
        g = self.open(GUI.replace("textbox_height = 270", "textbox_height = 0.25"))
        self.assertEqual((g["textbox_height"], g["name_ypos"]), (270, 27))            # 0.25 * 1080

    def test_a_negative_name_position_puts_the_name_above_the_box(self):
        g = self.open("define gui.name_ypos = -66\n")
        self.assertEqual(g["name_ypos"], -66)


if __name__ == "__main__":
    unittest.main()
