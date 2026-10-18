# Installs claudius from a GitHub release on Windows.
#
#   irm https://github.com/devlato/claudius/releases/latest/download/install.ps1 | iex
#
# With options, run it as a script block:
#
#   & ([scriptblock]::Create((irm https://github.com/devlato/claudius/releases/latest/download/install.ps1))) -Yes
#
# Options:
#   -Yes       run `claudius setup` afterwards without asking (the sample packs, about 2.2 GB)
#   -NoSetup   do not offer to run it
#
# Environment:
#   CLAUDIUS_VERSION       the version to install, e.g. 0.2.0 (default: the latest release)
#   CLAUDIUS_INSTALL_DIR   where the binaries go (default: %LOCALAPPDATA%\Programs\claudius\bin); the web studio
#                          goes to <that>\..\share\claudius\web, which is where claudius looks for it
#   CLAUDIUS_DOWNLOAD_URL  where the archives and SHA256SUMS are, an https:// address (default: the GitHub release)

[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidUsingWriteHost', '', Justification = 'an installer talks to the person running it')]
param(
    [switch]$Yes,
    [switch]$NoSetup
)

function Write-Step([string]$Message) {
    Write-Host "claudius: $Message"
}

# Adds a directory to the user's PATH in the registry, keeping %VARIABLES% in it unexpanded
function Add-UserPath([string]$Dir) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    try {
        $path = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $entries = @($path -split ';' | Where-Object { $_ })
        if ($entries -contains $Dir) {
            return $false
        }
        $key.SetValue('Path', ($entries + @($Dir)) -join ';', [Microsoft.Win32.RegistryValueKind]::ExpandString)
    } finally {
        $key.Close()
    }
    # setting any user variable through .NET tells Explorer, and the terminals it starts, that PATH changed
    [Environment]::SetEnvironmentVariable('CLAUDIUS_PATH_CHANGED', '1', 'User')
    [Environment]::SetEnvironmentVariable('CLAUDIUS_PATH_CHANGED', $null, 'User')
    return $true
}

function Install-Claudius([bool]$Yes, [bool]$NoSetup) {
    $ErrorActionPreference = 'Stop'
    # Invoke-WebRequest downloads many times slower while it draws a progress bar
    $ProgressPreference = 'SilentlyContinue'
    Set-StrictMode -Version 3
    # Windows PowerShell 5.1 does not offer TLS 1.2 unless asked
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'devlato/claudius'
    # Windows on Arm runs the x64 build
    if ($env:PROCESSOR_ARCHITECTURE -notin 'AMD64', 'ARM64') {
        throw "there is no build for $env:PROCESSOR_ARCHITECTURE Windows; build from source: https://github.com/$repo"
    }
    $target = 'x86_64-pc-windows-msvc'

    $version = "$env:CLAUDIUS_VERSION" -replace '^v', ''
    if ($env:CLAUDIUS_DOWNLOAD_URL) {
        $base = $env:CLAUDIUS_DOWNLOAD_URL.TrimEnd('/')
        if (-not $base.StartsWith('https://')) {
            throw 'CLAUDIUS_DOWNLOAD_URL has to be an https:// address'
        }
    } elseif ($version) {
        $base = "https://github.com/$repo/releases/download/v$version"
    } else {
        $base = "https://github.com/$repo/releases/latest/download"
    }
    if ($env:CLAUDIUS_INSTALL_DIR) {
        $binDir = [IO.Path]::GetFullPath($env:CLAUDIUS_INSTALL_DIR.TrimEnd('\', '/'))
    } else {
        $binDir = Join-Path $env:LOCALAPPDATA 'Programs\claudius\bin'
    }
    $webDir = Join-Path (Split-Path -Parent $binDir) 'share\claudius\web'

    $tmp = Join-Path ([IO.Path]::GetTempPath()) "claudius-$([Guid]::NewGuid())"
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $sums = Join-Path $tmp 'SHA256SUMS'
        Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums
        $archive = $null
        $pattern = '^([0-9a-fA-F]{64}) +\*?(claudius-(.+)-' + [regex]::Escape($target) + '\.zip)$'
        foreach ($line in Get-Content $sums) {
            if ($line -match $pattern) {
                $expected, $archive, $version = $Matches[1], $Matches[2], $Matches[3]
                break
            }
        }
        if (-not $archive) {
            throw "the release at $base has no build for $target; build from source: https://github.com/$repo"
        }

        Write-Step "downloading claudius $version for $target"
        $zip = Join-Path $tmp $archive
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile $zip
        $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash
        if ($actual -ne $expected) {
            throw "$archive does not match SHA256SUMS (expected $expected, got $actual); nothing was installed"
        }
        Write-Step 'checksum ok'

        Expand-Archive -Path $zip -DestinationPath $tmp
        $unpacked = Join-Path $tmp ($archive -replace '\.zip$', '')
        if (-not (Test-Path (Join-Path $unpacked 'bin\claudius.exe'))) {
            throw "$archive has no bin\claudius.exe"
        }

        New-Item -ItemType Directory -Force -Path $binDir, (Split-Path -Parent $webDir) | Out-Null
        foreach ($file in Get-ChildItem (Join-Path $unpacked 'bin')) {
            $dest = Join-Path $binDir $file.Name
            if (Test-Path $dest) {
                # a running claudius.exe cannot be overwritten, but it can be renamed out of the way
                Remove-Item -Force "$dest.old" -ErrorAction SilentlyContinue
                Move-Item -Force $dest "$dest.old"
            }
            Copy-Item $file.FullName $dest
        }
        if (Test-Path $webDir) {
            Remove-Item -Recurse -Force $webDir
        }
        Copy-Item -Recurse (Join-Path $unpacked 'share\claudius\web') $webDir
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    $exe = Join-Path $binDir 'claudius.exe'
    & $exe --version | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "claudius is installed in $binDir but does not start (exit code $LASTEXITCODE)"
    }

    Write-Step "installed claudius $version"
    # the Windows build has no ears for now (xtask/src/dist.rs, NO_EARS)
    Write-Step 'this build has no ears: ONNX Runtime for Windows needs DirectML, which is not verified yet, so verdicts use meters and music theory'
    Write-Host "  $binDir\claudius.exe"
    Write-Host "  $binDir\claudius-packs.exe"
    Write-Host "  $webDir (the web studio)"
    if (Add-UserPath $binDir) {
        $env:Path = "$binDir;$env:Path"
        Write-Step "added $binDir to your PATH; terminals opened from now on find claudius"
    }

    $run = $Yes
    if (-not $Yes -and -not $NoSetup -and [Environment]::UserInteractive -and -not [Console]::IsInputRedirected) {
        $answer = Read-Host 'claudius: download the sample packs now (about 2.2 GB)? [Y/n]'
        $run = $answer -notmatch '^[nN]'
    }
    if ($run -and -not $NoSetup) {
        & $exe setup --yes
    } else {
        Write-Host ''
        Write-Host 'Next: `claudius setup` downloads the sample packs (about 2.2 GB),'
        Write-Host '`claudius doctor` checks the installation, and `claudius studio` opens the studio.'
    }

    Write-Host ''
    Write-Host "To uninstall: Remove-Item '$binDir\claudius.exe', '$binDir\claudius-packs.exe'; Remove-Item -Recurse '$webDir'"
    Write-Host "and take $binDir out of your PATH (Settings, System, About, Advanced system settings, Environment Variables)."
    Write-Host 'What `claudius setup` downloaded is in the packs folder `claudius doctor` shows.'
}

try {
    Install-Claudius -Yes $Yes.IsPresent -NoSetup $NoSetup.IsPresent
} catch {
    Write-Host "claudius: error: $($_.Exception.Message)" -ForegroundColor Red
    # as a script file, fail the process; under `irm | iex`, leave the PowerShell session open
    if ($PSCommandPath) {
        exit 1
    }
}
