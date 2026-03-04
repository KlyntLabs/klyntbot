---
name: skill-creator
description: Create or update agent skills with proper structure and metadata
always: false
---

## About Skills

Skills are modular packages that extend agent capabilities with specialized knowledge, workflows, and tools.

## Skill Structure

```
skill-name/
  SKILL.md (required) — YAML frontmatter + markdown instructions
  scripts/            — Executable code (optional)
  references/         — Documentation loaded on demand (optional)
  assets/             — Files used in output (optional)
```

## SKILL.md Format

```yaml
---
name: skill-name
description: What the skill does and when to use it
always: false
---

# Skill Title

Instructions in imperative form.
```

## Core Principles

- **Concise is key** — the context window is shared. Only add what the agent doesn't already know.
- **Prefer examples over explanations**
- Naming: lowercase, hyphens, verb-led phrases
- Description: include both what it does AND when to trigger it
