// iCalendar VEVENT parser/generator - minimal RFC 5545 subset

use crate::types::{CalendarEvent, EventSource};
use chrono::{DateTime, TimeZone, Utc};
use common::Result;

/// Parse a VEVENT from iCalendar format (minimal RFC 5545 subset)
pub fn parse_vevent(ical_data: &str) -> Result<CalendarEvent> {
    let mut uid = None;
    let mut summary = None;
    let mut description = None;
    let mut dtstart = None;
    let mut dtend = None;

    let mut in_vevent = false;

    for line in ical_data.lines() {
        let line = line.trim();

        if line == "BEGIN:VEVENT" {
            in_vevent = true;
            continue;
        }
        if line == "END:VEVENT" {
            break;
        }

        if !in_vevent {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            match key {
                "UID" => uid = Some(value.to_string()),
                "SUMMARY" => summary = Some(value.to_string()),
                "DESCRIPTION" => description = Some(value.to_string()),
                "DTSTART" => dtstart = Some(parse_datetime(value)?),
                "DTEND" => dtend = Some(parse_datetime(value)?),
                _ => {}
            }
        }
    }

    let uid = uid.ok_or_else(|| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Missing UID".to_string(),
        ))
    })?;
    let summary = summary.ok_or_else(|| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Missing SUMMARY".to_string(),
        ))
    })?;
    let start = dtstart.ok_or_else(|| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Missing DTSTART".to_string(),
        ))
    })?;
    let end = dtend.ok_or_else(|| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Missing DTEND".to_string(),
        ))
    })?;

    Ok(CalendarEvent {
        uid,
        summary,
        description,
        start,
        end,
        source: EventSource::CalDAV,
        etag: None,
    })
}

/// Generate VEVENT in iCalendar format
pub fn generate_vevent(event: &CalendarEvent) -> Result<String> {
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Klyntbot//Calendar//EN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{}", event.uid),
        format!("SUMMARY:{}", event.summary),
    ];

    if let Some(desc) = &event.description {
        lines.push(format!("DESCRIPTION:{}", desc));
    }

    lines.push(format!("DTSTART:{}", format_datetime(&event.start)));
    lines.push(format!("DTEND:{}", format_datetime(&event.end)));
    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());

    Ok(lines.join("\r\n"))
}

/// Parse iCalendar datetime format (YYYYMMDDTHHMMSSZ or YYYYMMDDTHHMMSS floating)
fn parse_datetime(dt_str: &str) -> Result<DateTime<Utc>> {
    // Handle both UTC (with Z) and floating time (without Z)
    let dt_str = dt_str.trim_end_matches('Z');

    if dt_str.len() != 15 || dt_str.chars().nth(8) != Some('T') {
        return Err(common::KlyntbotError::Calendar(
            common::CalendarError::ProtocolError("Invalid datetime format".to_string()),
        ));
    }

    let year: i32 = dt_str[0..4].parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid year".to_string(),
        ))
    })?;
    let month: u32 = dt_str[4..6].parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid month".to_string(),
        ))
    })?;
    let day: u32 = dt_str[6..8].parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid day".to_string(),
        ))
    })?;
    let hour: u32 = dt_str[9..11].parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid hour".to_string(),
        ))
    })?;
    let minute: u32 = dt_str[11..13].parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid minute".to_string(),
        ))
    })?;
    let second: u32 = dt_str[13..15].parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid second".to_string(),
        ))
    })?;

    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .ok_or_else(|| {
            common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
                "Invalid datetime".to_string(),
            ))
        })
}

/// Format datetime as iCalendar floating time (no timezone)
/// Floating time is interpreted by calendar apps in the user's local timezone
/// This prevents timezone conversion issues where "5pm" becomes "midnight next day"
fn format_datetime(dt: &DateTime<Utc>) -> String {
    // Remove Z suffix to use floating time (local timezone interpretation)
    dt.format("%Y%m%dT%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CalendarEvent, EventSource};
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_parse_simple_vevent() {
        let ical_data = "\
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VEVENT
UID:event-123@example.com
SUMMARY:Team Meeting
DTSTART:20260215T140000Z
DTEND:20260215T150000Z
END:VEVENT
END:VCALENDAR";

        let event = parse_vevent(ical_data).unwrap();

        assert_eq!(event.uid, "event-123@example.com");
        assert_eq!(event.summary, "Team Meeting");
        assert_eq!(event.description, None);
        assert_eq!(
            event.start,
            Utc.with_ymd_and_hms(2026, 2, 15, 14, 0, 0).unwrap()
        );
        assert_eq!(
            event.end,
            Utc.with_ymd_and_hms(2026, 2, 15, 15, 0, 0).unwrap()
        );
        assert_eq!(event.source, EventSource::CalDAV);
    }

    #[test]
    fn test_parse_vevent_with_description() {
        let ical_data = "\
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:event-456
SUMMARY:Project Review
DESCRIPTION:Discuss Q1 roadmap and milestones
DTSTART:20260220T100000Z
DTEND:20260220T110000Z
END:VEVENT
END:VCALENDAR";

        let event = parse_vevent(ical_data).unwrap();

        assert_eq!(event.uid, "event-456");
        assert_eq!(event.summary, "Project Review");
        assert_eq!(
            event.description,
            Some("Discuss Q1 roadmap and milestones".to_string())
        );
    }

    #[test]
    fn test_generate_vevent() {
        let event = CalendarEvent {
            uid: "test-event-789".to_string(),
            summary: "Standup Meeting".to_string(),
            description: Some("Daily team sync".to_string()),
            start: Utc.with_ymd_and_hms(2026, 3, 1, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 9, 15, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: None,
        };

        let ical_data = generate_vevent(&event).unwrap();

        // Verify the generated iCalendar contains required fields
        assert!(ical_data.contains("BEGIN:VCALENDAR"));
        assert!(ical_data.contains("VERSION:2.0"));
        assert!(ical_data.contains("BEGIN:VEVENT"));
        assert!(ical_data.contains("UID:test-event-789"));
        assert!(ical_data.contains("SUMMARY:Standup Meeting"));
        assert!(ical_data.contains("DESCRIPTION:Daily team sync"));
        assert!(ical_data.contains("DTSTART:20260301T090000"));
        assert!(ical_data.contains("DTEND:20260301T091500"));
        assert!(ical_data.contains("END:VEVENT"));
        assert!(ical_data.contains("END:VCALENDAR"));
    }

    #[test]
    fn test_roundtrip_parse_generate() {
        let original = CalendarEvent {
            uid: "roundtrip-test".to_string(),
            summary: "Test Event".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 4, 15, 13, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: None,
        };

        let ical_data = generate_vevent(&original).unwrap();
        let parsed = parse_vevent(&ical_data).unwrap();

        assert_eq!(parsed.uid, original.uid);
        assert_eq!(parsed.summary, original.summary);
        assert_eq!(parsed.description, original.description);
        assert_eq!(parsed.start, original.start);
        assert_eq!(parsed.end, original.end);
    }

    #[test]
    fn test_parse_invalid_vevent() {
        let invalid_data = "NOT A VALID VEVENT";
        let result = parse_vevent(invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_required_fields() {
        let missing_uid = "\
BEGIN:VCALENDAR
BEGIN:VEVENT
SUMMARY:No UID
DTSTART:20260101T120000Z
DTEND:20260101T130000Z
END:VEVENT
END:VCALENDAR";

        let result = parse_vevent(missing_uid);
        assert!(result.is_err());
    }
}
