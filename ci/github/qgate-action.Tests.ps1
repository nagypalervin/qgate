#Requires -Version 7.4
#Requires -Modules @{ ModuleName = 'Pester'; RequiredVersion = '5.9.1' }

# Tests for qgate-action.ps1 (design D2 to D6 and D9 of the OpenSpec change
# add-github-action). Invoke-WebRequest and qgate are mocked; the real
# calls were tried in the spike and are rehearsed after the release.

BeforeAll {
    . (Join-Path $PSScriptRoot 'qgate-action.ps1') -Step Install

    $script:Token = 'secret-token-value'
    $script:Api = 'https://api.github.com/repos/owner/repo'

    # The environment a step of the action sees on a pull request.
    function Set-ActionEnvironment([string] $EventName = 'pull_request', [string] $Actor = 'someone') {
        $env:RUNNER_TEMP = Join-Path $TestDrive 'runner-temp'
        New-Item -ItemType Directory -Force $env:RUNNER_TEMP | Out-Null
        $env:RUNNER_OS = 'Linux'
        $env:RUNNER_ARCH = 'X64'
        $env:GITHUB_API_URL = 'https://api.github.com'
        $env:GITHUB_REPOSITORY = 'owner/repo'
        $env:GITHUB_EVENT_NAME = $EventName
        $env:GITHUB_BASE_REF = if ($EventName -eq 'pull_request') { 'main' } else { '' }
        $env:GITHUB_ACTOR = $Actor
        $env:GITHUB_STEP_SUMMARY = Join-Path $TestDrive 'summary.md'
        Set-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value 'earlier step' -NoNewline
        $env:GITHUB_TOKEN = $script:Token
        $env:QGATE_VERSION = '0.3.0'
        $env:QGATE_INPUT_BASE = ''
        $env:QGATE_INPUT_FAIL_ON = 'error'
        Set-PullRequestEvent -HeadRepository 'owner/repo'
    }

    function Clear-ActionEnvironment {
        foreach ($name in 'RUNNER_TEMP', 'RUNNER_OS', 'RUNNER_ARCH', 'GITHUB_API_URL', 'GITHUB_REPOSITORY',
            'GITHUB_EVENT_NAME', 'GITHUB_EVENT_PATH', 'GITHUB_BASE_REF', 'GITHUB_ACTOR', 'GITHUB_STEP_SUMMARY',
            'GITHUB_TOKEN', 'QGATE_VERSION', 'QGATE_INPUT_BASE', 'QGATE_INPUT_FAIL_ON') {
            Remove-Item "env:$name" -ErrorAction SilentlyContinue
        }
    }

    # The event payload of a pull request; -HeadRepository '' is a deleted fork.
    function Set-PullRequestEvent([string] $HeadRepository) {
        $repo = if ($HeadRepository) { @{ full_name = $HeadRepository } } else { $null }
        $path = Join-Path $TestDrive 'event.json'
        @{ pull_request = @{ number = 7; head = @{ repo = $repo } } } |
            ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $path
        $env:GITHUB_EVENT_PATH = $path
    }

    function New-Report([int] $Count, [int] $Failing = 0) {
        $body = if ($Count -eq 0) {
            "### qgate`n`nNo regressions found.`n"
        } else {
            "### qgate`n`n$Count findings, $Failing at or above ``error``.`n`n| Severity | Location | Finding | Rule |`n|---|---|---|---|`n| error | src/é.ts:1 | m | r/x |`n"
        }
        "<!-- qgate count=$Count failing=$Failing fail_on=error -->`n`n$body"
    }

    function Set-ReportFile([int] $Count, [int] $Failing = 0) {
        $path = Get-QgatePath -Name 'report.md'
        New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
        Set-Content -LiteralPath $path -NoNewline -Value (New-Report -Count $Count -Failing $Failing)
        $path
    }

    function New-Comment([long] $Id, [string] $Body, [string] $Login = 'github-actions[bot]') {
        @{ id = $Id; body = $Body; user = @{ login = $Login } }
    }

    # Mock Invoke-WebRequest for API calls: record each call and answer from
    # $script:Pages (lists of comments, one per page) or $script:Answers
    # (status by method, for failures).
    function Use-FakeApi([object[]] $Pages = @(, @()), [hashtable] $Answers = @{}) {
        $script:Pages = $Pages
        $script:Answers = $Answers
        $script:Calls = [System.Collections.Generic.List[object]]::new()
        Mock Invoke-WebRequest -ParameterFilter { -not $OutFile } {
            $uriText = if ($Uri -is [uri]) { $Uri.OriginalString } else { [string] $Uri }
            $script:Calls.Add([pscustomobject]@{
                    Method  = [string] $Method
                    Uri     = $uriText
                    Body    = if ($Body) { $Body | ConvertFrom-Json } else { $null }
                    Headers = $Headers
                })
            $answer = $script:Answers[[string] $Method]
            if ($answer -is [scriptblock]) { & $answer }
            if ($answer) {
                return [pscustomobject]@{ StatusCode = $answer.Status; Content = $answer.Content; Headers = @{} }
            }
            switch ([string] $Method) {
                'Get' {
                    $page = if ($uriText -match '[?&]page=(\d+)') { [int] $Matches[1] } else { 1 }
                    $headers = @{}
                    if ($page -lt $script:Pages.Count) {
                        $headers.Link = [string[]] @("<$($script:Api)/issues/7/comments?per_page=100&page=$($page + 1)>; rel=`"next`", <x>; rel=`"last`"")
                    }
                    [pscustomobject]@{
                        StatusCode = 200
                        Content    = ConvertTo-Json -InputObject @($script:Pages[$page - 1]) -Depth 5
                        Headers    = $headers
                    }
                }
                'Post' { [pscustomobject]@{ StatusCode = 201; Content = '{"id": 99}'; Headers = @{} } }
                default { [pscustomobject]@{ StatusCode = 200; Content = '{"id": 1}'; Headers = @{} } }
            }
        }
    }

    # Capture Write-Host, the only way the script writes to the log.
    function Use-FakeHost {
        $script:Log = [System.Collections.Generic.List[string]]::new()
        Mock Write-Host { $script:Log.Add([string] $Object) }
    }

    function Get-LogText { $script:Log -join "`n" }
}

Describe 'The script' {
    It 'has help that Get-Help finds' {
        $help = Get-Help (Join-Path $PSScriptRoot 'qgate-action.ps1')
        "$($help.Synopsis)" | Should -BeLike 'Runs one step of the qgate GitHub Action*'
    }
}

Describe 'Get-QgateTarget' {
    It 'maps <Os> <Arch> to <Target>' -ForEach @(
        @{ Os = 'Linux'; Arch = 'X64'; Target = 'x86_64-unknown-linux-musl' }
        @{ Os = 'Linux'; Arch = 'ARM64'; Target = 'aarch64-unknown-linux-musl' }
        @{ Os = 'macOS'; Arch = 'X64'; Target = 'x86_64-apple-darwin' }
        @{ Os = 'macOS'; Arch = 'ARM64'; Target = 'aarch64-apple-darwin' }
        @{ Os = 'Windows'; Arch = 'X64'; Target = 'x86_64-pc-windows-msvc' }
        @{ Os = 'Windows'; Arch = 'ARM64'; Target = 'aarch64-pc-windows-msvc' }
    ) {
        Get-QgateTarget -Os $Os -Arch $Arch | Should -BeExactly $Target
    }

    It 'names both values for a runner without a release archive' {
        { Get-QgateTarget -Os 'Linux' -Arch 'X86' } | Should -Throw '*Linux*X86*'
    }
}

Describe 'Resolve-QgateBase' {
    It 'uses the base input on any event' {
        Resolve-QgateBase -EventName 'push' -BaseRef '' -BaseInput 'origin/dev' | Should -BeExactly 'origin/dev'
        Resolve-QgateBase -EventName 'pull_request' -BaseRef 'main' -BaseInput 'origin/dev' | Should -BeExactly 'origin/dev'
    }

    It 'uses the base branch of a pull request' {
        Resolve-QgateBase -EventName 'pull_request' -BaseRef 'main' -BaseInput '' | Should -BeExactly 'origin/main'
    }

    It 'asks for a pull_request trigger or a base input otherwise' {
        { Resolve-QgateBase -EventName 'push' -BaseRef '' -BaseInput '' } | Should -Throw '*pull_request*base*'
    }

    It 'refuses a base with a line break' {
        { Resolve-QgateBase -EventName 'push' -BaseRef '' -BaseInput "main`nX=1" } | Should -Throw '*line break*'
    }
}

Describe 'Test-QgateChecksum' {
    BeforeEach {
        $script:Archive = Join-Path $TestDrive 'qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz'
        Set-Content -LiteralPath $script:Archive -Value 'archive bytes' -NoNewline
        $script:Hash = (Get-FileHash -Algorithm SHA256 $script:Archive).Hash.ToLowerInvariant()
        $script:Sums = Join-Path $TestDrive 'SHA256SUMS'
    }

    It 'accepts the archive whose line matches' {
        Set-Content -LiteralPath $script:Sums -Value @(
            "$('0' * 64)  qgate-0.3.0-aarch64-unknown-linux-musl.tar.gz",
            "$($script:Hash)  qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz")
        { Test-QgateChecksum -Archive $script:Archive -Sums $script:Sums } | Should -Not -Throw
    }

    It 'names the archive whose hash differs' {
        Set-Content -LiteralPath $script:Sums -Value "$('0' * 64)  qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz"
        { Test-QgateChecksum -Archive $script:Archive -Sums $script:Sums } |
            Should -Throw '*qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz*does not match*'
    }

    It 'names the archive that has no line' {
        Set-Content -LiteralPath $script:Sums -Value "$($script:Hash)  qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz.bak"
        { Test-QgateChecksum -Archive $script:Archive -Sums $script:Sums } |
            Should -Throw '*no line for qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz*'
    }
}

Describe 'Expand-QgateArchive' {
    BeforeEach {
        $script:Source = Join-Path $TestDrive 'src'
        $script:Inner = Join-Path $script:Source 'qgate-0.3.0-t'
        New-Item -ItemType Directory -Force $script:Inner | Out-Null
        Set-Content -LiteralPath (Join-Path $script:Inner 'qgate') -Value 'binary'
        $script:Out = Join-Path $TestDrive "out-$([guid]::NewGuid().ToString('N'))"
        New-Item -ItemType Directory -Force $script:Out | Out-Null
    }

    It 'unpacks a .tar.gz with tar' {
        $archive = Join-Path $TestDrive 'a.tar.gz'
        tar -czf $archive -C $script:Source 'qgate-0.3.0-t'
        Expand-QgateArchive -Archive $archive -Destination $script:Out
        Join-Path $script:Out 'qgate-0.3.0-t' 'qgate' | Should -Exist
    }

    It 'unpacks a .zip' {
        $archive = Join-Path $TestDrive 'a.zip'
        Compress-Archive -Path $script:Inner -DestinationPath $archive -Force
        Expand-QgateArchive -Archive $archive -Destination $script:Out
        Join-Path $script:Out 'qgate-0.3.0-t' 'qgate' | Should -Exist
    }

    It 'names an archive tar cannot unpack' {
        $archive = Join-Path $TestDrive 'broken.tar.gz'
        Set-Content -LiteralPath $archive -Value 'not an archive'
        { Expand-QgateArchive -Archive $archive -Destination $script:Out 2>$null } | Should -Throw '*broken.tar.gz*'
    }
}

Describe 'The Install step' {
    BeforeEach {
        Set-ActionEnvironment
        Use-FakeHost
        $script:Downloads = [System.Collections.Generic.List[object]]::new()
        Mock Invoke-WebRequest -ParameterFilter { $OutFile } {
            $script:Downloads.Add([pscustomobject]@{ Uri = [string] $Uri; Retries = $MaximumRetryCount })
            # SHA256SUMS holds the right line for the archive downloaded before it.
            if ($OutFile -like '*SHA256SUMS') {
                $hash = (Get-FileHash -Algorithm SHA256 $script:LastArchive).Hash.ToLowerInvariant()
                Set-Content -LiteralPath $OutFile -Value "$hash  $(Split-Path -Leaf $script:LastArchive)"
            } else {
                Set-Content -LiteralPath $OutFile -Value 'archive bytes' -NoNewline
                $script:LastArchive = $OutFile
            }
        }
        Mock Expand-QgateArchive {
            $dir = Join-Path $Destination "qgate-0.3.0-$($env:FAKE_TARGET)"
            New-Item -ItemType Directory -Force $dir | Out-Null
            Set-Content -LiteralPath (Join-Path $dir $env:FAKE_EXE) -Value 'binary'
        }
        $env:FAKE_TARGET = 'x86_64-unknown-linux-musl'
        $env:FAKE_EXE = 'qgate'
    }
    AfterEach { Clear-ActionEnvironment; Remove-Item env:FAKE_TARGET, env:FAKE_EXE -ErrorAction SilentlyContinue }

    It 'downloads the archive and SHA256SUMS of its version with retries, checks and unpacks it' {
        Invoke-QgateAction -Step Install | Should -Be 0
        $base = 'https://github.com/nagypalervin/qgate/releases/download/v0.3.0'
        $script:Downloads.Uri | Should -Be @("$base/qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz", "$base/SHA256SUMS")
        $script:Downloads.Retries | Should -Be @(3, 3)
        Should -Invoke Expand-QgateArchive -Times 1
        Get-QgateBinary | Should -Exist
        Get-LogText | Should -Match 'qgate 0\.3\.0 \(x86_64-unknown-linux-musl\).*origin/main'
    }

    It 'does not unpack an archive that fails the checksum' {
        Mock Test-QgateChecksum { throw 'qgate: qgate-0.3.0-x86_64-unknown-linux-musl.tar.gz does not match its line in SHA256SUMS' }
        Invoke-QgateAction -Step Install | Should -Be 1
        Should -Invoke Expand-QgateArchive -Times 0
        Get-LogText | Should -Match '^::error::qgate: qgate-0\.3\.0-x86_64-unknown-linux-musl\.tar\.gz does not match'
    }

    It 'fails before downloading without a base' {
        $env:GITHUB_EVENT_NAME = 'push'
        $env:GITHUB_BASE_REF = ''
        Invoke-QgateAction -Step Install | Should -Be 1
        $script:Downloads.Count | Should -Be 0
        Get-LogText | Should -Match '::error::.*pull_request.*base'
    }

    It 'refuses a version that is not x.y.z' {
        $env:QGATE_VERSION = '0.3.0/../x'
        Invoke-QgateAction -Step Install | Should -Be 1
        $script:Downloads.Count | Should -Be 0
    }

    It 'uses the zip archive and qgate.exe on Windows' {
        $env:RUNNER_OS = 'Windows'
        $env:FAKE_TARGET = 'x86_64-pc-windows-msvc'
        $env:FAKE_EXE = 'qgate.exe'
        Mock Test-QgateChecksum { }
        Invoke-QgateAction -Step Install | Should -Be 0
        $script:Downloads[0].Uri | Should -BeLike '*/qgate-0.3.0-x86_64-pc-windows-msvc.zip'
        Get-QgateBinary | Should -BeLike '*qgate.exe'
    }
}

Describe 'The Report step' {
    BeforeEach {
        Set-ActionEnvironment
        Use-FakeHost
        $script:QgateCalls = [System.Collections.Generic.List[object]]::new()
        $script:QgateExit = 1
        $script:QgateStderr = "qgate: failing: 1 finding at or above `"error`" (see --fail-on)`n"
        Mock Invoke-Qgate {
            $script:QgateCalls.Add($Arguments)
            Set-Content -LiteralPath $StdoutPath -NoNewline -Value (New-Report -Count 1 -Failing 1)
            Set-Content -LiteralPath $StderrPath -NoNewline -Value $script:QgateStderr
            $script:QgateExit
        }
    }
    AfterEach { Clear-ActionEnvironment }

    It 'writes the Markdown report and appends it to the job summary' {
        Invoke-QgateAction -Step Report | Should -Be 0
        $script:QgateCalls[0] | Should -Be @('check', '--base', 'origin/main', '--format', 'markdown', '--fail-on', 'error')
        $summary = Get-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Raw
        $summary | Should -BeLike "earlier step`n<!-- qgate count=1 failing=1*src/é.ts:1*"
    }

    It 'passes the fail-on input' {
        $env:QGATE_INPUT_FAIL_ON = 'warning'
        Invoke-QgateAction -Step Report | Should -Be 0
        $script:QgateCalls[0][-1] | Should -BeExactly 'warning'
    }

    It 'prints qgate''s standard error so that the runner reads no command in it' {
        $script:QgateStderr = "error: unknown revision ``x`n::error::injected"
        $script:QgateExit = 2
        Invoke-QgateAction -Step Report | Should -Be 1
        $script:Log[0] | Should -Match '^::stop-commands::[0-9a-f]{32}$'
        $token = $script:Log[0].Substring('::stop-commands::'.Length)
        $script:Log[1] | Should -BeLike '*::error::injected'
        $script:Log[2] | Should -BeExactly "::${token}::"
        $script:Log[3] | Should -Match '^::error::qgate check could not run \(exit code 2\)'
    }

    It 'accepts exit code 0' {
        $script:QgateExit = 0
        $script:QgateStderr = ''
        Invoke-QgateAction -Step Report | Should -Be 0
        $script:Log | Where-Object { $_ -like '::stop-commands::*' } | Should -BeNullOrEmpty
    }
}

Describe 'The Comment step' {
    BeforeEach {
        Set-ActionEnvironment
        Use-FakeHost
        Set-ReportFile -Count 1 -Failing 1 | Out-Null
    }
    AfterEach { Clear-ActionEnvironment }

    It 'creates the comment with the whole report when there are findings and no comment' {
        Use-FakeApi -Pages @(, @((New-Comment 1 'hello' 'someone')))
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Method | Should -Be @('Get', 'Post')
        $script:Calls[0].Uri | Should -BeExactly "$($script:Api)/issues/7/comments?per_page=100"
        $script:Calls[1].Uri | Should -BeExactly "$($script:Api)/issues/7/comments"
        $script:Calls[1].Body.body | Should -BeExactly (New-Report -Count 1 -Failing 1)
        Get-LogText | Should -Match 'created the qgate comment'
    }

    It 'sends the token, the API version and the media type' {
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 0
        $headers = $script:Calls[0].Headers
        $headers.Authorization | Should -BeExactly "Bearer $($script:Token)"
        $headers.Accept | Should -BeExactly 'application/vnd.github+json'
        $headers['X-GitHub-Api-Version'] | Should -BeExactly '2022-11-28'
    }

    It 'updates the existing qgate comment' {
        Use-FakeApi -Pages @(, @((New-Comment 5 'hello' 'someone'), (New-Comment 6 (New-Report -Count 2 -Failing 2))))
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Method | Should -Be @('Get', 'Patch')
        $script:Calls[1].Uri | Should -BeExactly "$($script:Api)/issues/comments/6"
        $script:Calls[1].Body.body | Should -BeExactly (New-Report -Count 1 -Failing 1)
    }

    It 'ignores a report pasted by someone else' {
        Use-FakeApi -Pages @(, @((New-Comment 5 (New-Report -Count 2) 'someone')))
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Method | Should -Be @('Get', 'Post')
    }

    It 'updates the first of two qgate comments and warns' {
        Use-FakeApi -Pages @(, @((New-Comment 6 (New-Report -Count 2)), (New-Comment 8 (New-Report -Count 2))))
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls[1].Uri | Should -BeExactly "$($script:Api)/issues/comments/6"
        Get-LogText | Should -Match '::warning::.*more than one qgate comment'
    }

    It 'follows the next link to find the comment' {
        Use-FakeApi -Pages @(@((New-Comment 1 'a' 'someone')), @((New-Comment 9 (New-Report -Count 2))))
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Method | Should -Be @('Get', 'Get', 'Patch')
        $script:Calls[1].Uri | Should -BeExactly "$($script:Api)/issues/7/comments?per_page=100&page=2"
        $script:Calls[2].Uri | Should -BeExactly "$($script:Api)/issues/comments/9"
    }

    It 'rewrites an existing comment when the findings are gone' {
        Set-ReportFile -Count 0 | Out-Null
        Use-FakeApi -Pages @(, @((New-Comment 6 (New-Report -Count 2))))
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Method | Should -Be @('Get', 'Patch')
        $script:Calls[1].Body.body | Should -BeLike '*No regressions found.*'
    }

    It 'writes nothing on a clean pull request without a comment' {
        Set-ReportFile -Count 0 | Out-Null
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Method | Should -Be @('Get')
        Get-LogText | Should -Match 'nothing to write'
    }

    It 'skips a pull request from a fork with a notice' {
        Set-PullRequestEvent -HeadRepository 'someone/fork'
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Count | Should -Be 0
        Get-LogText | Should -Match '^::notice::.*fork'
    }

    It 'skips a pull request from a deleted fork' {
        Set-PullRequestEvent -HeadRepository ''
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Count | Should -Be 0
        Get-LogText | Should -Match '::notice::.*fork'
    }

    It 'skips a pull request opened by Dependabot' {
        $env:GITHUB_ACTOR = 'dependabot[bot]'
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Count | Should -Be 0
        Get-LogText | Should -Match '::notice::.*Dependabot'
    }

    It 'writes no comment outside a pull request' {
        $env:GITHUB_EVENT_NAME = 'push'
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 0
        $script:Calls.Count | Should -Be 0
    }

    It 'names pull-requests: write when <Method> answers <Status>' -ForEach @(
        @{ Method = 'Get'; Status = 403 }
        @{ Method = 'Post'; Status = 403 }
        @{ Method = 'Get'; Status = 404 }
    ) {
        Use-FakeApi -Answers @{ $Method = @{ Status = $Status; Content = '{"message":"Resource not accessible by integration"}' } }
        Invoke-QgateAction -Step Comment | Should -Be 1
        $text = Get-LogText
        $text | Should -Match '^::error::'
        $text | Should -Match 'pull-requests: write'
        $text | Should -Match "$Status.*Resource not accessible by integration"
        $script:Calls.Method | Should -Not -Contain 'Patch'
        if ($Method -eq 'Get') { $script:Calls.Method | Should -Not -Contain 'Post' }
    }

    It 'gives the status and GitHub''s message for other failures' {
        Use-FakeApi -Answers @{ Post = @{ Status = 500; Content = '{"message":"Server Error"}' } }
        Invoke-QgateAction -Step Comment | Should -Be 1
        Get-LogText | Should -Match '::error::qgate: GitHub answered 500 to POST .*/issues/7/comments: Server Error'
    }

    It 'says when GitHub does not answer' {
        Use-FakeApi -Answers @{ Get = { throw [System.Net.Http.HttpRequestException]::new('Name or service not known') } }
        Invoke-QgateAction -Step Comment | Should -Be 1
        Get-LogText | Should -Match '::error::qgate: no answer from GitHub to GET .*Name or service not known'
    }

    It 'never prints the token' {
        Use-FakeApi -Answers @{ Get = @{ Status = 401; Content = '{"message":"Bad credentials"}' } }
        Invoke-QgateAction -Step Comment | Should -Be 1
        Get-LogText | Should -Not -Match $script:Token
        Get-LogText | Should -Match '401'
    }

    It 'refuses a report without the marker line' {
        Set-Content -LiteralPath (Get-QgatePath -Name 'report.md') -Value '# something else'
        Use-FakeApi
        Invoke-QgateAction -Step Comment | Should -Be 1
        Get-LogText | Should -Match 'not a qgate Markdown report'
        $script:Calls.Count | Should -Be 0
    }
}

Describe 'The Gate step' {
    BeforeEach {
        Set-ActionEnvironment
        Use-FakeHost
        $script:QgateCalls = [System.Collections.Generic.List[object]]::new()
        $script:QgateExit = 1
        $script:QgateStderr = "qgate: failing: 1 finding at or above `"error`" (see --fail-on)`n"
        Mock Invoke-Qgate {
            $script:QgateCalls.Add($Arguments)
            Set-Content -LiteralPath $StdoutPath -NoNewline -Value "::error file=src/a.ts,line=1,title=r/x::m`n"
            Set-Content -LiteralPath $StderrPath -NoNewline -Value $script:QgateStderr
            $script:QgateExit
        }
        $script:Raw = [System.Collections.Generic.List[string]]::new()
        Mock Write-QgateOutput { $script:Raw.Add((Get-Content -LiteralPath $Path -Raw)) }
    }
    AfterEach { Clear-ActionEnvironment }

    It 'returns qgate''s exit code and passes its annotations through unchanged' {
        Invoke-QgateAction -Step Gate | Should -Be 1
        $script:QgateCalls[0] | Should -Be @('check', '--base', 'origin/main', '--format', 'github', '--fail-on', 'error')
        $script:Raw | Should -Be @("::error file=src/a.ts,line=1,title=r/x::m`n")
        Get-LogText | Should -Match 'qgate: failing: 1 finding'
    }

    It 'succeeds when qgate does' {
        $script:QgateExit = 0
        Invoke-QgateAction -Step Gate | Should -Be 0
    }

    It 'uses the base input on a push' {
        $env:GITHUB_EVENT_NAME = 'push'
        $env:GITHUB_BASE_REF = ''
        $env:QGATE_INPUT_BASE = 'origin/main'
        Invoke-QgateAction -Step Gate | Should -Be 1
        $script:QgateCalls[0][2] | Should -BeExactly 'origin/main'
    }

    It 'reports a qgate that could not run' {
        $script:QgateExit = 2
        $script:QgateStderr = 'error: unknown revision `origin/main`'
        Invoke-QgateAction -Step Gate | Should -Be 2
        Get-LogText | Should -Match '::error::qgate check could not run \(exit code 2\)'
    }
}

Describe 'Invoke-QgateAction' {
    BeforeEach { Use-FakeHost }

    It 'escapes %, CR and LF in its error message' {
        Mock Invoke-QgateInstall { throw "100% broken`r`nnext" }
        Invoke-QgateAction -Step Install | Should -Be 1
        $script:Log | Should -Be @('::error::100%25 broken%0D%0Anext')
    }
}
