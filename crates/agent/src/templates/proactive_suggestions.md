You are analyzing a task to generate actionable suggestions. You will receive the task details and a trigger reason.

## Trigger: {{ trigger }}

## Task
- ID: {{ task_id }}
- Title: {{ title }}
- Status: {{ status }}
- Priority: {{ priority }}
- Energy Level: {{ energy_level }}
- Estimated Minutes: {{ estimated_minutes }}
- Due Date: {{ due_date }}
- Tags: {{ tags }}
- Created: {{ created_at }}
- Description: {{ description }}

## Context
{{ context }}

## Instructions

Based on the trigger and task details, generate 0-3 suggestion candidates. Each suggestion should be:
1. **Actionable** — maps to a concrete action type
2. **Confident** — include a confidence score (0.0-1.0) reflecting how sure you are this suggestion is helpful
3. **Reasoned** — brief explanation of why this suggestion is appropriate

Available suggestion types and their action formats:
- "reprioritize" → `{"type": "setpriority", "priority": <1-5>}`
- "reschedule" → `{"type": "setduedate", "due_date": "YYYY-MM-DD"}`
- "decompose" → `{"type": "triggerdecomposition"}`
- "adjustestimation" → `{"type": "updateestimationbaseline", "minutes": <integer>}`
- "adjustenergy" → `{"type": "setenergylevel", "level": "low"|"medium"|"high"|"deep"}`
- "abandon" → `{"type": "archive"}`
- "workflowinsight" → `{"type": "informational"}`

Respond in JSON:
```json
{
  "suggestions": [
    {
      "suggestion_type": "reprioritize",
      "title": "Brief title",
      "description": "Why this suggestion",
      "confidence": 0.85,
      "action": {"type": "setpriority", "priority": 1}
    }
  ]
}
```

If no suggestions are warranted, return `{"suggestions": []}`.
