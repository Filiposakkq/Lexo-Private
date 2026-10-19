label a:
    mc "Before the if."
    if x:
        mc "In the if branch."
    else:
        menu:
            "Warm it up":
                na "You warm it."
            "Leave it":
                $ y = 1
    mc "After."
    menu:
        "A":
            mc "Said A."
            if y:
                mc "Deep in A."
        "B":
            pass
