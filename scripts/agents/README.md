# Agent stacks

Optional role definitions for agentic coding tools. Nothing in DeskSpace depends
on them. They hold orchestration only; the rules are in the root `AGENTS.md` and
the documents it references.

The source is here: `stacks.psd1` (models, roles, permissions) and `roles/` (role
text). `sync.ps1` generates `.opencode/agents/`, `.claude/agents/`, and
`.github/agents/`. Edit the source and run it; `sync.ps1 -Check` fails on drift.

## Tiers

Use the cheapest agent that can do the job reliably.

| Tier | Job | Roles |
|------|-----|-------|
| A: lead | Ambiguity, architecture, risk, integration, final acceptance | `main` (`lead` outside OpenCode), `senior-reviewer` |
| B: engineer | Implementation, debugging, deep exploration, review | `worker`, `tester`, `explorer`, `reviewer` |
| C: scout | Search, inventories, known commands, log triage, exact edits | `scout`, `mechanic` |

`stacks.psd1` says which model fills each tier. A tool with no subagents is the
fourth tier: the lead does every step itself, reviews its own work as a skeptic,
and says that independent review was unavailable.

## Using them

| Tool | Pick a lead | Call a subagent yourself |
|------|-------------|--------------------------|
| OpenCode | Tab, or `opencode --agent <family>-main` | `@<family>-<role>` |
| Claude Code | `claude --agent lead` | `@agent-<role>`, or ask by name |
| Copilot CLI | `copilot --agent lead` | `/agent`, or ask by name |

**A model with no stack:** in OpenCode, pick the model with `any-main`. Its
subagents have no model of their own and inherit it. That needs no setup and
survives model churn.

## Changing the stacks

**Change a model:** edit its tier in `stacks.psd1` and run `sync.ps1`. In
OpenCode, `opencode models` lists the ids and `opencode models <provider>
--verbose` lists the variants. Do not guess one. A listed model may still be
unusable on your account, so run it once. Do not assume what a name such as
`flash` or `mini` means.

**Add a family:** add one entry under `OpenCode` and run `sync.ps1`. Put the
strongest reliable model in A and competent cheaper ones in B and C. With no
cheaper sibling, use the same model throughout; isolated context and independent
review are still worth having.

**Cross-family routing:** none is configured, so cost, privacy, and behavior stay
predictable. A primary can only delegate inside its own family. To add a route
for independent review, a missing capability, or a much cheaper verified scout,
set that model in the tier and write the reason in a comment beside it.

**Free or promotional models:** none is configured. They are optional capacity,
not infrastructure. Use one only for tier C or low-risk tier B work, once you
know its tool use is adequate and its data handling is acceptable. Never pick a
model because it is free.

## What is enforced

Turning a tool off is a real boundary. Everything else below is not.

| Restriction | OpenCode | Claude Code | Copilot CLI |
|-------------|----------|-------------|-------------|
| Read-only roles: no edit, no shell | Enforced | Enforced | Tool filter |
| Tester: no production edits | Asks each time | Enforced | Tool filter |
| Subagents cannot start subagents | Enforced | Enforced | Tool filter |
| Lead delegates only to its own roles | Enforced | With `--agent lead` | Prompt only |
| No commit or push by a worker | Partial (below) | Prompt only | Prompt only |
| Destructive commands ask first | Partial (below) | Prompt only | Prompt only |
| Directive 1, scope, on-screen checks | Prompt only | Prompt only | Prompt only |

Typing an `@` mention yourself can bypass the OpenCode delegation limit.

OpenCode's bash patterns catch common commands only. In OpenCode 1.19.5 on
Windows, `./script.ps1` and any command with a bare `--` skipped every pattern,
even `"*": deny`. So read-only roles have no shell at all, and hand-offs of git
output go through the lead. Re-test before relying on a bash rule as a boundary.

## Limits

- **Claude Code:** `--agent lead` replaces the default system prompt and has only
  the tools the file lists, so no MCP tools.
- **Copilot CLI:** it cannot limit which agents the lead starts. Running it
  queued this repository for GitHub's remote code index.
- **VS Code:** it reads both `.github/agents/` and `.claude/agents/`, so the same
  names can appear twice. These files have not been tested there, and its model
  names may differ from the CLI's.
- **Other tools:** a tool that reads `AGENTS.md` already has the rules, and the
  text in `roles/` reads as plain instructions.
