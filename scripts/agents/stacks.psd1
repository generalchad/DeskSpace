@{
    # OpenCode stacks: one primary and its subagents per family. A, B, and C are
    # the tiers in README.md. A tier with no Model inherits the session's model.
    # Ids come from `opencode models`; Variant must be one that model lists.
    OpenCode = @{
        any      = @{ Label = 'model-agnostic'; A = @{}; B = @{}; C = @{} }
        claude   = @{
            Label = 'Claude'
            A     = @{ Model = 'anthropic/claude-opus-5-5'; Variant = 'medium' }
            B     = @{ Model = 'anthropic/claude-sonnet-5-5'; Variant = 'medium' }
            C     = @{ Model = 'anthropic/claude-haiku-5-5'; Variant = 'medium' }
        }
        deepseek = @{
            Label = 'DeepSeek'
            A     = @{ Model = 'deepseek/deepseek-v4-pro' }
            B     = @{ Model = 'deepseek/deepseek-flash' }
            C     = @{ Model = 'deepseek/deepseek-flash' }
        }
        glm      = @{
            Label = 'GLM'
            A     = @{ Model = 'openrouter/z-ai/glm-5.3' }
            B     = @{ Model = 'openrouter/z-ai/glm-5.3-flash' }
            C     = @{ Model = 'openrouter/z-ai/glm-5.3-flash' }
        }
        kimi     = @{
            Label = 'Kimi'
            A     = @{ Model = 'openrouter/moonshotai/kimi-k3.1-pro' }
            B     = @{ Model = 'openrouter/moonshotai/kimi-k2.7-code' }
            C     = @{ Model = 'openrouter/moonshotai/kimi-k2.7-code' }
        }
        openai   = @{
            Label = 'OpenAI'
            A     = @{ Model = 'openrouter/openai/gpt-6.1-sol'; Variant = 'medium' }
            B     = @{ Model = 'openrouter/openai/gpt-6.1-terra'; Variant = 'medium' }
            C     = @{ Model = 'openrouter/openai/gpt-6-luna'; Variant = 'medium' }
        }
    }

    # Claude Code has one family. Model is an alias its docs define, so it follows
    # new releases on its own.
    ClaudeCode = @{
        A = @{ Model = 'opus'; Effort = 'medium' }
        B = @{ Model = 'sonnet'; Effort = 'medium' }
        C = @{ Model = 'haiku'; Effort = 'medium' }
    }

    # Copilot CLI. Tier A has no model, so it follows the one picked for the session.
    Copilot = @{
        A = @{}
        B = @{ Model = 'gpt-6.1-terra' }
        C = @{ Model = 'gpt-6-luna' }
    }

    # Access decides what each tool's file may do; sync.ps1 maps it per tool.
    # Body is the file under roles/.
    Roles = @{
        main              = @{
            Tier = 'A'; Access = 'lead'; Body = 'lead'
            Description = 'Lead and senior implementer. Owns ambiguous requirements, architecture, risk decisions, integration, and final acceptance; delegates search, bounded work, and review to its subagents.'
        }
        scout             = @{
            Tier = 'C'; Access = 'read'; Body = 'scout'
            Description = 'Cheap, fast, read-only search. Use proactively for high-volume, low-ambiguity lookups: every call to a symbol, the tests or docs touching a module, the files of a feature, an inventory of config keys, stale references in docs. Not for design or correctness judgments.'
        }
        mechanic          = @{
            Tier = 'C'; Access = 'write'; Body = 'mechanic'
            Description = 'Cheap, fast execution of exact instructions. Use to run known test and build commands and summarize failures, triage verbose logs, or make a tightly specified mechanical edit. Not for anything that needs judgment.'
        }
        explorer          = @{
            Tier = 'B'; Access = 'read'; Body = 'explorer'
            Description = 'Read-only investigation that needs interpretation. Use to explain how a code path works, which decisions constrain it, and which existing pattern is closest. Use the scout for plain lookups.'
        }
        worker            = @{
            Tier = 'B'; Access = 'write'; Body = 'worker'
            Description = 'Engineer for substantive, clearly bounded implementation: contained features, refactors with known architecture, and meaningful tests. Use after the lead has set the boundaries.'
        }
        tester            = @{
            Tier = 'B'; Access = 'verify'; Body = 'tester'
            Description = 'Verification and diagnosis that needs reasoning. Use to check a change against its acceptance criteria and to debug failures. Use the mechanic to run known commands.'
        }
        reviewer          = @{
            Tier = 'B'; Access = 'read'; Body = 'reviewer'
            Description = 'Independent, skeptical, read-only code review of a significant change. Inspects the code itself rather than trusting the summary, and reports findings by severity.'
        }
        'senior-reviewer' = @{
            Tier = 'A'; Access = 'read'; Body = 'reviewer'
            Description = 'The same independent review on the strongest model, in a fresh context. Use only for risky changes: security, destructive file semantics, concurrency, lifetimes, Win32 correctness, or reviewers who disagree.'
        }
    }

    # Commands that ask first on agents with a shell, under OpenCode. They catch
    # common forms only; README.md says what escapes them.
    AskBash = @(
        'rm *', 'rmdir *', 'rd *', 'del *', 'erase *', 'ri *', 'mv *', 'move *'
        'Remove-Item *', 'Move-Item *', 'Clear-Content *', 'Clear-RecycleBin*'
        'git reset --hard*', 'git checkout -- *', 'git checkout * -- *'
        'git restore *', 'git clean *', 'git branch -D *', 'git stash drop*'
        'git stash clear*', 'git push *--force*', 'git push -f*', 'git push * -f*'
        '*dev.ps1*-Fresh*'
    )

    # The rust MCP tools that cannot change anything.
    ReadOnlyRustTools = @('rust_cargo-metadata', 'rust_cargo-tree', 'rust_workspace-info')
}
