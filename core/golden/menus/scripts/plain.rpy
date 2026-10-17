label start:
    scene room
    mc "I need coffee."
    js "Same."
    menu:
        "Invite her" if met_her:
            mc "Want a coffee?"
            js "Sure."
            jump cafe
        "Leave":
            "You walk away."
            menu:
                "Look back":
                    mc "Bye."
                "Do not":
                    pass
    mc "Later."
    menu:
        mc "What now?"
        "Sleep":
            return
