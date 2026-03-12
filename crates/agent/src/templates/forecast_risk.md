You are analyzing estimation accuracy data and historical patterns to identify risks for a task or project forecast.

## Task/Project
{{ subject }}

## Historical Data
- Sample size: {{ sample_size }} completed tasks
- Data quality: {{ data_quality }}
- Mean deviation: {{ mean_deviation }}%
- Estimated minutes: {{ estimated_minutes }}
- Confidence low: {{ confidence_low }} minutes
- Confidence high: {{ confidence_high }} minutes

## Additional Context
{{ context }}

## Instructions

Analyze the data and identify 0-3 risks. For each risk:
1. Classify the kind: historicalunderestimation, dependencychain, unknowncomplexity, resourcecontention, externaldependency
2. Provide a brief description explaining the risk
3. Estimate impact in minutes if applicable

Respond in JSON:
```json
{
  "risks": [
    {
      "kind": "historicalunderestimation",
      "description": "Based on 15 similar tasks, estimates are typically 38% too optimistic.",
      "impact_minutes": 30
    }
  ]
}
```

If no risks, return `{"risks": []}`.
