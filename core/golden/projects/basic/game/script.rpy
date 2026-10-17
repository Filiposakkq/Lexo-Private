define mc = Character("Mike", color="#ffaf05")
define sa = Character("Sage", color="#99cc33")
define narrator = Character(None)

label start:
    scene bg room with fade
    show sage smile at left
    mc "Hello there, friend."
    sa "Hey, it's you again!"
    menu:
        "Invite her" if met_her:
            mc "Want a coffee?"
            sa "Sure."
        "Leave":
            $ dk(-1)
            $ renpy.pause(0.2)
    scene bg street
    mc "Bye, [name]."
    sa "{i}See you{/i} around."
    "The end of the scene."
