# A tiny test launcher for Aside: pick how to run it on this PC (Free, Pro with everything, Pro demo, Pro with the real licence file),
# optionally build first, and the server opens in the browser. Start it with Spustit-test.cmd.
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Target = 'D:\rust-target-msvc'            # see the Rust toolchain notes: build output is on D:
$Server = Join-Path $Target 'debug\lexo-server.exe'
$Sidecar = Join-Path $Target 'debug\lexo-pro-sidecar.exe'
$App = Join-Path $Target 'debug\lexo-app.exe'
$Log = Join-Path $env:TEMP 'aside-test-server.log'
$State = Join-Path $env:TEMP 'aside-test-folder.txt'

function Set-RustEnv {
  $env:RUSTUP_HOME = 'E:\rust\rustup'; $env:CARGO_HOME = 'E:\rust\cargo'
  $env:Path = 'E:\rust\cargo\bin;' + $env:Path; $env:CARGO_TARGET_DIR = $Target
}

# Start-Process does not quote arguments: a path with a space would be cut in pieces
function Q($s) { '"' + $s + '"' }

function Stop-Aside { Get-Process lexo-server, lexo-app, lexo-pro-sidecar -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue }

$form = New-Object Windows.Forms.Form
$form.Text = 'Aside - test launcher'; $form.Size = New-Object Drawing.Size(520, 510); $form.StartPosition = 'CenterScreen'
$form.Font = New-Object Drawing.Font('Segoe UI', 10); $form.FormBorderStyle = 'FixedDialog'; $form.MaximizeBox = $false

$modes = [ordered]@{
  'free'     = 'Free - bez Pro balicku (LEXO_NO_PRO)'
  'prodev'   = 'Pro - vse odemcene (vyvojove, bez licencniho souboru)'
  'prodemo'  = 'Pro - demo s limity (LEXO_PRO_DEMO)'
  'proreal'  = 'Pro - skutecny licencni soubor z tohoto PC'
}
$y = 12
$group = New-Object Windows.Forms.GroupBox; $group.Text = 'Jak spustit'; $group.Location = '12,8'; $group.Size = '480,160'
$form.Controls.Add($group)
$radios = @{}
$yy = 24
foreach ($k in $modes.Keys) {
  $r = New-Object Windows.Forms.RadioButton; $r.Text = $modes[$k]; $r.Location = "14,$yy"; $r.Size = '455,28'
  $group.Controls.Add($r); $radios[$k] = $r; $yy += 31
}
$radios['prodev'].Checked = $true

$lbl = New-Object Windows.Forms.Label; $lbl.Text = 'Slozka prekladu (prazdne = bez projektu):'; $lbl.Location = '14,208'; $lbl.Size = '470,22'
$form.Controls.Add($lbl)
$folder = New-Object Windows.Forms.TextBox; $folder.Location = '14,232'; $folder.Size = '390,26'
if (Test-Path $State) { $folder.Text = (Get-Content $State -Raw).Trim() }
$form.Controls.Add($folder)
$browse = New-Object Windows.Forms.Button; $browse.Text = '...'; $browse.Location = '412,230'; $browse.Size = '80,30'
$browse.Add_Click({ $d = New-Object Windows.Forms.FolderBrowserDialog; if ($d.ShowDialog() -eq 'OK') { $folder.Text = $d.SelectedPath } })
$form.Controls.Add($browse)

$selftest = New-Object Windows.Forms.CheckBox; $selftest.Text = 'Zkusebni projekt (--selftest) misto slozky'; $selftest.Location = '14,266'; $selftest.Size = '470,26'
$form.Controls.Add($selftest)
$clean = New-Object Windows.Forms.CheckBox; $clean.Text = 'Cista nastaveni (docasna slozka, nic se neulozi do tvych)'; $clean.Location = '14,294'; $clean.Size = '470,26'
$form.Controls.Add($clean)
$build = New-Object Windows.Forms.CheckBox; $build.Text = 'Nejdriv sestavit (cargo build, chvili to trva)'; $build.Location = '14,322'; $build.Size = '470,26'
$form.Controls.Add($build)

$asapp = New-Object Windows.Forms.CheckBox; $asapp.Text = 'Jako appka (okno Tauri) misto prohlizece'; $asapp.Location = '14,346'; $asapp.Size = '470,26'
$form.Controls.Add($asapp)
$go = New-Object Windows.Forms.Button; $go.Text = 'Spustit'; $go.Location = '14,380'; $go.Size = '150,40'; $go.BackColor = [Drawing.Color]::FromArgb(150, 30, 30); $go.ForeColor = 'White'
$stop = New-Object Windows.Forms.Button; $stop.Text = 'Zastavit'; $stop.Location = '172,380'; $stop.Size = '110,40'
$status = New-Object Windows.Forms.Label; $status.Location = '14,428'; $status.Size = '480,26'; $status.Text = ''
$form.Controls.AddRange(@($go, $stop, $status))

$stop.Add_Click({ Stop-Aside; $status.Text = 'Zastaveno.' })
$form.Add_FormClosed({ Stop-Aside })

$go.Add_Click({
  try {
    Stop-Aside
    if ($build.Checked) {
      $status.Text = 'Sestavuji...'; $form.Refresh(); Set-RustEnv
      Push-Location (Join-Path $Repo 'rust'); & cargo +stable-x86_64-pc-windows-msvc build -p lexo-server; $a = $LASTEXITCODE; Pop-Location
      Push-Location (Join-Path $Repo 'pro\rust'); & cargo +stable-x86_64-pc-windows-msvc build --bins; $b = $LASTEXITCODE; Pop-Location
      if ($asapp.Checked) { Push-Location (Join-Path $Repo 'rust'); & cargo +stable-x86_64-pc-windows-msvc build -p lexo-app; if ($LASTEXITCODE -ne 0) { $a = 1 }; Pop-Location }
      if ($a -ne 0 -or $b -ne 0) { $status.Text = 'Sestaveni selhalo (viz okno konzole).'; return }
    }
    if (-not $asapp.Checked -and -not (Test-Path $Server)) { $status.Text = 'Chybi lexo-server.exe - zaskrtni "sestavit".'; return }
    if ($asapp.Checked -and -not (Test-Path $App)) { $status.Text = 'Chybi lexo-app.exe - zaskrtni "sestavit".'; return }
    $mode = ($radios.GetEnumerator() | Where-Object { $_.Value.Checked } | Select-Object -First 1).Key
    # a clean environment for this run, then the mode
    foreach ($v in 'LEXO_NO_PRO', 'LEXO_PRO_DEMO', 'LEXO_LICENCE_FILE', 'LEXO_PRO_SIDECAR', 'LEXO_CONFIG_DIR') { Remove-Item "Env:$v" -ErrorAction SilentlyContinue }
    if ($mode -eq 'free') { $env:LEXO_NO_PRO = '1' }
    else {
      $env:LEXO_PRO_SIDECAR = $Sidecar
      if ($mode -eq 'prodev') { $env:LEXO_LICENCE_FILE = Join-Path $env:TEMP 'aside-no-licence.json' }      # no file: the development build counts as licensed
      if ($mode -eq 'prodemo') { $env:LEXO_PRO_DEMO = '1'; $env:LEXO_LICENCE_FILE = Join-Path $env:TEMP 'aside-no-licence.json' }
    }
    if ($clean.Checked) { $env:LEXO_CONFIG_DIR = Join-Path $env:TEMP ('aside-test-cfg-' + [guid]::NewGuid().ToString('N').Substring(0, 8)) }
    if ($asapp.Checked) {
      if (-not $selftest.Checked -and $folder.Text.Trim()) { Set-Content $State $folder.Text.Trim(); Start-Process $App -ArgumentList (Q $folder.Text.Trim()) } else { Start-Process $App }
      $status.Text = 'Appka spustena.'; return
    }
    $args2 = @('--ui', (Q (Join-Path $Repo 'rust\resources\ui')), '--packs', (Q (Join-Path $Repo 'rust\resources\packs')))
    if ($selftest.Checked) { $args2 += '--selftest' }
    elseif ($folder.Text.Trim()) { $args2 += @('--project', (Q $folder.Text.Trim())); Set-Content $State $folder.Text.Trim() }
    if (Test-Path $Log) { Remove-Item $Log -Force }
    $p = Start-Process $Server -ArgumentList $args2 -RedirectStandardOutput $Log -WindowStyle Hidden -PassThru
    $url = $null
    for ($i = 0; $i -lt 50 -and -not $url; $i++) {
      Start-Sleep -Milliseconds 200
      if (Test-Path $Log) { $m = Select-String -Path $Log -Pattern 'http://\S+' | Select-Object -First 1; if ($m) { $url = $m.Matches[0].Value } }
      if ($p.HasExited) { break }
    }
    if ($url) { Start-Process $url; $status.Text = "Bezi: $url" } else { $status.Text = 'Server se nespustil (log: ' + $Log + ')' }
  } catch { $status.Text = 'Chyba: ' + $_.Exception.Message }
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




