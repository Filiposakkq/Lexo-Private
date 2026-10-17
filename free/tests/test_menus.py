"""Menus of a game script: the choices, the question or the lines before, what follows each choice; and the app call that joins them with the translation."""
import unittest

from helpers import TempCase

from lexo import menus

SCRIPT = [
    'label start:',                                  # 1
    '    scene room',                                # 2
    '    mc "I need coffee."',                       # 3
    '    js "Same."',                                # 4
    '    menu:',                                     # 5
    '        "Invite her" if met_her:',              # 6
    '            mc "Want a coffee?"',               # 7
    '            js "Sure."',                        # 8
    '            jump cafe',                         # 9
    '        "Leave":',                              # 10
    '            "You walk away."',                  # 11
    '            menu:',                             # 12
    '                "Look back":',                  # 13
    '                    mc "Bye."',                 # 14
    '                "Do not":',                     # 15
    '                    pass',                      # 16
    '    mc "Later."',                               # 17
    '    menu:',                                     # 18
    '        mc "What now?"',                        # 19
    '        "Sleep":',                              # 20
    '            return',                            # 21
]


class FindMenuTests(unittest.TestCase):
    def test_a_menu_without_a_question_gives_the_lines_said_before_it(self):
        m = menus.find_menu(SCRIPT, 10)
        self.assertEqual((m["menu_line"], m["question"], m["before"]), (5, [], [3, 4]))
        self.assertEqual([(o["n"], o["text"], o["cond"], o["after"]) for o in m["options"]],
                         [(6, "Invite her", "met_her", [7, 8]), (10, "Leave", "", [11])])         # the inner menu is not part of "Leave"
        self.assertEqual(m["current"], 1)

    def test_a_menu_inside_a_choice_is_found_on_its_own(self):
        m = menus.find_menu(SCRIPT, 15)
        self.assertEqual((m["menu_line"], [o["text"] for o in m["options"]], m["current"]), (12, ["Look back", "Do not"], 1))

    def test_a_question_inside_the_menu_replaces_the_lines_before_it(self):
        m = menus.find_menu(SCRIPT, 20)
        self.assertEqual((m["menu_line"], m["question"], m["before"]), (18, [19], []))

    def test_a_line_that_is_no_choice_gives_nothing(self):
        self.assertIsNone(menus.find_menu(SCRIPT, 3))
        self.assertIsNone(menus.find_menu(SCRIPT, 99))
        self.assertIsNone(menus.find_menu(['label a:', '    "Text":', '        pass'], 2))        # a string with a colon, but no menu above

    def test_what_is_a_line_of_dialogue(self):
        for line in ('    "Hi."', '    mc "Hi."', '    mc happy "Hi." with vpunch', "    mc 'Hi.'"):
            self.assertTrue(menus.is_say(line), line)
        for line in ('    show anna "x"', '    jump cafe', '    $ x = "a"', '    # mc "Hi."', '    if x == "a":', '    play music "a.ogg"'):
            self.assertFalse(menus.is_say(line), line)


class MenuInTheAppTests(TempCase):
    def test_the_choice_is_joined_with_its_menu_the_question_and_the_translations(self):
        game = self.tmp / "game"
        tl = game / "tl" / "czech"
        tl.mkdir(parents=True)
        (game / "script.rpy").write_text("\n".join(SCRIPT) + "\n", encoding="utf-8")
        (tl / "script.rpy").write_text(
            '# game/script.rpy:3\ntranslate czech a_1:\n\n    # mc "I need coffee."\n    mc "Potřebuju kafe."\n\n'
            '# game/script.rpy:4\ntranslate czech a_2:\n\n    # js "Same."\n    js "Taky."\n\n'
            '# game/script.rpy:7\ntranslate czech a_3:\n\n    # mc "Want a coffee?"\n    mc "Dáš si kafe?"\n\n'
            'translate czech strings:\n\n'
            '    # game/script.rpy:6\n    old "Invite her"\n    new "Pozvat ji"\n\n'
            '    # game/script.rpy:10\n    old "Leave"\n    new "Odejít"\n\n', encoding="utf-8")
        app = self.new_app(open_root=False)
        app.open_project(tl)
        leave = next(e for e in app.project.entries("script.rpy") if e.unit == "string" and e.source == "Leave")
        r = app.entry_menu("script.rpy", leave.line)
        self.assertTrue(r["available"])
        self.assertEqual([x["entry"]["target"] for x in r["before"]], ["Potřebuju kafe.", "Taky."])
        self.assertEqual([(o["text"], o["entry"]["target"], o["current"]) for o in r["options"]], [("Invite her", "Pozvat ji", False), ("Leave", "Odejít", True)])
        self.assertEqual(r["options"][0]["after"][0]["entry"]["target"], "Dáš si kafe?")       # what follows the choice
        self.assertIsNone(r["options"][0]["after"][1]["entry"])                                    # not in the translation: the script line is shown
        self.assertEqual(r["options"][0]["after"][1]["script"], 'js "Sure."')
        dialogue = next(e for e in app.project.entries("script.rpy") if e.unit == "dialogue")
        self.assertFalse(app.entry_menu("script.rpy", dialogue.line)["available"])


if __name__ == "__main__":
    unittest.main()
