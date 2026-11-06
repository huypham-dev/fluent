# Installs or updates TextActions on Windows from the latest GitHub release.
#
#   irm https://raw.githubusercontent.com/huypham-dev/text-actions/main/install.ps1 | iex
#
# The body is a script block and reports errors with throw, because `exit` would
# close the user's PowerShell window when the script runs through iex.
& {
    $ErrorActionPreference = 'Stop'
    # Windows PowerShell 5.1 can default to a TLS version that GitHub rejects.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    # The progress bar makes Invoke-WebRequest much slower on Windows PowerShell 5.1.
    $ProgressPreference = 'SilentlyContinue'

    $repoUrl = 'https://github.com/huypham-dev/text-actions'

    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'TextActions requires 64-bit Windows.'
    }

    $request = [Net.WebRequest]::Create("$repoUrl/releases/latest")
    $request.Method = 'HEAD'
    $response = $request.GetResponse()
    $latestUrl = $response.ResponseUri.AbsoluteUri
    $response.Dispose()
    if ($latestUrl -notmatch '/releases/tag/v(.+)$') {
        throw "Could not find the latest release at $repoUrl/releases"
    }
    $version = $Matches[1]

    $asset = "TextActions.Setup.$version.exe"
    $installer = Join-Path ([IO.Path]::GetTempPath()) $asset

    Write-Host "Downloading TextActions $version..."
    Invoke-WebRequest -UseBasicParsing -Uri "$repoUrl/releases/download/v$version/$asset" -OutFile $installer

    try {
        Write-Host 'Installing...'
        # Start-Process -Wait would also wait for the app the installer opens, and
        # that keeps running. WaitForExit waits for the installer alone.
        $process = Start-Process -FilePath $installer -PassThru
        $null = $process.Handle # Keeps ExitCode readable after the process ends.
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {
            throw "The installer stopped with exit code $($process.ExitCode)."
        }
    }
    finally {
        Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue
    }

    Write-Host ''
    Write-Host "TextActions $version is installed. It runs from the system tray."

    $defaultCli = Join-Path $env:USERPROFILE '.local\bin\claude.exe'
    if (-not (Test-Path -LiteralPath $defaultCli) -and -not (Get-Command claude -ErrorAction SilentlyContinue)) {
        Write-Host ''
        Write-Host 'TextActions also needs the Claude Code CLI, which was not found on this computer.'
        Write-Host 'Install it and log in: https://claude.com/product/claude-code'
    }
}
