# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec for Lexo.exe (one file, no console window).
# Build with build_exe.bat (or: py -m PyInstaller --noconfirm --clean Lexo.spec).
# The UI files (lexo/ui) are bundled; settings live in %APPDATA%\Lexo, project data in
# <translation folder>\_progress_data, so the exe does not depend on the folder it is started from.
import os

here = os.path.dirname(os.path.abspath(SPEC))

datas = [(os.path.join(here, "lexo", "ui"), os.path.join("lexo", "ui")),
         (os.path.join(here, "lexo", "packs"), os.path.join("lexo", "packs"))]
binaries, hiddenimports = [], []
try:                                   # pywebview is optional; without it the browser fallback is used
    from PyInstaller.utils.hooks import collect_all
    d, b, h = collect_all("webview")
    datas += d
    binaries += b
    hiddenimports += h
except Exception as exc:               # noqa: BLE001
    print("pywebview not bundled:", exc)

a = Analysis(
    [os.path.join(here, "run_lexo.py")],
    pathex=[here],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    runtime_hooks=[],
    excludes=["pytest", "unittest.mock"],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="Lexo",
    debug=False,
    strip=False,
    upx=False,
    console=False,
    icon=os.path.join(here, "assets", "icon.ico"),
)
