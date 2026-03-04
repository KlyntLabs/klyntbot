---
name: project-management
description: Project, area, and OKR management using the PARA+OKR framework
always: false
---

## Framework

Tasks are organized using PARA (Projects, Areas, Resources, Archives) combined with OKR (Objectives and Key Results).

## Areas

Areas are top-level categories (e.g., Work, Personal, Health). Every task belongs to an area.

- `area list` — show all areas
- `area add` — create a new area
- `area show` — view area details with projects

## Projects

Projects group related tasks within an area. They have deadlines and completion targets.

- `project list` — list projects (optionally filtered by area)
- `project add` — create a project within an area
- `project show` — view project details with tasks

## OKRs

Objectives and Key Results provide goal-setting at the area level.

- `okr list` — list objectives
- `okr add_objective` — create an objective
- `okr add_key_result` — add a measurable key result
- `okr update_progress` — update key result progress

## Workflow

1. User creates areas for life domains
2. Within areas, create projects for time-bound initiatives
3. Set objectives with measurable key results
4. Tasks link to projects and key results for tracking
