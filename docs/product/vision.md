# Product Vision: KlyntBot

Status: Draft
Date: 2026-09-02

<!--
The repo-level north star: WHAT this project is and for whom, above any single
feature. Optional — only large/long-lived projects need it. `frame-change` reads it
to check a new idea's scope; it never gates. Keep each section tight; every heading
is a REQUIRED slot — fill it or write `None`.
Flesh out with `/define-project`.
-->

## Problem

Personal AI assistants are either cloud-locked chat boxes or fragmented tool
stacks. Users need a local-first desktop agent that can chat, run tools, and
persist memory without surrendering their data to a single SaaS.

## Users

Primary: individual power users who want a personal AI agent on their Mac
(tasks, notes, coding assistance, local memory) with optional cloud model APIs.

## Goals

- **GOAL-1** Ship a reliable macOS Tauri desktop agent with assistant and coding modes.
- **GOAL-2** Keep user data local-first under `~/.klyntbot/` with explicit tool approval.
- **GOAL-3** Expose a coherent tool surface to the LLM, subagents, and optional MCP.

## Non-goals

- Multi-tenant hosted SaaS backend
- Structured observability dashboards (OpenTelemetry / Prometheus) for this single-user app
- Pre-1.0 backwards-compatible data migrations (alter in place until first release)

## Scope boundaries

- Platforms: macOS desktop (Tauri 2) first
- Modes: assistant vs coding sessions are creation-time and immutable
- Flesh remaining product constraints with `/define-project`
