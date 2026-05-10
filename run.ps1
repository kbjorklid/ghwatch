# run.ps1 - Compile and run ghnotify-gemini

param(
    [switch]$Release,
    [switch]$BuildOnly,
    [switch]$SkipBuild
)

function Check-Command($cmd) {
    $path = Get-Command $cmd -ErrorAction SilentlyContinue
    if ($null -eq $path) {
        Write-Error "Error: '$cmd' is not installed or not in PATH."
        return $false
    }
    return $true
}

# 1. Check prerequisites
if (-not (Check-Command "cargo")) {
    Write-Host "Please install Rust: https://rustup.rs/" -ForegroundColor Yellow
    exit 1
}

if (-not (Check-Command "gh")) {
    Write-Host "Please install GitHub CLI: https://cli.github.com/" -ForegroundColor Yellow
    exit 1
}

# Check if gh is authenticated
$authStatus = gh auth status 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "GitHub CLI is not authenticated. Please run 'gh auth login'." -ForegroundColor Yellow
    exit 1
}

# 2. Build the application
if (-not $SkipBuild) {
    $buildArgs = @("build")
    if ($Release) {
        $buildArgs += "--release"
    }

    Write-Host "Building ghnotify-gemini..." -ForegroundColor Cyan
    cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Build failed."
        exit $LASTEXITCODE
    }
}

if ($BuildOnly) {
    Write-Host "Build successful." -ForegroundColor Green
    exit 0
}

# 3. Run the application
$runArgs = @("run")
if ($Release) {
    $runArgs += "--release"
}

Write-Host "Starting ghnotify-gemini..." -ForegroundColor Cyan
cargo @runArgs
