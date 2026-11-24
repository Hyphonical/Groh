#Requires -Version 5.1
<#
.SYNOPSIS
    Groh CLI Design Mockup - visual standard for the hand-written CLI

.DESCRIPTION
    A simulation of the Groh CLI: help, startup, the live containers
    table, containers appearing and disappearing as the stack changes,
    and the error and pipe cases.

    Layout guide for the hand-written parser and renderer in the Rust
    binary. At runtime the version and description come from Cargo.toml
    via env!("CARGO_PKG_VERSION") and env!("CARGO_PKG_DESCRIPTION");
    this mockup reads the same file so the two can never drift.

    Design language: "Paper and Ink", re-tinted with the terminal
    palette so the CLI feels like the dashboard. Columns everywhere,
    heavy rules as section headers, dimmed supporting text.
    Global max width 96.

    Behavior the mockup cannot show:
      - Human-facing output goes to stderr; help and version go to stdout.
      - Not a TTY (pipes, CI, services): no color, no in-place rewriting,
        one line per event, full UTC timestamps. The last scene shows this.
      - Container additions and removals are pushed live over SSE, the
        same way they appear in the web UI.

    Source is plain ASCII: every glyph comes from a [char] constant.

.NOTES
    Run:    powershell -ExecutionPolicy Bypass -File "CLI Design.ps1"
    Output: Terminal simulation (~20 seconds)
#>

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try { chcp 65001 | Out-Null } catch {}

# --- Version and description, read live from Cargo.toml --------------------
$manifest = Join-Path $PSScriptRoot "..\Cargo.toml"
$package = @()
$in_package = $false
foreach ($line in Get-Content $manifest) {
    if ($line -match '^\[package\]') { $in_package = $true; continue }
    if ($line -match '^\[')          { $in_package = $false }
    if ($in_package)                 { $package += $line }
}
$Version = ($package | Select-String '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$Description = ($package | Select-String '^description\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value

# --- Design tokens (terminal palette) ---------------------------------------
$HEAD         = "#ffffd4"   # headings, rule labels
$FG           = "#ffffd4"   # body text
$DIM          = "#778385"   # supporting text
$MUTED        = "#44474a"   # rules, prompts, chatter
$OK           = "#afb97a"   # sage - running, healthy, success
$FAIL         = "#c06d44"   # errors

$script:Width = 96
$esc   = [char]27
$H     = [string][char]0x2501   # heavy horizontal (rules)
$ELL   = [string][char]0x2026
$DOT   = [string][char]0x00B7
$EM    = [string][char]0x2014
$CHECK = [string][char]0x2713
$CROSS = [string][char]0x2715
$PLUS  = [string][char]0x002B
$MINUS = [string][char]0x2212

# --- Helpers -----------------------------------------------------------------
function Write-C {
    param([string]$Text, [string]$Color, [switch]$NoNewline)
    $hex = $Color.TrimStart('#')
    $r = [Convert]::ToInt32($hex.Substring(0, 2), 16)
    $g = [Convert]::ToInt32($hex.Substring(2, 2), 16)
    $b = [Convert]::ToInt32($hex.Substring(4, 2), 16)
    Write-Host "$esc[38;2;$r;$g;${b}m$Text$esc[0m" -NoNewline:$NoNewline
}
function Write-Rule {
    param([string]$Label)
    $fill = $H * ($script:Width - $Label.Length - 8)
    Write-C "  " $MUTED -NoNewline
    Write-C ($H * 2) $MUTED -NoNewline
    Write-C "  $Label  " $HEAD -NoNewline
    Write-C $fill $MUTED
}
function Cut {
    param([string]$Text, [int]$Max)
    if ($Text.Length -le $Max) { return $Text }
    return $Text.Substring(0, $Max - 1) + $ELL
}
function Pad {
    param([string]$Text, [int]$W)
    $visible = ($Text.ToCharArray() | Where-Object { $_ -ne [char]0x2026 }).Count
    return $Text + (" " * [Math]::Max(0, $W - $visible))
}
function Write-Group {
    param([string]$Name)
    Write-Host ""
    Write-C "  $Name" $HEAD
}
function Wrap {
    param([string]$Text, [int]$Max)
    $lines = @()
    $current = ""
    foreach ($word in ($Text -split ' ')) {
        if ($current -eq "") { $current = $word; continue }
        if (($current.Length + 1 + $word.Length) -le $Max) { $current += " " + $word }
        else { $lines += $current; $current = $word }
    }
    if ($current -ne "") { $lines += $current }
    return $lines
}
function Write-Flags {
    param([array]$Rows)
    $wFlag  = (($Rows | ForEach-Object { $_.Flag.Length })  | Measure-Object -Maximum).Maximum + 2
    $wAlias = (($Rows | ForEach-Object { $_.Alias.Length }) | Measure-Object -Maximum).Maximum + 2
    $wDesc  = (($Rows | ForEach-Object { $_.Desc.Length })  | Measure-Object -Maximum).Maximum + 2
    $wKind  = (($Rows | ForEach-Object { $_.Kind.Length })  | Measure-Object -Maximum).Maximum + 2
    foreach ($r in $Rows) {
        Write-C ("    " + (Pad $r.Flag $wFlag) + (Pad $r.Alias $wAlias)) $FG -NoNewline
        Write-C ((Pad $r.Desc $wDesc) + (Pad $r.Kind $wKind) + $r.Default) $DIM
    }
}
function Write-Examples {
    param([array]$Rows)
    $wCmd = (($Rows | ForEach-Object { $_.Command.Length }) | Measure-Object -Maximum).Maximum + 2
    foreach ($r in $Rows) {
        Write-C (Pad ("    " + $r.Command) ($wCmd + 4)) $FG -NoNewline
        Write-C $r.Desc $DIM
    }
}
function Write-Row {
    param([string]$Label, [string]$Value, [string]$Color = "#ffffd4", [string]$Note = "")
    Write-C ("  " + (Pad $Label 10)) $DIM -NoNewline
    Write-C $Value $Color -NoNewline
    if ($Note) { Write-C "  $Note" $DIM }
    Write-Host ""
}
function Write-Cmdline { param([string]$Text); Write-C "  ✦ $Text" $DIM }
function Write-Done {
    param([string]$Label, [string]$Text, [string]$Note = "")
    Write-C ("  " + (Pad $Label 10)) $DIM -NoNewline
    Write-C "$CHECK  $Text" $OK -NoNewline
    if ($Note) { Write-C "  $Note" $DIM }
    Write-Host ""
}
function Write-ErrorBlock {
    param([string]$Title, [string]$Detail, [string]$Hint = "")
    Write-C "  $CROSS  $Title" $FAIL
    if ($Detail) { Write-C "     $Detail" $FG }
    if ($Hint)   { Write-C "     $Hint" $DIM }
}
function Write-ContainerRow {
    param([string]$Name, [string]$State, [string]$StateColor, [string]$Detail)
    Write-C "  " $FG -NoNewline
    Write-C (Pad $Name 14) $FG -NoNewline
    Write-C (Pad $State 10) $StateColor -NoNewline
    Write-C $Detail $DIM
}
function Write-ContainerChange {
    param([string]$Mark, [string]$MarkColor, [string]$Name)
    Write-C ("  " + (Pad $Mark 3)) $MarkColor -NoNewline
    Write-C $Name $FG
}

Clear-Host

# ============================================================================
# 1. Groh --help
# ============================================================================
Write-Rule "Groh (v$Version)"
foreach ($line in (Wrap $Description ($script:Width - 2))) { Write-C "  $line" $DIM }
Write-Group "Usage"
Write-C "    Groh [options]" $FG
Write-Group "Options"
Write-Flags @(
    [pscustomobject]@{ Flag = "--bind";    Alias = "";   Desc = "Listen address";             Kind = "text"; Default = "0.0.0.0" }
    [pscustomobject]@{ Flag = "--port";    Alias = "";   Desc = "Port to listen on";          Kind = "int";  Default = "3000" }
    [pscustomobject]@{ Flag = "--no-color"; Alias = "";   Desc = "Plain output without color"; Kind = "bool"; Default = "" }
    [pscustomobject]@{ Flag = "--help";    Alias = "-h"; Desc = "Show help";                  Kind = "bool"; Default = "" }
    [pscustomobject]@{ Flag = "--version"; Alias = "-V"; Desc = "Show the version";           Kind = "bool"; Default = "" }
)
Write-Group "Examples"
Write-Examples @(
    [pscustomobject]@{ Command = "Groh";                   Desc = "the dashboard, on :3000" }
    [pscustomobject]@{ Command = "Groh --port 8080";       Desc = "the dashboard, on :8080" }
    [pscustomobject]@{ Command = "Groh --bind 127.0.0.1";  Desc = "only this machine" }
)
Start-Sleep -Milliseconds 600

# ============================================================================
# 2. Startup: identity rows, then listening. Containers come after this.
# ============================================================================
Write-Host ""
Write-Rule "Startup"
Write-Cmdline "Groh --port 3000"
Start-Sleep -Milliseconds 400
Write-Row "url"  "http://0.0.0.0:3000" $FG
Start-Sleep -Milliseconds 300
Write-Done "serve" "listening"
Start-Sleep -Milliseconds 500

# ============================================================================
# 3. Containers: the live table, one row per container in the stack.
# ============================================================================
Write-Host ""
Write-Rule "Containers"
Write-C ("  " + (Pad "name" 14) + (Pad "state" 10) + "detail") $DIM
Write-ContainerRow "onyun"      "running" $OK  "Up 3 days"
Write-ContainerRow "grafana"    "running" $OK  "Up 3 days"
Write-ContainerRow "prometheus" "exited"  $FAIL "Exited 2 hours ago"
Start-Sleep -Milliseconds 600

# ============================================================================
# 4. Live: a container is added to the stack, then one is removed.
#    The table updates in place; the + and - say what changed.
# ============================================================================
Write-Host ""
Write-Rule "Live"
Write-C "  Watching the stack $EM additions and removals appear as they happen." $DIM
Start-Sleep -Milliseconds 400
Write-ContainerChange $PLUS $OK "uptime-kuma"
Start-Sleep -Milliseconds 600
Write-ContainerChange $MINUS $FAIL "prometheus"
Start-Sleep -Milliseconds 600

# ============================================================================
# 5. Errors
# ============================================================================
function Show-Error {
    param([string]$Cmd, [string]$Title, [string]$Detail, [string]$Hint = "")
    Write-Host ""
    Write-Rule "Error"
    Write-Cmdline $Cmd
    Start-Sleep -Milliseconds 300
    Write-ErrorBlock $Title $Detail $Hint
    Start-Sleep -Milliseconds 300
}
Show-Error "Groh --port 3000" "failed to bind 0.0.0.0:3000" "address already in use"
Show-Error "Groh --port abc" "invalid value for '--port'" "expected a port number between 1 and 65535, got `"abc`"" "see Groh --help"

# ============================================================================
# 6. Not a terminal (plain output)
# ============================================================================
Write-Host ""
Write-Rule "Pipe"
Write-C "  ✦ Groh | tee groh.log" $DIM
Write-C "  Not a terminal, so plain output: no color, one line per event, UTC timestamps." $DIM
Write-Host "2026-11-24T20:14:02Z  startup    listening on http://0.0.0.0:3000"
Write-Host "2026-11-24T20:14:03Z  containers 3 running, 1 exited"
Write-Host "2026-11-24T20:15:41Z  containers uptime-kuma added"
Write-Host "2026-11-24T20:16:07Z  containers prometheus removed"
Start-Sleep -Milliseconds 700

# ============================================================================
# 7. Shutdown
# ============================================================================
Write-Host ""
Write-Rule "Shutdown"
Write-Done "serve" "shutting down"
Write-Host ""
Write-C "// End of Groh CLI Visual Standard" $MUTED
Write-Host ""