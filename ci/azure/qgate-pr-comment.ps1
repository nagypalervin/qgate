<#
.SYNOPSIS
Posts a qgate Markdown report to an Azure Repos pull request as one
comment thread, and keeps that thread up to date.

.DESCRIPTION
Run it in an Azure Pipelines pull request build, after

    qgate check --base origin/<target> --format markdown > qgate.md

with the same --fail-on as the step that gates the build, so the comment
names that level. That check exits 1 when a finding is at or above the
level; the step writing qgate.md should accept 1 and fail only on 2.

The script finds the pull request's qgate thread by its `qgate.report`
property, then:

- findings, no thread:      creates one (active);
- findings, a thread:       replaces its comment and sets it active;
- no findings, a thread:    replaces its comment ("No regressions found.")
                            and closes it;
- no findings, no thread:   writes nothing.

It reads the pipeline's predefined variables from the environment and
needs `System.AccessToken` passed explicitly:

    env:
      SYSTEM_ACCESSTOKEN: $(System.AccessToken)

The project's Build Service needs the "Contribute to pull requests"
permission on the repository. A failure is reported as one build error
and the script exits 1. Part of qgate (https://github.com/nagypalervin/qgate);
see design D4 of the OpenSpec change add-pr-comments.

.PARAMETER Report
The output of `qgate check --format markdown`.
#>
# After the help block: a line before it hides the help from Get-Help.
#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $Report
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

# The thread property that marks qgate's thread; its value versions the
# thread's layout.
$script:QgateProperty = 'qgate.report'
$script:QgateLayout = '1'
$script:ApiVersion = '7.1'
# CommentThreadStatus values of the Azure DevOps REST API.
$script:Active = 1
$script:Closed = 4

function Read-QgateReport {
    <# The counts from the marker line, and the document without it. #>
    param([Parameter(Mandatory)] [string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "qgate: the report $Path does not exist; write it with qgate check --format markdown"
    }
    $text = Get-Content -LiteralPath $Path -Raw
    $marker = '^<!-- qgate count=(\d+) failing=(\d+) fail_on=([a-z]+) -->\r?\n'
    if ($null -eq $text -or $text -notmatch $marker) {
        throw "qgate: $Path is not a qgate Markdown report (its first line must be the <!-- qgate count=... --> marker of qgate check --format markdown)"
    }
    [pscustomobject]@{
        Count   = [int] $Matches[1]
        Failing = [int] $Matches[2]
        FailOn  = $Matches[3]
        Body    = ($text -replace $marker, '').TrimStart("`r", "`n")
    }
}

function Get-PipelineContext {
    <# The threads URL of this pull request and the token, from the environment. #>
    $names = 'SYSTEM_COLLECTIONURI', 'SYSTEM_TEAMPROJECT', 'BUILD_REPOSITORY_ID', 'SYSTEM_PULLREQUEST_PULLREQUESTID'
    $values = @{}
    foreach ($name in $names) {
        $value = [Environment]::GetEnvironmentVariable($name)
        if ([string]::IsNullOrEmpty($value)) {
            throw "qgate: the variable $name is not set; run this script in an Azure Pipelines pull request build"
        }
        $values[$name] = $value
    }
    $token = $env:SYSTEM_ACCESSTOKEN
    # An unmapped macro reaches the script as its own text.
    if ([string]::IsNullOrEmpty($token) -or $token -eq '$(System.AccessToken)') {
        throw "qgate: SYSTEM_ACCESSTOKEN is not set; add to this step:`n  env:`n    SYSTEM_ACCESSTOKEN: `$(System.AccessToken)"
    }

    $collection = $values.SYSTEM_COLLECTIONURI
    if (-not $collection.EndsWith('/')) { $collection += '/' }
    $project = [uri]::EscapeDataString($values.SYSTEM_TEAMPROJECT)
    $repository = [uri]::EscapeDataString($values.BUILD_REPOSITORY_ID)
    $pullRequest = [uri]::EscapeDataString($values.SYSTEM_PULLREQUEST_PULLREQUESTID)
    [pscustomobject]@{
        ThreadsUrl = "${collection}${project}/_apis/git/repositories/${repository}/pullRequests/${pullRequest}/threads"
        Token      = $token
    }
}

function Get-AzureErrorMessage {
    <# A message for a failed REST call; StatusCode 0 means no answer. #>
    param(
        [string] $Method,
        [string] $Uri,
        [int] $StatusCode,
        [string] $Body
    )

    # Azure DevOps answers with a JSON document; its message is the readable
    # part, and only decoding it turns the escaped quotes back into quotes.
    try {
        $document = $Body | ConvertFrom-Json -ErrorAction Stop
        $message = $document.PSObject.Properties['message']
        if ($null -ne $message -and $message.Value -is [string]) { $Body = $message.Value }
    } catch {
        Write-Debug "qgate: the answer is not JSON: $_"
    }

    if ($StatusCode -eq 403 -and $Body -match 'TF401027') {
        $identity = if ($Body -match "identity '([^']+)'") { " ($($Matches[1]))" } else { '' }
        return "qgate: the build identity$identity may not comment on pull requests. " +
        "Give the project's Build Service the 'Contribute to pull requests' permission on this repository " +
        "(Project settings > Repositories > <repository> > Security)."
    }
    if ($StatusCode -eq 401) {
        return "qgate: Azure DevOps refused the token (401). Pass System.AccessToken to this step " +
        "(env: SYSTEM_ACCESSTOKEN: `$(System.AccessToken)); it expires when the job ends."
    }
    if ($StatusCode -eq 0) {
        return "qgate: no answer from Azure DevOps to $Method ${Uri}: $Body"
    }
    "qgate: Azure DevOps answered $StatusCode to $Method ${Uri}: $Body"
}

function Invoke-Azure {
    <# One REST call with the build's token; a failure throws a readable message. #>
    param(
        [Parameter(Mandatory)] $Context,
        [Parameter(Mandatory)] [string] $Method,
        [Parameter(Mandatory)] [string] $Uri,
        $Body
    )

    $request = @{
        Method  = $Method
        Uri     = $Uri
        Headers = @{ Authorization = "Bearer $($Context.Token)" }
    }
    if ($null -ne $Body) {
        $request.ContentType = 'application/json'
        $request.Body = $Body | ConvertTo-Json -Depth 10 -Compress
    }
    try {
        Invoke-RestMethod @request
    } catch {
        $status = 0
        $response = $_.Exception.PSObject.Properties['Response']
        if ($null -ne $response -and $null -ne $response.Value) {
            $status = [int] $response.Value.StatusCode
        }
        $detail = if ($_.ErrorDetails -and $_.ErrorDetails.Message) { $_.ErrorDetails.Message } else { $_.Exception.Message }
        throw (Get-AzureErrorMessage -Method $Method.ToUpperInvariant() -Uri $Uri -StatusCode $status -Body $detail)
    }
}

function Test-QgateThread {
    <# Whether a thread is qgate's: not deleted, with the qgate property. #>
    param([Parameter(Mandatory)] $Thread)

    $deleted = $Thread.PSObject.Properties['isDeleted']
    if ($null -ne $deleted -and $deleted.Value) { return $false }
    $properties = $Thread.PSObject.Properties['properties']
    if ($null -eq $properties -or $null -eq $properties.Value) { return $false }
    $null -ne $properties.Value.PSObject.Properties[$script:QgateProperty]
}

function Sync-QgateComment {
    <#
    Brings the pull request's qgate thread in line with the report and
    returns what it did: created, updated, closed or none.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)] $Context,
        [Parameter(Mandatory)] $Report
    )

    $url = $Context.ThreadsUrl
    $query = "?api-version=$($script:ApiVersion)"
    $answer = Invoke-Azure -Context $Context -Method Get -Uri "${url}${query}"
    $list = if ($null -ne $answer) { $answer.PSObject.Properties['value'] } else { $null }
    if ($null -eq $list) {
        throw "qgate: Azure DevOps did not answer with a thread list to GET ${url}${query}"
    }
    $mine = @(@($list.Value) | Where-Object { $null -ne $_ -and (Test-QgateThread $_) } | Sort-Object { [int] $_.id })
    if ($mine.Count -gt 1) {
        $others = ($mine | Select-Object -Skip 1 | ForEach-Object { $_.id }) -join ', '
        Write-Warning "qgate: more than one qgate thread; updating thread $($mine[0].id), leaving $others"
    }

    if ($mine.Count -eq 0) {
        if ($Report.Count -eq 0) { return 'none' }
        $thread = @{
            comments   = @(@{ parentCommentId = 0; commentType = 1; content = $Report.Body })
            status     = $script:Active
            properties = @{ $script:QgateProperty = @{ '$type' = 'System.String'; '$value' = $script:QgateLayout } }
        }
        Invoke-Azure -Context $Context -Method Post -Uri "${url}${query}" -Body $thread | Out-Null
        return 'created'
    }

    $thread = $mine[0]
    # The first comment is qgate's; later ones are replies. Ids are not
    # assumed to start at 1.
    $comments = $thread.PSObject.Properties['comments']
    $comment = if ($null -ne $comments) {
        @($comments.Value) | Where-Object { $null -ne $_ } | Sort-Object { [int] $_.id } | Select-Object -First 1
    }
    if ($null -eq $comment) {
        throw "qgate: the qgate thread $($thread.id) has no comment to update; Azure DevOps answered in a shape this script does not know"
    }
    Invoke-Azure -Context $Context -Method Patch -Uri "${url}/$($thread.id)/comments/$($comment.id)${query}" `
        -Body @{ content = $Report.Body } | Out-Null
    $status = if ($Report.Count -gt 0) { $script:Active } else { $script:Closed }
    Invoke-Azure -Context $Context -Method Patch -Uri "${url}/$($thread.id)${query}" -Body @{ status = $status } | Out-Null
    if ($Report.Count -gt 0) { 'updated' } else { 'closed' }
}

function Invoke-QgatePrComment {
    <#
    The script's work. Says what it did and returns 0, or reports the
    failure as one build error (a logging command, so the message is
    neither wrapped nor buried in a stack excerpt) and returns 1.
    #>
    param([Parameter(Mandatory)] [string] $Report)

    try {
        $parsed = Read-QgateReport -Path $Report
        $context = Get-PipelineContext
        $done = Sync-QgateComment -Context $context -Report $parsed
        $what = switch ($done) {
            'created' { 'created the qgate comment' }
            'updated' { 'updated the qgate comment' }
            'closed' { 'updated and closed the qgate comment' }
            'none' { 'no findings and no qgate comment; nothing to write' }
        }
        $noun = if ($parsed.Count -eq 1) { 'finding' } else { 'findings' }
        Write-Host "qgate: $what ($($parsed.Count) $noun, $($parsed.Failing) at or above $($parsed.FailOn))"
        0
    } catch {
        # A logging command's message escapes %, CR and LF.
        $message = $_.Exception.Message.Replace('%', '%AZP25').Replace("`r", '%0D').Replace("`n", '%0A')
        Write-Host "##vso[task.logissue type=error]$message"
        1
    }
}

# Run unless dot-sourced (the tests load the functions that way).
if ($MyInvocation.InvocationName -ne '.') {
    exit (Invoke-QgatePrComment -Report $Report)
}
