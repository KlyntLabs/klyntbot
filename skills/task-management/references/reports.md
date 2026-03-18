---
name: reports
description: Data-driven report generation — Weekly Review Report, Project Retrospective, Knowledge Growth
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-17"
  source: official
  tags: "report,review,retrospective,knowledge,summary"
  always: false
  triggers: "weekly report,project retrospective,what did I learn,knowledge review,knowledge growth,how did the project go,retrospective for"
  agent: task
---

## When to Use

- User asks for a **report** (data summary), NOT an interactive review
- Distinction: "weekly review" → interactive `weekly-review.md`; "weekly report" → this reference
- Automated cron trigger (e.g., "Every Sunday at 6pm, generate my weekly report")

## Report: Weekly Review Report

### Triggers
"weekly report", "week summary", "what happened this week"

### Sections (execute in order)

1. **Accomplishments**
   - Tool: `tasks list` with filter for completed this week
   - Present: count + top highlights

2. **In Progress**
   - Tool: `tasks list` with filter for status = in_progress
   - Present: grouped by project if possible

3. **Blockers & Overdue**
   - Tool: `tasks list` with filter for overdue or blocked
   - Present: each with days overdue and priority

4. **Patterns**
   - Tool: `productivity activity_summary` for the week
   - Tool: `productivity focus_sessions` if available
   - Present: total focused hours, peak days, trends vs last week

5. **Knowledge Growth**
   - Tool: `notes list` filtered to created/modified this week
   - If cognitive memory available: mention new facts learned count
   - Present: topics explored, notes created

6. **Next Week**
   - Tool: `tasks list` with filter for due next 7 days
   - Present: upcoming deadlines, suggested focus areas

### Output Format
Present as a clean markdown report with section headers (##).
End with a brief "Key takeaway" sentence.
Do NOT ask interactive questions — this is a passive report, not a review workflow.

---

## Report: Project Retrospective

### Triggers
"retrospective for {project}", "how did {project} go", "project retrospective"

### Sections

1. **Project Overview**
   - Tool: `project get` for the named project
   - Present: title, status, date range, completion %

2. **Task Analysis**
   - Tool: `tasks list` filtered by project
   - Present: completed vs total, overdue count, avg completion time

3. **OKR Progress** (if objectives exist)
   - Tool: `okr list` filtered by project
   - Present: each KR with current vs target

4. **Time Investment**
   - Tool: `productivity activity_summary` filtered by project tags
   - Present: total hours, focus distribution

5. **Lessons Learned**
   - Synthesize patterns from the data above
   - Present: what went well, what could improve, key decisions

### Output Format
Structured markdown. If the user doesn't name a project, list active projects and ask which one.

---

## Report: Knowledge Growth

### Triggers
"what did I learn", "knowledge review", "knowledge growth"

### Sections

1. **Notes Activity**
   - Tool: `notes list` filtered to recent period (default: 7 days)
   - Present: notes created, notes modified, word count growth

2. **Topics Explored**
   - Analyze note titles and tags for theme clustering
   - Present: main topics with note counts

3. **Memory Evolution**
   - If TemporalService available: use `change_summary` for the period
   - Present: new facts learned, facts updated, contradictions resolved

4. **Flashcard Performance** (if available)
   - Query flashcard review stats for the period
   - Present: cards reviewed, accuracy rate, mastery progress

### Output Format
Markdown report. Focus on growth narrative, not raw numbers.
