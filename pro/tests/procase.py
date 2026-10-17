"""Base class of the tests of Pro features that run through the application: every app made by `new_app` has the Pro pack installed."""
from helpers import TempCase

import lexo_pro
from lexo.extensions import ExtensionHost


class ProCase(TempCase):
    def new_app(self, *args, **kwargs):
        app = super().new_app(*args, **kwargs)
        app.ext = ExtensionHost()
        app.ext.register(lexo_pro.provider())
        return app
