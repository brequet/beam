# Controls a detached beam dev server.
#
# The server is spawned through WMI (Win32_Process.Create) with a .cmd wrapper,
# so it is fully detached from the calling shell: agent shells that start it
# return immediately instead of waiting on the server's process tree.
#
# Usage:
#   scripts/beam-dev.ps1 start -Port 5001 -Mock   # mock input (no real keystrokes)
#   scripts/beam-dev.ps1 start -Port 5000         # REAL input injection
#   scripts/beam-dev.ps1 stop  -Port 5001
#   scripts/beam-dev.ps1 log  -Port 5001 [-Follow]
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet("start", "stop", "log")][string]$Command,
    [int]$Port = 5001,
    [switch]$Mock,
    [switch]$Follow
)

$ErrorActionPreference = "Stop"
$root   = (Resolve-Path "$PSScriptRoot\..").Path
$exe    = Join-Path $root "target\debug\beam.exe"
$stamp  = "beam-dev-$Port"
$log    = Join-Path $root "target\$stamp.out.log"

function Get-BeamPidOnPort {
    Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Where-Object {
            (Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName -eq "beam"
        } |
        Select-Object -ExpandProperty OwningProcess -Unique
}

switch ($Command) {

    "start" {
        if (-not (Test-Path $exe)) {
            throw "beam.exe not found at $exe - run 'just build' first"
        }

        # Kill a stale server holding the port instead of failing to bind.
        $stale = @(Get-BeamPidOnPort)
        foreach ($procId in $stale) {
            Stop-Process -Id $procId -Force
            Write-Host "killed stale beam pid=$procId on port $Port"
        }

        # Wrapper script: WMI spawns 'cmd /c wrapper', wrapper owns the log
        # handles so nothing is inherited by (or waits on) the caller's shell.
        $beamArgs = if ($Mock) { "--mock" } else { "" }
        $wrapper = Join-Path $root "target\$stamp.cmd"
        Set-Content -Path $wrapper -Encoding ascii -Value @(
            "@echo off"
            "cd /d `"$root`""
            "`"$exe`" $beamArgs --port $Port > `"$log`" 2>&1"
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
            throw "beam did not start listening on port $Port within 10s"
        }

        $mode = if ($Mock) { "mock (no real keystrokes)" } else { "REAL input injection" }
        Write-Host "beam dev server: pid=$($result.ProcessId) port=$Port mode=$mode"
        Write-Host "log: $log"
    }

    "stop" {
        $procIds = @(Get-BeamPidOnPort)
        if (-not $procIds) {
            Write-Host "no beam server listening on port $Port"
            return
        }
        foreach ($procId in $procIds) {
            Stop-Process -Id $procId -Force
            Write-Host "stopped beam pid=$procId on port $Port"
        }
        Start-Sleep -Milliseconds 300
        if (Get-BeamPidOnPort) { throw "beam still listening on port $Port" }
    }

    "log" {
        if (-not (Test-Path $log)) {
            Write-Host "no log yet for port $Port (server never started?)"
            return
        }
        if ($Follow) { Get-Content $log -Wait } else { Get-Content $log }
    }
}
