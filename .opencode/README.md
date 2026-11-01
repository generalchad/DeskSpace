# OpenCode agents

Optional role definitions for [OpenCode](https://opencode.ai/docs/agents/).
Nothing in DeskSpace depends on them. They hold orchestration only; the rules
are in the root `AGENTS.md` and the documents it references.

Each stack is a primary agent with four subagents: explorer, worker, tester,
and reviewer. A primary can only delegate to its own family. Press Tab to pick
a primary, or start one with `opencode --agent <family>-main`. Type
`@<family>-<role>` to call a subagent yourself.

**Change a model:** edit `model:` (and `variant:`, if the model has one) in the
frontmatter. `opencode models` lists the ids. Nothing else needs to change.

**Add a family:** copy the five files of a stack to a new prefix, then replace
the old prefix and set the models. The role text is the same in every family
apart from agent names, because OpenCode does not expand `{file:}` in an agent's
Markdown body. So when you change a role, change it in every family.

**What is enforced:** turning a tool off is a real boundary. Explorers and
reviewers have no edit tool and no shell, and subagents cannot start subagents.
Bash patterns are not. In OpenCode 1.19.5 on Windows, `./script.ps1` and any
command with a bare `--` skipped every pattern, even `"*": deny`. So the `ask`
and `deny` bash rules on the main, worker, and tester agents catch common
commands only. Directive 1 in `AGENTS.md` still binds every agent. Re-test
before relying on a bash rule as a boundary.

**Another tool:** the role split carries over. A tool that reads `AGENTS.md`
already has the rules, and the role text here reads as plain instructions. A
tool with no subagents should follow the main agent's lifecycle itself.
