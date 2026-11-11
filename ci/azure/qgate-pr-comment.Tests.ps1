#Requires -Version 7.4
#Requires -Modules @{ ModuleName = 'Pester'; RequiredVersion = '5.9.1' }

# Tests for qgate-pr-comment.ps1 (design D4 and D5 of the OpenSpec change
# add-pr-comments). Invoke-RestMethod is mocked; the script's real calls
# are rehearsed against Azure DevOps (task 1.8).

BeforeAll {
    . (Join-Path $PSScriptRoot 'qgate-pr-comment.ps1') -Report 'not-used-when-dot-sourced'

    $script:Variables = @{
        SYSTEM_COLLECTIONURI             = 'https://dev.azure.com/example/'
        SYSTEM_TEAMPROJECT               = 'My Project'
        BUILD_REPOSITORY_ID              = '11111111-2222-3333-4444-555555555555'
        SYSTEM_PULLREQUEST_PULLREQUESTID = '42'
        SYSTEM_ACCESSTOKEN               = 'secret-token'
    }
    $script:ThreadsUrl = 'https://dev.azure.com/example/My%20Project/_apis/git/repositories/11111111-2222-3333-4444-555555555555/pullRequests/42/threads'

    function Set-PipelineVariables {
        foreach ($name in $script:Variables.Keys) {
            Set-Item "env:$name" $script:Variables[$name]
        }
    }

    function Clear-PipelineVariables {
        foreach ($name in $script:Variables.Keys) {
            Remove-Item "env:$name" -ErrorAction SilentlyContinue
        }
    }

    function New-ReportFile([int] $Count, [int] $Failing = 0, [string] $FailOn = 'error') {
        $path = Join-Path $TestDrive "report-$Count-$Failing.md"
        $body = if ($Count -eq 0) {
            "### qgate`n`nNo regressions found.`n"
        } else {
            "### qgate`n`n$Count findings, $Failing at or above ``$FailOn``.`n`n| Severity | Location | Finding | Rule |`n|---|---|---|---|`n| error | a.ts:1 | m | r/x |`n"
        }
        Set-Content -LiteralPath $path -NoNewline -Value "<!-- qgate count=$Count failing=$Failing fail_on=$FailOn -->`n`n$body"
        $path
    }

    # A thread as the REST API returns it, after ConvertFrom-Json.
    function New-Thread([int] $Id, [switch] $Qgate, [int] $CommentId = 1, [switch] $Deleted, [string] $Status = 'active') {
        $properties = if ($Qgate) {
            [pscustomobject]@{ 'qgate.report' = [pscustomobject]@{ '$type' = 'System.String'; '$value' = '1' } }
        } else {
            [pscustomobject]@{ CodeReviewThreadType = [pscustomobject]@{ '$type' = 'System.String'; '$value' = 'RefUpdate' } }
        }
        [pscustomobject]@{
            id         = $Id
            status     = $Status
            isDeleted  = [bool] $Deleted
            properties = $properties
            comments   = @([pscustomobject]@{ id = $CommentId; content = 'old' })
        }
    }

    # Mock Invoke-RestMethod: record every call and answer GET with
    # $script:Existing.
    function Use-FakeApi([object[]] $Existing = @()) {
        $script:Existing = $Existing
        $script:Calls = [System.Collections.Generic.List[object]]::new()
        Mock Invoke-RestMethod {
            $script:Calls.Add([pscustomobject]@{
                    Method  = [string] $Method
                    # [uri]'s ToString() unescapes; keep what the script passed.
                    Uri     = if ($Uri -is [uri]) { $Uri.OriginalString } else { [string] $Uri }
                    Body    = if ($Body) { $Body | ConvertFrom-Json } else { $null }
                    Headers = $Headers
                })
            if ($Method -eq 'Get') { return [pscustomobject]@{ value = @($script:Existing); count = @($script:Existing).Count } }
            if ($Method -eq 'Post') { return [pscustomobject]@{ id = 99; status = 'active' } }
            return [pscustomobject]@{ id = 1 }
        }
    }
}

Describe 'Read-QgateReport' {
    It 'reads the counts and drops the marker line from the body' {
        $report = Read-QgateReport -Path (New-ReportFile -Count 2 -Failing 1)
        $report.Count | Should -Be 2
        $report.Failing | Should -Be 1
        $report.FailOn | Should -Be 'error'
        $report.Body | Should -BeLike '### qgate*'
        $report.Body | Should -Not -Match 'qgate count='
    }

    It 'refuses a file that is not a qgate Markdown report' {
        $path = Join-Path $TestDrive 'other.md'
        Set-Content -LiteralPath $path -Value '# something else'
        { Read-QgateReport -Path $path } | Should -Throw '*not a qgate Markdown report*'
    }

    It 'names a missing report file' {
        { Read-QgateReport -Path (Join-Path $TestDrive 'missing.md') } | Should -Throw '*missing.md*'
    }
}

Describe 'Get-PipelineContext' {
    BeforeEach { Set-PipelineVariables }
    AfterEach { Clear-PipelineVariables }

    It 'builds the threads URL with an escaped project name and without the ? trap' {
        $context = Get-PipelineContext
        $context.ThreadsUrl | Should -BeExactly $script:ThreadsUrl
        $context.Token | Should -BeExactly 'secret-token'
    }

    It 'adds the missing slash after the collection URI' {
        $env:SYSTEM_COLLECTIONURI = 'https://dev.azure.com/example'
        (Get-PipelineContext).ThreadsUrl | Should -BeExactly $script:ThreadsUrl
    }

    It 'names a missing variable' {
        Remove-Item env:SYSTEM_PULLREQUEST_PULLREQUESTID
        { Get-PipelineContext } | Should -Throw '*SYSTEM_PULLREQUEST_PULLREQUESTID*'
    }

    It 'says how to pass the token when it is missing' {
        Remove-Item env:SYSTEM_ACCESSTOKEN
        { Get-PipelineContext } | Should -Throw '*SYSTEM_ACCESSTOKEN: $(System.AccessToken)*'
    }

    It 'treats an unexpanded macro as a missing token' {
        $env:SYSTEM_ACCESSTOKEN = '$(System.AccessToken)'
        { Get-PipelineContext } | Should -Throw '*SYSTEM_ACCESSTOKEN: $(System.AccessToken)*'
    }
}

Describe 'Sync-QgateComment' {
    BeforeEach { Set-PipelineVariables; $script:Context = Get-PipelineContext }
    AfterEach { Clear-PipelineVariables }

    It 'creates an active thread with the property when there are findings and no thread' {
        Use-FakeApi -Existing @(New-Thread -Id 1)
        $report = Read-QgateReport -Path (New-ReportFile -Count 1 -Failing 1)

        Sync-QgateComment -Context $script:Context -Report $report | Should -Be 'created'

        $script:Calls.Count | Should -Be 2
        $script:Calls[0].Method | Should -Be 'Get'
        $script:Calls[0].Uri | Should -BeExactly "$($script:ThreadsUrl)?api-version=7.1"
        $script:Calls[0].Headers.Authorization | Should -BeExactly 'Bearer secret-token'
        $post = $script:Calls[1]
        $post.Method | Should -Be 'Post'
        $post.Uri | Should -BeExactly "$($script:ThreadsUrl)?api-version=7.1"
        $post.Body.status | Should -Be 1
        $post.Body.comments[0].content | Should -BeExactly $report.Body
        $post.Body.properties.'qgate.report'.'$value' | Should -Be '1'
    }

    It 'updates and reactivates an existing thread when there are findings' {
        Use-FakeApi -Existing @((New-Thread -Id 1), (New-Thread -Id 7 -Qgate -CommentId 3 -Status 'closed'))
        $report = Read-QgateReport -Path (New-ReportFile -Count 2 -Failing 1)

        Sync-QgateComment -Context $script:Context -Report $report | Should -Be 'updated'

        $script:Calls.Count | Should -Be 3
        $script:Calls[1].Method | Should -Be 'Patch'
        $script:Calls[1].Uri | Should -BeExactly "$($script:ThreadsUrl)/7/comments/3?api-version=7.1"
        $script:Calls[1].Body.content | Should -BeExactly $report.Body
        $script:Calls[2].Method | Should -Be 'Patch'
        $script:Calls[2].Uri | Should -BeExactly "$($script:ThreadsUrl)/7?api-version=7.1"
        $script:Calls[2].Body.status | Should -Be 1
    }

    It 'updates and closes an existing thread when there are no findings' {
        Use-FakeApi -Existing @(New-Thread -Id 7 -Qgate)
        $report = Read-QgateReport -Path (New-ReportFile -Count 0)

        Sync-QgateComment -Context $script:Context -Report $report | Should -Be 'closed'

        $script:Calls.Count | Should -Be 3
        $script:Calls[1].Body.content | Should -BeLike '*No regressions found.*'
        $script:Calls[2].Uri | Should -BeExactly "$($script:ThreadsUrl)/7?api-version=7.1"
        $script:Calls[2].Body.status | Should -Be 4
    }

    It 'writes nothing when there are no findings and no thread' {
        Use-FakeApi -Existing @(New-Thread -Id 1)
        $report = Read-QgateReport -Path (New-ReportFile -Count 0)

        Sync-QgateComment -Context $script:Context -Report $report | Should -Be 'none'

        $script:Calls.Count | Should -Be 1
        $script:Calls[0].Method | Should -Be 'Get'
    }

    It 'keeps the thread with the lowest id up to date and warns about the others' {
        Use-FakeApi -Existing @((New-Thread -Id 12 -Qgate), (New-Thread -Id 5 -Qgate -CommentId 2))
        $report = Read-QgateReport -Path (New-ReportFile -Count 1 -Failing 1)

        Sync-QgateComment -Context $script:Context -Report $report -WarningVariable warnings -WarningAction SilentlyContinue |
            Should -Be 'updated'

        $script:Calls[1].Uri | Should -BeExactly "$($script:ThreadsUrl)/5/comments/2?api-version=7.1"
        $warnings | Should -HaveCount 1
        "$warnings" | Should -BeLike '*12*'
    }

    It 'ignores a deleted qgate thread' {
        Use-FakeApi -Existing @(New-Thread -Id 7 -Qgate -Deleted)
        $report = Read-QgateReport -Path (New-ReportFile -Count 1 -Failing 1)

        Sync-QgateComment -Context $script:Context -Report $report | Should -Be 'created'
    }

    It 'handles a pull request without threads' {
        Use-FakeApi -Existing @()
        $report = Read-QgateReport -Path (New-ReportFile -Count 1 -Failing 1)

        Sync-QgateComment -Context $script:Context -Report $report | Should -Be 'created'
    }
}

Describe 'Get-AzureErrorMessage' {
    It 'names the permission and the identity for TF401027' {
        $body = '{"message":"TF401027: You need the Git ''PullRequestContribute'' permission to perform this action. Details: identity ''Build\\abc'', scope ''repository''."}'
        $message = Get-AzureErrorMessage -Method 'POST' -Uri 'https://x/threads' -StatusCode 403 -Body $body
        $message | Should -BeLike "*'Contribute to pull requests'*"
        $message | Should -BeLike '*Build Service*'
        $message | Should -BeLike '*Build\abc*'
    }

    It 'says the token was not passed or has expired for 401' {
        $message = Get-AzureErrorMessage -Method 'GET' -Uri 'https://x/threads' -StatusCode 401 -Body ''
        $message | Should -BeLike '*System.AccessToken*'
    }

    It 'reports any other answer with method, URL, status and body' {
        $message = Get-AzureErrorMessage -Method 'GET' -Uri 'https://x/threads' -StatusCode 500 -Body 'boom'
        $message | Should -BeLike '*GET https://x/threads*'
        $message | Should -BeLike '*500*'
        $message | Should -BeLike '*boom*'
    }

    It 'reports a failure without an answer' {
        $message = Get-AzureErrorMessage -Method 'GET' -Uri 'https://x/threads' -StatusCode 0 -Body 'name not resolved'
        $message | Should -BeLike '*no answer*'
        $message | Should -BeLike '*name not resolved*'
    }
}
