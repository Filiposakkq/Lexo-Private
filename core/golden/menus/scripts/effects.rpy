menu Skip_Intro:
    "No":
                    jump game_start
    "Yes":
                    show skip_intro 2
                    menu Skip_Intro_Question_1:
                        "Yes (+1)":
                                        $ dylan_lap = True
                                        $ dylan += 1
                                        show skip_intro 3
                                        menu Skip_Intro_Question_2:
                                            "Deep":
                                                            jump day1
                        "No (+1)":
                                        $ good_wife += 1
                                        jump day1
