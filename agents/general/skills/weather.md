---
name: weather
description: Get current weather and forecasts (no API key required)
always: false
---

Two free services, no API keys needed.

## wttr.in (primary)

```bash
curl -s "wttr.in/London?format=3"          # Quick: London: +8C
curl -s "wttr.in/London?format=%l:+%c+%t+%h+%w"  # Compact
curl -s "wttr.in/London?T"                  # Full forecast
```

Tips: URL-encode spaces (`New+York`), airport codes (`JFK`), units `?m`/`?u`, today `?1`, current `?0`.

## Open-Meteo (fallback, JSON)

```bash
curl -s "https://api.open-meteo.com/v1/forecast?latitude=51.5&longitude=-0.12&current_weather=true"
```
