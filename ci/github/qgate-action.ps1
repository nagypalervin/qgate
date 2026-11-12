<#
.SYNOPSIS
Runs one step of the qgate GitHub Action: Install, Report, Comment or Gate.

.DESCRIPTION
action.yml at the repository root runs this script four times, once per
step, with the action's inputs in the environment:

    QGATE_VERSION        the qgate release to install (action.yml's own)
    QGATE_INPUT_BASE     the base input; empty means origin/<base branch>
    QGATE_INPUT_FAIL_ON  the fail-on input
    GITHUB_TOKEN         the job's token (Comment only)

- Install: resolves the base, downloads the release archive for this
  runner and SHA256SUMS, checks the archive and unpacks it under
  RUNNER_TEMP.
- Report:  qgate check --format markdown; appends the report to the job
  summary. Exit codes 0 and 1 are both fine here.
- Comment: keeps one qgate comment on the pull request up to date; skips
  forks and Dependabot, whose token cannot write.
- Gate:    qgate check --format github; prints the annotations and exits
  with qgate's exit code.

Every step recomputes the base and the binary's path instead of passing
them through GITHUB_ENV, so the job's environment stays as it was. Text
that comes from the pull request (qgate's standard error) is printed
between ::stop-commands:: and its end token, so the runner reads no
workflow command in it. A failure is reported as one ::error:: and the
step exits 1 (Gate: qgate's exit code). Part of qgate
(https://github.com/nagypalervin/qgate); see design D2 to D6 of the
OpenSpec change add-github-action.

.PARAMETER Step
Install, Report, Comment or Gate.
#>
# After the help block: a line before it hides the help from Get-Help.
#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Install', 'Report', 'Comment', 'Gate')]
    [string] $Step
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Exit codes are read from $LASTEXITCODE; 1 from qgate is not an error.
$PSNativeCommandUseErrorActionPreference = $false

$script:ReleaseUrl = 'https://github.com/nagypalervin/qgate/releases/download'
$script:ApiVersion = '2022-11-28'
$script:Bot = 'github-actions[bot]'
$script:Marker = '<!-- qgate '
# A guard against a Link header that never ends: 100 pages of 100 comments.
$script:MaxPages = 100

function Get-QgateVersion {
    <# The version to install, from action.yml; only x.y.z is accepted. #>
    $version = $env:QGATE_VERSION
    if ($version -notmatch '^\d+\.\d+\.\d+$') {
        throw "qgate: QGATE_VERSION is '$version', not a version like 0.3.0; it comes from the action's action.yml"
    }
    $version
}

function Get-QgatePath {
    <# The action's directory under RUNNER_TEMP, or a file in it. #>
    param([string] $Name)

    if ([string]::IsNullOrEmpty($env:RUNNER_TEMP)) {
        throw 'qgate: RUNNER_TEMP is not set; run this script as a step of the qgate GitHub Action'
    }
    $dir = Join-Path $env:RUNNER_TEMP "qgate-$(Get-QgateVersion)"
    if ($Name) { Join-Path $dir $Name } else { $dir }
}

function Get-QgateTarget {
    <# The release target for a runner's RUNNER_OS and RUNNER_ARCH. #>
    param(
        [string] $Os,
        [string] $Arch
    )

    $system = @{ Linux = 'unknown-linux-musl'; macOS = 'apple-darwin'; Windows = 'pc-windows-msvc' }[$Os]
    $cpu = @{ X64 = 'x86_64'; ARM64 = 'aarch64' }[$Arch]
    if ($null -eq $system -or $null -eq $cpu) {
        throw "qgate: no qgate release archive for this runner ($Os $Arch); releases exist for Linux, macOS and Windows on X64 and ARM64"
    }
    "$cpu-$system"
}

function Get-QgateBinary {
    <# Where Install puts the qgate binary for this runner. #>
    $version = Get-QgateVersion
    $target = Get-QgateTarget -Os $env:RUNNER_OS -Arch $env:RUNNER_ARCH
    $exe = if ($target -like '*-windows-*') { 'qgate.exe' } else { 'qgate' }
    Get-QgatePath -Name (Join-Path "qgate-$version-$target" $exe)
}

function Resolve-QgateBase {
    <# The revision to compare with: the base input, else the pull request's base branch. #>
    param(
        [string] $EventName,
        [string] $BaseRef,
        [string] $BaseInput
    )

    $base = if ($BaseInput) {
        $BaseInput
    } elseif ($EventName -eq 'pull_request' -and $BaseRef) {
        "origin/$BaseRef"
    } else {
        throw "qgate: no base revision to compare with; run the action on a pull_request event, or set its base input (for example base: origin/main)"
    }
    if ($base -match '[\r\n]') {
        throw 'qgate: the base input has a line break; give one revision'
    }
    $base
}

function Test-QgateChecksum {
    <# Throws unless the archive's SHA-256 matches its line in SHA256SUMS. #>
    param(
        [Parameter(Mandatory)] [string] $Archive,
        [Parameter(Mandatory)] [string] $Sums
    )

    $name = Split-Path -Leaf $Archive
    $want = $null
    foreach ($line in Get-Content -LiteralPath $Sums) {
        if ($line -match '^([0-9a-f]{64})  (.+)$' -and $Matches[2] -ceq $name) {
            $want = $Matches[1]
            break
        }
    }
    if ($null -eq $want) {
        throw "qgate: SHA256SUMS has no line for $name; not installing it"
    }
    $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $Archive).Hash.ToLowerInvariant()
    if ($got -cne $want) {
        throw "qgate: $name does not match its line in SHA256SUMS (SHA-256 $got); not installing it"
    }
}

function Expand-QgateArchive {
    <# Unpacks a release archive: .zip on Windows, .tar.gz elsewhere. #>
    param(
        [Parameter(Mandatory)] [string] $Archive,
        [Parameter(Mandatory)] [string] $Destination
    )

    if ($Archive.EndsWith('.zip')) {
        Expand-Archive -LiteralPath $Archive -DestinationPath $Destination -Force
    } else {
        tar -xzf $Archive -C $Destination
        if ($LASTEXITCODE -ne 0) {
            throw "qgate: tar could not unpack $(Split-Path -Leaf $Archive) (exit code $LASTEXITCODE)"
        }
    }
}

function Invoke-QgateInstall {
    <# The Install step. #>
    $base = Resolve-QgateBase -EventName $env:GITHUB_EVENT_NAME -BaseRef $env:GITHUB_BASE_REF -BaseInput $env:QGATE_INPUT_BASE
    $version = Get-QgateVersion
    $target = Get-QgateTarget -Os $env:RUNNER_OS -Arch $env:RUNNER_ARCH
    $extension = if ($target -like '*-windows-*') { 'zip' } else { 'tar.gz' }
    $name = "qgate-$version-$target.$extension"

    $dir = Get-QgatePath
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $archive = Join-Path $dir $name
    $sums = Join-Path $dir 'SHA256SUMS'
    $url = "$($script:ReleaseUrl)/v$version"
    Invoke-WebRequest -Uri "$url/$name" -OutFile $archive -MaximumRetryCount 3 -RetryIntervalSec 5
    Invoke-WebRequest -Uri "$url/SHA256SUMS" -OutFile $sums -MaximumRetryCount 3 -RetryIntervalSec 5
    Test-QgateChecksum -Archive $archive -Sums $sums
    Expand-QgateArchive -Archive $archive -Destination $dir

    $binary = Get-QgateBinary
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        throw "qgate: $name did not contain $(Split-Path -Leaf $binary)"
    }
    Write-Host "qgate: installed qgate $version ($target); comparing with $base"
}

function Invoke-Qgate {
    <# Runs qgate with its standard output and error in files; returns the exit code. #>
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $StdoutPath,
        [Parameter(Mandatory)] [string] $StderrPath
    )

    # PowerShell 7.4 and later keep a native command's bytes with `>`.
    & (Get-QgateBinary) @Arguments > $StdoutPath 2> $StderrPath
    $LASTEXITCODE
}

function Invoke-QgateCheck {
    <# Runs qgate check against the base in one format; returns the exit code and file paths. #>
    param(
        [Parameter(Mandatory)] [string] $Format,
        [Parameter(Mandatory)] [string] $Stdout,
        [Parameter(Mandatory)] [string] $Stderr
    )

    $base = Resolve-QgateBase -EventName $env:GITHUB_EVENT_NAME -BaseRef $env:GITHUB_BASE_REF -BaseInput $env:QGATE_INPUT_BASE
    $failOn = if ($env:QGATE_INPUT_FAIL_ON) { $env:QGATE_INPUT_FAIL_ON } else { 'error' }
    New-Item -ItemType Directory -Force -Path (Get-QgatePath) | Out-Null
    $out = Get-QgatePath -Name $Stdout
    $err = Get-QgatePath -Name $Stderr
    $arguments = @('check', '--base', $base, '--format', $Format, '--fail-on', $failOn)
    $code = Invoke-Qgate -Arguments $arguments -StdoutPath $out -StderrPath $err
    [pscustomobject]@{ Code = [int] $code; Stdout = $out; Stderr = $err }
}

function Write-Untrusted {
    <# Prints text from the pull request so that the runner reads no workflow command in it. #>
    param([AllowNull()] [AllowEmptyString()] [string] $Text)

    if ([string]::IsNullOrWhiteSpace($Text)) { return }
    $token = [guid]::NewGuid().ToString('N')
    Write-Host "::stop-commands::$token"
    Write-Host $Text.TrimEnd("`r", "`n")
    Write-Host "::${token}::"
}

function Write-QgateFailure {
    <# The error line for a qgate that could not run. #>
    param([int] $Code)
    Write-Host "::error::qgate check could not run (exit code $Code); its message is printed above"
}

function Add-QgateSummary {
    <# Appends text to this step's job summary, on a line of its own. #>
    param([Parameter(Mandatory)] [string] $Text)

    $path = $env:GITHUB_STEP_SUMMARY
    if ([string]::IsNullOrEmpty($path)) {
        throw 'qgate: GITHUB_STEP_SUMMARY is not set; run this script as a step of the qgate GitHub Action'
    }
    $existing = if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path -Raw } else { $null }
    $prefix = if ($existing -and -not $existing.EndsWith("`n")) { "`n" } else { '' }
    [IO.File]::AppendAllText($path, $prefix + $Text, [Text.UTF8Encoding]::new($false))
}

function Invoke-QgateReport {
    <# The Report step; returns the step's exit code. #>
    $run = Invoke-QgateCheck -Format 'markdown' -Stdout 'report.md' -Stderr 'report.err'
    Write-Untrusted (Get-Content -LiteralPath $run.Stderr -Raw -ErrorAction SilentlyContinue)
    if ($run.Code -ne 0 -and $run.Code -ne 1) {
        Write-QgateFailure -Code $run.Code
        return 1
    }
    Add-QgateSummary -Text (Get-Content -LiteralPath $run.Stdout -Raw -Encoding utf8)
    Write-Host 'qgate: added the report to the job summary'
    0
}

function Write-QgateOutput {
    <# Copies a file's bytes to standard output unchanged, for the runner to read. #>
    param([Parameter(Mandatory)] [string] $Path)

    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -eq 0) { return }
    $stdout = [Console]::OpenStandardOutput()
    $stdout.Write($bytes, 0, $bytes.Length)
    $stdout.Flush()
}

function Invoke-QgateGate {
    <# The Gate step; returns qgate's exit code. #>
    $run = Invoke-QgateCheck -Format 'github' -Stdout 'gate.out' -Stderr 'gate.err'
    Write-QgateOutput -Path $run.Stdout
    Write-Untrusted (Get-Content -LiteralPath $run.Stderr -Raw -ErrorAction SilentlyContinue)
    if ($run.Code -ne 0 -and $run.Code -ne 1) { Write-QgateFailure -Code $run.Code }
    $run.Code
}

function Read-QgateReport {
    <# The counts from the marker line, and the whole report as the comment's body. #>
    param([Parameter(Mandatory)] [string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "qgate: the report $Path does not exist; the Report step writes it"
    }
    $text = Get-Content -LiteralPath $Path -Raw -Encoding utf8
    $marker = '^<!-- qgate count=(\d+) failing=(\d+) fail_on=([a-z]+) -->\r?\n'
    if ($null -eq $text -or $text -notmatch $marker) {
        throw "qgate: $Path is not a qgate Markdown report (its first line must be the <!-- qgate count=... --> marker of qgate check --format markdown)"
    }
    # The marker stays in the body: it marks the comment as qgate's.
    [pscustomobject]@{
        Count   = [int] $Matches[1]
        Failing = [int] $Matches[2]
        FailOn  = $Matches[3]
        Body    = $text
    }
}

function Get-GitHubErrorMessage {
    <# A message for a failed REST call. #>
    param(
        [string] $Method,
        [string] $Uri,
        [int] $StatusCode,
        [string] $Content
    )

    $message = $Content
    try {
        $document = $Content | ConvertFrom-Json -ErrorAction Stop
        $field = $document.PSObject.Properties['message']
        if ($null -ne $field -and $field.Value -is [string]) { $message = $field.Value }
    } catch {
        Write-Debug "qgate: the answer is not JSON: $_"
    }

    if ($StatusCode -eq 403 -or $StatusCode -eq 404) {
        return "qgate: the job's GITHUB_TOKEN may not comment on this pull request " +
        "(GitHub answered $StatusCode to $Method ${Uri}: $message). " +
        "Grant it in the workflow or the job:`n  permissions:`n    contents: read`n    pull-requests: write"
    }
    if ($StatusCode -eq 401) {
        return "qgate: GitHub refused the job's token (401 to $Method ${Uri}: $message)"
    }
    "qgate: GitHub answered $StatusCode to $Method ${Uri}: $message"
}

function Invoke-GitHub {
    <# One REST call with the job's token; returns the decoded body and the next page's URL. #>
    param(
        [Parameter(Mandatory)] $Context,
        [Parameter(Mandatory)] [string] $Method,
        [Parameter(Mandatory)] [string] $Uri,
        $Body
    )

    $request = @{
        Method             = $Method
        Uri                = $Uri
        Headers            = @{
            Authorization          = "Bearer $($Context.Token)"
            Accept                 = 'application/vnd.github+json'
            'X-GitHub-Api-Version' = $script:ApiVersion
        }
        SkipHttpErrorCheck = $true
    }
    if ($null -ne $Body) {
        $request.ContentType = 'application/json'
        $request.Body = $Body | ConvertTo-Json -Depth 10 -Compress
    }
    $verb = $Method.ToUpperInvariant()
    try {
        $response = Invoke-WebRequest @request
    } catch {
        throw "qgate: no answer from GitHub to $verb ${Uri}: $($_.Exception.Message)"
    }
    $status = [int] $response.StatusCode
    if ($status -ge 300) {
        throw (Get-GitHubErrorMessage -Method $verb -Uri $Uri -StatusCode $status -Content $response.Content)
    }

    $next = $null
    if ($response.Headers -and $response.Headers.ContainsKey('Link')) {
        foreach ($link in @($response.Headers['Link'])) {
            if ($link -match '<([^>]+)>;\s*rel="next"') { $next = $Matches[1] }
        }
    }
    $decoded = if ($response.Content) { $response.Content | ConvertFrom-Json } else { $null }
    [pscustomobject]@{ Body = $decoded; Next = $next }
}

function Test-QgateComment {
    <# Whether a comment is qgate's: written by github-actions[bot], starting with the marker. #>
    param([Parameter(Mandatory)] $Comment)

    $user = $Comment.PSObject.Properties['user']
    $body = $Comment.PSObject.Properties['body']
    if ($null -eq $user -or $null -eq $user.Value -or $null -eq $body -or $body.Value -isnot [string]) { return $false }
    $login = $user.Value.PSObject.Properties['login']
    $null -ne $login -and $login.Value -eq $script:Bot -and $body.Value.StartsWith($script:Marker)
}

function Sync-QgateComment {
    <#
    Brings the pull request's qgate comment in line with the report and
    returns what it did: created, updated, cleared or none.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)] $Context,
        [Parameter(Mandatory)] $Report
    )

    $comments = [System.Collections.Generic.List[object]]::new()
    $uri = "$($Context.Url)/issues/$($Context.Number)/comments?per_page=100"
    for ($page = 0; $uri -and $page -lt $script:MaxPages; $page++) {
        $answer = Invoke-GitHub -Context $Context -Method Get -Uri $uri
        foreach ($comment in @($answer.Body)) {
            if ($null -ne $comment) { $comments.Add($comment) }
        }
        $uri = $answer.Next
    }
    $mine = @($comments | Where-Object { Test-QgateComment $_ } | Sort-Object { [long] $_.id })
    if ($mine.Count -gt 1) {
        $others = ($mine | Select-Object -Skip 1 | ForEach-Object { $_.id }) -join ', '
        Write-Host "::warning::qgate: more than one qgate comment; updating comment $($mine[0].id), leaving $others"
    }

    if ($mine.Count -eq 0) {
        if ($Report.Count -eq 0) { return 'none' }
        Invoke-GitHub -Context $Context -Method Post -Uri "$($Context.Url)/issues/$($Context.Number)/comments" `
            -Body @{ body = $Report.Body } | Out-Null
        return 'created'
    }
    Invoke-GitHub -Context $Context -Method Patch -Uri "$($Context.Url)/issues/comments/$($mine[0].id)" `
        -Body @{ body = $Report.Body } | Out-Null
    if ($Report.Count -gt 0) { 'updated' } else { 'cleared' }
}

function Invoke-QgateComment {
    <# The Comment step; returns the step's exit code. #>
    if ($env:GITHUB_EVENT_NAME -ne 'pull_request') {
        Write-Host "qgate: not a pull_request event ($env:GITHUB_EVENT_NAME); no comment to write"
        return 0
    }
    $event = Get-Content -LiteralPath $env:GITHUB_EVENT_PATH -Raw | ConvertFrom-Json
    $pullRequest = $event.pull_request
    $head = $pullRequest.head.repo
    if ($null -eq $head -or $head.full_name -ne $env:GITHUB_REPOSITORY) {
        Write-Host ('::notice::qgate: the comment is skipped for pull requests from forks: their GITHUB_TOKEN ' +
            'cannot write. The annotations and the job summary show the findings.')
        return 0
    }
    if ($env:GITHUB_ACTOR -eq 'dependabot[bot]') {
        Write-Host ('::notice::qgate: the comment is skipped for pull requests opened by Dependabot: their ' +
            'GITHUB_TOKEN cannot write. The annotations and the job summary show the findings.')
        return 0
    }

    $report = Read-QgateReport -Path (Get-QgatePath -Name 'report.md')
    if ([string]::IsNullOrEmpty($env:GITHUB_TOKEN)) {
        throw 'qgate: GITHUB_TOKEN is not set; the action passes github.token to this step'
    }
    $context = [pscustomobject]@{
        Url    = "$($env:GITHUB_API_URL)/repos/$($env:GITHUB_REPOSITORY)"
        Number = [long] $pullRequest.number
        Token  = $env:GITHUB_TOKEN
    }
    $done = Sync-QgateComment -Context $context -Report $report
    $what = switch ($done) {
        'created' { 'created the qgate comment' }
        'updated' { 'updated the qgate comment' }
        'cleared' { 'updated the qgate comment: no regressions found' }
        'none' { 'no findings and no qgate comment; nothing to write' }
    }
    $noun = if ($report.Count -eq 1) { 'finding' } else { 'findings' }
    Write-Host "qgate: $what ($($report.Count) $noun, $($report.Failing) at or above $($report.FailOn))"
    0
}

function Invoke-QgateAction {
    <#
    Runs one step and returns its exit code. A failure is reported as one
    ::error:: line, so the message is neither wrapped nor buried in a
    stack excerpt.
    #>
    param([Parameter(Mandatory)] [string] $Step)

    try {
        switch ($Step) {
            'Install' { Invoke-QgateInstall; 0 }
            'Report' { Invoke-QgateReport }
            'Comment' { Invoke-QgateComment }
            'Gate' { Invoke-QgateGate }
            default { throw "qgate: unknown step $Step" }
        }
    } catch {
        # A workflow command's message escapes %, CR and LF.
        $message = $_.Exception.Message.Replace('%', '%25').Replace("`r", '%0D').Replace("`n", '%0A')
        Write-Host "::error::$message"
        1
    }
}

# Run unless dot-sourced (the tests load the functions that way).
if ($MyInvocation.InvocationName -ne '.') {
    exit (Invoke-QgateAction -Step $Step)
}
