# Controls a detached zappette dev server.
#
# The server is spawned through WMI (Win32_Process.Create) with a .cmd wrapper,
# so it is fully detached from the calling shell: agent shells that start it
# return immediately instead of waiting on the server's process tree.
#
# Usage:
#   scripts/zappette-dev.ps1 start -Port 5001 -Mock   # mock input (no real keystrokes)
#   scripts/zappette-dev.ps1 start -Port 5000         # REAL input injection
#   scripts/zappette-dev.ps1 stop  -Port 5001
#   scripts/zappette-dev.ps1 log  -Port 5001 [-Follow]
#
# 'start' kills any stale server on the port, then rebuilds the binary and
# re-bundles assets (skip with -NoBuild) before spawning — so the page always
# serves fresh code. Kill must happen first: a running zappette.exe locks the
# binary and cargo could not relink over it.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet("start", "stop", "log")][string]$Command,
    [int]$Port = 5001,
    [switch]$Mock,
    [switch]$Follow,
    [switch]$NoBuild
)

$ErrorActionPreference = "Stop"
$root   = (Resolve-Path "$PSScriptRoot\..").Path
$exe    = Join-Path $root "target\debug\zappette.exe"
$stamp  = "zappette-dev-$Port"
$log    = Join-Path $root "target\$stamp.out.log"

function Get-ZappettePidOnPort {
    Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Where-Object {
            (Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName -eq "zappette"
        } |
        Select-Object -ExpandProperty OwningProcess -Unique
}

switch ($Command) {

    "start" {
        # Kill a stale server holding the port BEFORE building (a running
        # zappette.exe locks the binary), instead of failing to bind later.
        $stale = @(Get-ZappettePidOnPort)
        foreach ($procId in $stale) {
            Stop-Process -Id $procId -Force
            Write-Host "killed stale zappette pid=$procId on port $Port"
        }

        if (-not $NoBuild) {
            Push-Location $root
            try {
                cargo build
                if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
                topcoat asset bundle
                if ($LASTEXITCODE -ne 0) { throw "topcoat asset bundle failed with exit code $LASTEXITCODE" }
            }
            finally {
                Pop-Location
            }
        }

        if (-not (Test-Path $exe)) {
            throw "zappette.exe not found at $exe - run 'just build' first"
        }

        # Wrapper script: WMI spawns 'cmd /c wrapper', wrapper owns the log
        # handles so nothing is inherited by (or waits on) the caller's shell.
        $zappetteArgs = if ($Mock) { "--mock" } else { "" }
        $wrapper = Join-Path $root "target\$stamp.cmd"
        Set-Content -Path $wrapper -Encoding ascii -Value @(
            "@echo off"
            "cd /d `"$root`""
            "`"$exe`" $zappetteArgs --port $Port > `"$log`" 2>&1"
        )

        $result = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
            CommandLine      = "cmd /c `"$wrapper`""
            CurrentDirectory = $root
        }
        if ($result.ReturnValue -ne 0) {
            throw "WMI process spawn failed with code $($result.ReturnValue)"
        }

        # Wait (max 10s) for the port to come up; dump the log on failure.
        $up = $false
        foreach ($i in 1..50) {
            if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
                $up = $true
                break
            }
            Start-Sleep -Milliseconds 200
        }
        if (-not $up) {
            Get-Content $log -ErrorAction SilentlyContinue
            throw "zappette did not start listening on port $Port within 10s"
        }

        $mode = if ($Mock) { "mock (no real keystrokes)" } else { "REAL input injection" }
        Write-Host "zappette dev server: pid=$($result.ProcessId) port=$Port mode=$mode"
        Write-Host "log: $log"
    }

    "stop" {
        $procIds = @(Get-ZappettePidOnPort)
        if (-not $procIds) {
            Write-Host "no zappette server listening on port $Port"
            return
        }
        foreach ($procId in $procIds) {
            Stop-Process -Id $procId -Force
            Write-Host "stopped zappette pid=$procId on port $Port"
        }
        Start-Sleep -Milliseconds 300
        if (Get-ZappettePidOnPort) { throw "zappette still listening on port $Port" }
    }

    "log" {
        if (-not (Test-Path $log)) {
            Write-Host "no log yet for port $Port (server never started?)"
            return
        }
        if ($Follow) { Get-Content $log -Wait } else { Get-Content $log }
    }
}
