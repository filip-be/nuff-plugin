Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$pluginRoot = Split-Path -Parent $scriptRoot

function Read-Text {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    return Get-Content -Path $Path -Raw -Encoding UTF8
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Needle,
        [Parameter(Mandatory = $true)]
        [string]$Because
    )

    $text = Read-Text -Path $Path
    if ($text.IndexOf($Needle, [System.StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "Assertion failed: $Because`nMissing text: $Needle`nFile: $Path"
    }
}

function Assert-NotContains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Needle,
        [Parameter(Mandatory = $true)]
        [string]$Because
    )

    $text = Read-Text -Path $Path
    if ($text.IndexOf($Needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "Assertion failed: $Because`nUnexpected text: $Needle`nFile: $Path"
    }
}

$doCommand = Join-Path $pluginRoot 'commands/do.nuff.md'
$doEverythingCommand = Join-Path $pluginRoot 'commands/do-everything.nuff.md'
$improveCommand = Join-Path $pluginRoot 'commands/improve.nuff.md'
$refineSkill = Join-Path $pluginRoot 'skills/nuff-workflow-refine/SKILL.md'
$planSkill = Join-Path $pluginRoot 'skills/nuff-workflow-plan/SKILL.md'
$codeSkill = Join-Path $pluginRoot 'skills/nuff-workflow-code/SKILL.md'
$improveSkill = Join-Path $pluginRoot 'skills/nuff-workflow-improve/SKILL.md'
$stateSkill = Join-Path $pluginRoot 'skills/nuff-state-update/SKILL.md'
$adoSkill = Join-Path $pluginRoot 'skills/nuff-ado-orchestration/SKILL.md'
$readme = Join-Path $pluginRoot 'README.md'

$checks = @(
    @{ Path = $doCommand; Needle = '[--phase refine|plan|code|improve]'; Because = 'do.nuff should advertise improve as a supported phase override.' },
    @{ Path = $doCommand; Needle = 'consult the `nuff-state-update` skill with `initialize-state`'; Because = 'do.nuff should delegate STATE.md initialization to the shared state helper.' },
    @{ Path = $doCommand; Needle = 'consult the `nuff-ado-orchestration` skill with `detect-process-template`'; Because = 'do.nuff should delegate process template detection to the shared ADO helper.' },
    @{ Path = $refineSkill; Needle = 'Consult the `nuff-ado-orchestration` skill with `fetch-work-item`'; Because = 'refine should use the ADO helper for work item retrieval.' },
    @{ Path = $refineSkill; Needle = 'Consult the `nuff-state-update` skill with `complete-refine`'; Because = 'refine should use the shared state helper for the refine-to-plan transition.' },
    @{ Path = $planSkill; Needle = 'Consult the `nuff-ado-orchestration` skill with `create-child-tasks`'; Because = 'plan should use the shared ADO helper for child task creation.' },
    @{ Path = $planSkill; Needle = 'Consult the `nuff-state-update` skill with `complete-plan`'; Because = 'plan should use the shared state helper for the plan-to-code transition.' },
    @{ Path = $codeSkill; Needle = 'Consult the `nuff-ado-orchestration` skill with `transition-parent-work-item`'; Because = 'code should use the shared ADO helper when coding starts.' },
    @{ Path = $codeSkill; Needle = 'Consult the `nuff-ado-orchestration` skill with `transition-review-state`'; Because = 'code should use the shared ADO helper for review transitions.' },
    @{ Path = $codeSkill; Needle = 'Consult the `nuff-state-update` skill with `mark-code-task-active`'; Because = 'code should use the shared state helper for task activation.' },
    @{ Path = $codeSkill; Needle = 'Consult the `nuff-state-update` skill with `mark-code-task-complete`'; Because = 'code should use the shared state helper for task completion.' },
    @{ Path = $codeSkill; Needle = 'Consult the `nuff-state-update` skill with `set-verification-status`'; Because = 'code should use the shared state helper for verification status updates.' },
    @{ Path = $codeSkill; Needle = 'Consult the `nuff-state-update` skill with `set-user-approval-status`'; Because = 'code should use the shared state helper for user approval status updates.' },
    @{ Path = $improveSkill; Needle = 'Consult the `nuff-state-update` skill with `complete-improve`'; Because = 'improve should use the shared state helper for completion.' },
    @{ Path = $improveCommand; Needle = 'If neither file exists'; Because = 'improve.nuff should handle the no-artifacts case explicitly.' },
    @{ Path = $improveCommand; Needle = 'If only one exists, continue.'; Because = 'improve.nuff should support improvements-only and reflections-only runs.' },
    @{ Path = $doEverythingCommand; Needle = 'Refine → Plan → Code → Improve'; Because = 'do-everything.nuff should cover all four phases.' },
    @{ Path = $stateSkill; Needle = 'name: nuff-state-update'; Because = 'the shared state helper skill must exist.' },
    @{ Path = $adoSkill; Needle = 'name: nuff-ado-orchestration'; Because = 'the shared ADO helper skill must exist.' },
    @{ Path = $readme; Needle = '`nuff-state-update`'; Because = 'README should document the shared state helper.' },
    @{ Path = $readme; Needle = '`nuff-ado-orchestration`'; Because = 'README should document the shared ADO helper.' }
)

foreach ($check in $checks) {
    Assert-Contains -Path $check.Path -Needle $check.Needle -Because $check.Because
}

Assert-NotContains -Path $refineSkill -Needle 'Consult the `nuff-ado-update` skill (artifact comment, no state transition)' -Because 'refine should route artifact posting through the shared ADO helper.'
Assert-NotContains -Path $planSkill -Needle 'Consult the `nuff-ado-update` skill (artifact comment, no state transition)' -Because 'plan should route artifact posting through the shared ADO helper.'

Write-Host 'Behavioral workflow prompt validation passed.' -ForegroundColor Green