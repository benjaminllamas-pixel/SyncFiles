param(
    [Parameter(Mandatory = $true)][string]$OutDir,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$StageDir
)

$ErrorActionPreference = "Stop"
$compiler = $env:INNO_SETUP_COMPILER
if (-not $compiler) {
    $candidates = @(
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe"
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            $compiler = $candidate
            break
        }
    }
}
if (-not $compiler) {
    Write-Error "Inno Setup compiler not found. Set INNO_SETUP_COMPILER."
    exit 1
}

$iss = Join-Path $OutDir "syncfiles-$Version-windows-x86_64-installer.iss"
$stage = $StageDir.Replace('\', '\')
$escapedOut = $OutDir.Replace('\', '\')
$issContent = @"
[Setup]
AppId={{$Version.Replace('.', '-')}}
AppName=SyncFiles
AppVersion=$Version
DefaultDirName={autopf}\SyncFiles
DefaultGroupName=SyncFiles
OutputDir=$escapedOut
OutputBaseFilename=syncfiles-$Version-windows-x86_64-setup
Compression=lzma
SolidCompression=yes
PrivilegesRequired=standard
ArchitecturesInstallIn64BitMode=x64

[Files]
Source: "$stage\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\SyncFiles Server"; Filename: "{app}\syncfiles-server.exe"
Name: "{group}\SyncFiles Client"; Filename: "{app}\syncfiles-client.exe"
Name: "{group}\SyncFiles CLI"; Filename: "{app}\syncfiles-cli.exe"
"@
Set-Content -Path $iss -Value $issContent -Encoding UTF8
& $compiler $iss
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$setup = Join-Path $OutDir "syncfiles-$Version-windows-x86_64-setup.exe"
if ($env:SF_WINDOWS_PFX_BASE64 -and (Get-Command signtool -ErrorAction SilentlyContinue)) {
    $pfx = Join-Path ([System.IO.Path]::GetTempPath()) "syncfiles-windows.pfx"
    [System.IO.File]::WriteAllBytes($pfx, [System.Convert]::FromBase64String($env:SF_WINDOWS_PFX_BASE64))
    try {
        & signtool sign /fd SHA256 /f $pfx /p $env:SF_WINDOWS_CERT_PASSWORD $setup
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    } finally {
        Remove-Item -Force $pfx -ErrorAction SilentlyContinue
    }
}
