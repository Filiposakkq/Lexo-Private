# A small test launcher for Aside: choose how to run it on this PC (Free, Pro with everything, Pro demo, Pro with the real licence file),
# which build (release is much faster than debug), which interface (classic, new, both), build first when the sources are newer, and the server opens in the browser.
# Start it with Spustit-test.cmd. The texts are ASCII on purpose (no diacritics) so that the file needs no special encoding.
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Target = 'D:\rust-target-msvc'            # see the Rust toolchain notes: build output is on D:
$Log = Join-Path $env:TEMP 'aside-test-server.log'
$Settings = Join-Path $env:LOCALAPPDATA 'AsideLauncher.json'      # what was chosen last time
$OldState = Join-Path $env:TEMP 'aside-test-folder.txt'           # the folder, from the older launcher
$UiNextDir = Join-Path $Repo 'rust\resources\ui-next'
$UiClassicDir = Join-Path $Repo 'rust\resources\ui-classic'
$script:Started = @()                      # the processes THIS launcher started: only these are stopped
$script:LastUrl = $null

function Set-RustEnv {
  $env:RUSTUP_HOME = 'E:\rust\rustup'; $env:CARGO_HOME = 'E:\rust\cargo'
  $env:Path = 'E:\rust\cargo\bin;' + $env:Path; $env:CARGO_TARGET_DIR = $Target
}
# Start-Process does not quote arguments: a path with a space would be cut in pieces
function Q($s) { '"' + $s + '"' }
function ExePath($profile, $name) { Join-Path $Target ($profile + '\' + $name + '.exe') }

# ---- is a build older than its sources?
function Newest($paths, $skip) {
  $best = [datetime]::MinValue
  foreach ($p in $paths) {
    if (-not (Test-Path $p)) { continue }
    $items = if ((Get-Item $p).PSIsContainer) { Get-ChildItem $p -Recurse -File -ErrorAction SilentlyContinue } else { Get-Item $p }
    foreach ($f in $items) {
      if ($skip -and ($skip | Where-Object { $f.FullName -like "*\$_\*" })) { continue }
      if ($f.LastWriteTime -gt $best) { $best = $f.LastWriteTime }
    }
  }
  $best
}
function Rust-Sources { Newest @((Join-Path $Repo 'rust\crates'), (Join-Path $Repo 'rust\Cargo.toml'), (Join-Path $Repo 'rust\Cargo.lock'), (Join-Path $Repo 'rust\resources'), (Join-Path $Repo 'pro\rust')) @('ui-next', 'ui-classic', 'packs', 'target', 'node_modules') }
function Ui-Sources { Newest @((Join-Path $Repo 'ui-next-src\src'), (Join-Path $Repo 'ui-next-src\public'), (Join-Path $Repo 'ui-next-src\index.html'), (Join-Path $Repo 'ui-next-src\package.json')) @('node_modules') }
function Build-Text($profile) {
  $exe = ExePath $profile 'lexo-server'
  if (-not (Test-Path $exe)) { return 'chybi - sestavi se' }
  $t = (Get-Item $exe).LastWriteTime
  if ((Rust-Sources) -gt $t) { return 'sestaveno ' + $t.ToString('d.M. HH:mm') + ' - ZASTARALE (zdroje jsou novejsi)' }
  'sestaveno ' + $t.ToString('d.M. HH:mm') + ' - aktualni'
}
function Ui-Text {
  $idx = Join-Path $UiNextDir 'index.html'
  if (-not (Test-Path $idx)) { return 'nove rozhrani: nesestaveno' }
  $t = (Get-Item $idx).LastWriteTime
  if ((Ui-Sources) -gt $t) { return 'nove rozhrani: ZASTARALE (zdroje jsou novejsi)' }
  'nove rozhrani: aktualni (' + $t.ToString('d.M. HH:mm') + ')'
}

# ---- stop only what this launcher started (and the Pro sidecar that the server started), never another copy of Aside that is running
function Stop-Tree($procId) {
  foreach ($c in (Get-CimInstance Win32_Process -Filter "ParentProcessId=$procId" -ErrorAction SilentlyContinue)) { Stop-Tree $c.ProcessId }
  Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
}
function Stop-Aside { foreach ($id in $script:Started) { Stop-Tree $id }; $script:Started = @() }
function Others-Running {
  $mine = $script:Started
  @(Get-Process lexo-server, lexo-app -ErrorAction SilentlyContinue | Where-Object { $mine -notcontains $_.Id })
}

# ---- the form
$form = New-Object Windows.Forms.Form
$form.Text = 'Aside - test launcher'; $form.Size = New-Object Drawing.Size(560, 760); $form.StartPosition = 'CenterScreen'
$form.Font = New-Object Drawing.Font('Segoe UI', 10); $form.FormBorderStyle = 'FixedDialog'; $form.MaximizeBox = $false

function Add-Group($text, $y, $h) { $g = New-Object Windows.Forms.GroupBox; $g.Text = $text; $g.Location = "12,$y"; $g.Size = "524,$h"; $form.Controls.Add($g); $g }
function Add-Radios($group, $items) {
  $map = @{}; $yy = 24
  foreach ($k in $items.Keys) { $r = New-Object Windows.Forms.RadioButton; $r.Text = $items[$k]; $r.Location = "14,$yy"; $r.Size = '496,26'; $group.Controls.Add($r); $map[$k] = $r; $yy += 28 }
  $map
}
function Add-Check($text, $y) { $c = New-Object Windows.Forms.CheckBox; $c.Text = $text; $c.Location = "16,$y"; $c.Size = '500,26'; $form.Controls.Add($c); $c }

$gMode = Add-Group 'Pro balicek' 8 142
$modes = [ordered]@{
  'free'    = 'Free - bez Pro balicku (LEXO_NO_PRO)'
  'prodev'  = 'Pro - vse odemcene (vyvojove, bez licencniho souboru)'
  'prodemo' = 'Pro - demo s limity (LEXO_PRO_DEMO)'
  'proreal' = 'Pro - skutecny licencni soubor z tohoto PC'
}
$radios = Add-Radios $gMode $modes

$gBuild = Add-Group 'Build serveru' 156 98
$builds = [ordered]@{ 'release' = 'Rychly (release) - doporuceno, nacitani je mnohem rychlejsi'; 'debug' = 'Ladici (debug) - pomaly, ale sestavi se rychleji' }
$bradios = Add-Radios $gBuild $builds
$buildInfo = New-Object Windows.Forms.Label; $buildInfo.Location = '16,258'; $buildInfo.Size = '520,22'; $buildInfo.ForeColor = [Drawing.Color]::FromArgb(224, 150, 154); $form.Controls.Add($buildInfo)

$gUi = Add-Group 'Rozhrani' 284 98
$uis = [ordered]@{ 'classic' = 'Klasicke'; 'next' = 'Nove (/next/)'; 'both' = 'Obe vedle sebe (jeden server, dve karty)' }
$uradios = Add-Radios $gUi $uis
$uiInfo = New-Object Windows.Forms.Label; $uiInfo.Location = '16,386'; $uiInfo.Size = '520,22'; $uiInfo.ForeColor = [Drawing.Color]::FromArgb(224, 150, 154); $form.Controls.Add($uiInfo)

$lbl = New-Object Windows.Forms.Label; $lbl.Text = 'Slozka prekladu (prazdne = bez projektu):'; $lbl.Location = '16,414'; $lbl.Size = '500,22'; $form.Controls.Add($lbl)
$folder = New-Object Windows.Forms.TextBox; $folder.Location = '16,438'; $folder.Size = '420,26'; $form.Controls.Add($folder)
$browse = New-Object Windows.Forms.Button; $browse.Text = '...'; $browse.Location = '444,436'; $browse.Size = '92,30'
$browse.Add_Click({ $d = New-Object Windows.Forms.FolderBrowserDialog; if ($folder.Text -and (Test-Path $folder.Text)) { $d.SelectedPath = $folder.Text }; if ($d.ShowDialog() -eq 'OK') { $folder.Text = $d.SelectedPath } })
$form.Controls.Add($browse)

$selftest = Add-Check 'Zkusebni projekt (--selftest) misto slozky' 472
$clean = Add-Check 'Cista nastaveni (docasna slozka, nic se neulozi do tvych)' 498
$asapp = Add-Check 'Jako appka (okno Tauri) misto prohlizece' 524
$build = Add-Check 'Nejdriv sestavit server (cargo build; zaskrtne se samo, kdyz je zastaraly)' 550
$uib = Add-Check 'Nejdriv sestavit nove rozhrani (npm; zaskrtne se samo, kdyz je zastarale)' 576
$kill = Add-Check 'Ukoncit jine bezici kopie Aside (jinak zustanou, ale mohou drzet zamek projektu)' 602

$go = New-Object Windows.Forms.Button; $go.Text = 'Spustit'; $go.Location = '16,640'; $go.Size = '140,40'
$stop = New-Object Windows.Forms.Button; $stop.Text = 'Zastavit'; $stop.Location = '164,640'; $stop.Size = '100,40'
$reopen = New-Object Windows.Forms.Button; $reopen.Text = 'Znovu otevrit'; $reopen.Location = '272,640'; $reopen.Size = '130,40'
$logb = New-Object Windows.Forms.Button; $logb.Text = 'Log'; $logb.Location = '410,640'; $logb.Size = '70,40'
$status = New-Object Windows.Forms.Label; $status.Location = '16,690'; $status.Size = '520,26'; $status.Text = ''
$form.Controls.AddRange(@($go, $stop, $reopen, $logb, $status))

# ---- what was chosen last time
function Check-One($map, $key, $fallback) { if ($key -and $map.Contains($key)) { $map[$key].Checked = $true } else { $map[$fallback].Checked = $true } }
$saved = $null
if (Test-Path $Settings) { try { $saved = Get-Content $Settings -Raw | ConvertFrom-Json } catch { $saved = $null } }
Check-One $radios $saved.mode 'prodev'
Check-One $bradios $saved.build 'release'
Check-One $uradios $saved.ui 'both'
if ($saved -and $saved.folder) { $folder.Text = [string]$saved.folder } elseif (Test-Path $OldState) { $folder.Text = (Get-Content $OldState -Raw).Trim() }
if ($saved) { $clean.Checked = [bool]$saved.clean; $asapp.Checked = [bool]$saved.asapp }
function Selected($map) { ($map.GetEnumerator() | Where-Object { $_.Value.Checked } | Select-Object -First 1).Key }
function Save-Settings {
  @{ mode = (Selected $radios); build = (Selected $bradios); ui = (Selected $uradios); folder = $folder.Text.Trim(); clean = $clean.Checked; asapp = $asapp.Checked } | ConvertTo-Json | Set-Content $Settings
}

# ---- the state of the builds, shown and used to tick "build first" by itself
function Refresh-Info {
  $p = Selected $bradios
  $text = Build-Text $p
  $buildInfo.Text = $p + ': ' + $text
  $build.Checked = ($text -like 'chybi*' -or $text -like '*ZASTARALE*')
  $u = Ui-Text
  $uiInfo.Text = $u
  $uib.Checked = ((Selected $uradios) -ne 'classic') -and ($u -like '*nesestaveno*' -or $u -like '*ZASTARALE*')
}
foreach ($r in $bradios.Values) { $r.Add_CheckedChanged({ Refresh-Info }) }
foreach ($r in $uradios.Values) { $r.Add_CheckedChanged({ Refresh-Info }) }
$form.Add_Shown({ $status.Text = 'Zjistuji stav sestaveni...'; $form.Refresh(); Refresh-Info; $status.Text = '' })

$stop.Add_Click({ Stop-Aside; $status.Text = 'Zastaveno.' })
$reopen.Add_Click({ if ($script:LastUrl) { Start-Process $script:LastUrl } else { $status.Text = 'Zatim nic nebezi.' } })
$logb.Add_Click({ if (Test-Path $Log) { Start-Process notepad.exe (Q $Log) } else { $status.Text = 'Log zatim neexistuje.' } })
$form.Add_FormClosed({ Stop-Aside })

$go.Add_Click({
  try {
    $go.Enabled = $false
    Stop-Aside
    $profile = Selected $bradios
    $flag = if ($profile -eq 'release') { @('--release') } else { @() }
    $Server = ExePath $profile 'lexo-server'; $Sidecar = ExePath $profile 'lexo-pro-sidecar'; $App = ExePath $profile 'lexo-app'
    $uiMode = Selected $uradios
    $others = Others-Running
    if ($others.Count -and $kill.Checked) { $others | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 400 }
    elseif ($others.Count) { $status.Text = 'Pozor: bezi jina kopie Aside (PID ' + (($others | ForEach-Object { $_.Id }) -join ', ') + ').'; $form.Refresh() }

    if ($uib.Checked) {
      $status.Text = 'Sestavuji nove rozhrani...'; $form.Refresh(); $env:Path = 'E:\node;' + $env:Path
      Push-Location (Join-Path $Repo 'ui-next-src')
      if (-not (Test-Path 'node_modules')) { & npm.cmd install --no-audit --no-fund }
      & npm.cmd run build; $nb = $LASTEXITCODE; Pop-Location
      if ($nb -ne 0) { $status.Text = 'Sestaveni noveho rozhrani selhalo (viz okno konzole).'; return }
    }
    if ($build.Checked) {
      $status.Text = 'Sestavuji server (' + $profile + ')...'; $form.Refresh(); Set-RustEnv
      Push-Location (Join-Path $Repo 'rust'); & cargo +stable-x86_64-pc-windows-msvc build @flag -p lexo-server; $a = $LASTEXITCODE; Pop-Location
      Push-Location (Join-Path $Repo 'pro\rust'); & cargo +stable-x86_64-pc-windows-msvc build @flag --bins; $b = $LASTEXITCODE; Pop-Location
      if ($asapp.Checked) { Push-Location (Join-Path $Repo 'rust'); & cargo +stable-x86_64-pc-windows-msvc build @flag -p lexo-app; if ($LASTEXITCODE -ne 0) { $a = 1 }; Pop-Location }
      if ($a -ne 0 -or $b -ne 0) { $status.Text = 'Sestaveni selhalo (viz okno konzole).'; return }
      Refresh-Info
    }
    if (-not $asapp.Checked -and -not (Test-Path $Server)) { $status.Text = 'Chybi ' + $Server + ' - zaskrtni "sestavit".'; return }
    if ($asapp.Checked -and -not (Test-Path $App)) { $status.Text = 'Chybi ' + $App + ' - zaskrtni "sestavit".'; return }
    if ($uiMode -ne 'classic' -and -not (Test-Path (Join-Path $UiNextDir 'index.html'))) { $status.Text = 'Nove rozhrani neni sestavene - zaskrtni "sestavit nove rozhrani".'; return }

    $mode = Selected $radios
    # a clean environment for this run, then the mode
    foreach ($v in 'LEXO_NO_PRO', 'LEXO_PRO_DEMO', 'LEXO_LICENCE_FILE', 'LEXO_PRO_SIDECAR', 'LEXO_CONFIG_DIR') { Remove-Item "Env:$v" -ErrorAction SilentlyContinue }
    if ($mode -eq 'free') { $env:LEXO_NO_PRO = '1' }
    else {
      $env:LEXO_PRO_SIDECAR = $Sidecar
      if ($mode -eq 'prodev') { $env:LEXO_LICENCE_FILE = Join-Path $env:TEMP 'aside-no-licence.json' }      # no file: the development build counts as licensed
      if ($mode -eq 'prodemo') { $env:LEXO_PRO_DEMO = '1'; $env:LEXO_LICENCE_FILE = Join-Path $env:TEMP 'aside-no-licence.json' }
    }
    if ($clean.Checked) { $env:LEXO_CONFIG_DIR = Join-Path $env:TEMP ('aside-test-cfg-' + [guid]::NewGuid().ToString('N').Substring(0, 8)) }
    $env:LEXO_UI_NEXT_DIR = $UiNextDir
    Save-Settings
    if ($asapp.Checked) {
      $p = if (-not $selftest.Checked -and $folder.Text.Trim()) { Start-Process $App -ArgumentList (Q $folder.Text.Trim()) -PassThru } else { Start-Process $App -PassThru }
      $script:Started += $p.Id
      $status.Text = 'Appka spustena (' + $profile + ').'; return
    }
    $args2 = @('--ui', (Q $UiClassicDir), '--packs', (Q (Join-Path $Repo 'rust\resources\packs')))
    if ($selftest.Checked) { $args2 += '--selftest' }
    elseif ($folder.Text.Trim()) { $args2 += @('--project', (Q $folder.Text.Trim())) }
    if (Test-Path $Log) { Remove-Item $Log -Force }
    $p = Start-Process $Server -ArgumentList $args2 -RedirectStandardOutput $Log -WindowStyle Hidden -PassThru
    $script:Started += $p.Id
    $url = $null
    for ($i = 0; $i -lt 75 -and -not $url; $i++) {
      Start-Sleep -Milliseconds 200
      if (Test-Path $Log) { $m = Select-String -Path $Log -Pattern 'http://\S+' | Select-Object -First 1; if ($m) { $url = $m.Matches[0].Value } }
      if ($p.HasExited) { break }
    }
    if (-not $url) { $status.Text = 'Server se nespustil (tlacitko Log ukaze proc).'; return }
    $base = ($url -replace '\?.*$', '').TrimEnd('/')
    if ($uiMode -eq 'both') {
      # one server, two tabs: both interfaces use the same Core and the same project, so nothing fights over the lock
      Start-Process $url; Start-Process ($base + '/next/'); $script:LastUrl = $base + '/next/'
      $status.Text = "Bezi (" + $profile + ", klasicke i nove): $base"
    } else {
      $open = if ($uiMode -eq 'next') { $base + '/next/' } else { $url }
      Start-Process $open; $script:LastUrl = $open; $status.Text = "Bezi (" + $profile + "): $open"
    }
  } catch { $status.Text = 'Chyba: ' + $_.Exception.Message }
  finally { $go.Enabled = $true }
})

# the look: dark red like the licence admin
$bg = [Drawing.Color]::FromArgb(24, 14, 16); $panel = [Drawing.Color]::FromArgb(38, 22, 25); $red = [Drawing.Color]::FromArgb(176, 38, 46); $txt = [Drawing.Color]::FromArgb(236, 226, 226)
$form.BackColor = $bg; $form.ForeColor = $txt
function Skin($c) {
  foreach ($x in $c.Controls) {
    if ($x -is [Windows.Forms.Button]) { $x.FlatStyle = 'Flat'; $x.FlatAppearance.BorderColor = $red; $x.FlatAppearance.BorderSize = 1; $x.BackColor = $panel; $x.ForeColor = $txt; $x.Cursor = 'Hand' }
    elseif ($x -is [Windows.Forms.TextBox]) { $x.BackColor = $panel; $x.ForeColor = $txt; $x.BorderStyle = 'FixedSingle' }
    elseif ($x -is [Windows.Forms.GroupBox]) { $x.ForeColor = [Drawing.Color]::FromArgb(224, 110, 116); Skin $x }
    elseif ($x -is [Windows.Forms.RadioButton] -or $x -is [Windows.Forms.CheckBox]) { $x.ForeColor = $txt; $x.FlatStyle = 'Standard' }
  }
}
Skin $form
$go.BackColor = $red; $go.ForeColor = [Drawing.Color]::White; $go.Font = New-Object Drawing.Font('Segoe UI Semibold', 11)
$status.ForeColor = [Drawing.Color]::FromArgb(224, 150, 154)

[void]$form.ShowDialog()
