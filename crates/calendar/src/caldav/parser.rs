// iCalendar VEVENT parser/generator - minimal RFC 5545 subset with VTIMEZONE support

use crate::types::{CalendarEvent, EventSource};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use common::Result;

/// Parse a VEVENT from iCalendar format (minimal RFC 5545 subset)
/// Handles DTSTART/DTEND with optional TZID parameter and UTC Z suffix
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

        // Split at first colon to get key (with possible params) and value
        if let Some((key_with_params, value)) = line.split_once(':') {
            // Separate field name from parameters (e.g., "DTSTART;TZID=Asia/Bangkok")
            let (field_name, params) = match key_with_params.split_once(';') {
                Some((name, params)) => (name, Some(params)),
                None => (key_with_params, None),
            };

            match field_name {
                "UID" => uid = Some(value.to_string()),
                "SUMMARY" => summary = Some(value.to_string()),
                "DESCRIPTION" => description = Some(value.to_string()),
                "DTSTART" => dtstart = Some(parse_datetime_with_params(value, params)?),
                "DTEND" => dtend = Some(parse_datetime_with_params(value, params)?),
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

/// Generate VEVENT in iCalendar format with VTIMEZONE component
/// Uses DTSTART;TZID=X format for proper timezone handling in calendar apps
pub fn generate_vevent(event: &CalendarEvent, timezone: &str) -> Result<String> {
    let tz: Tz = timezone.parse().unwrap_or_else(|_| {
        tracing::warn!(
            "Invalid timezone '{}', falling back to UTC for calendar events",
            timezone
        );
        chrono_tz::UTC
    });

    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Klyntbot//Calendar//EN".to_string(),
    ];

    // Add VTIMEZONE component for non-UTC timezones
    if timezone != "UTC" {
        lines.extend(generate_vtimezone(timezone, event.start.year()));
    }

    lines.push("BEGIN:VEVENT".to_string());
    lines.push(format!("UID:{}", event.uid));
    lines.push(format!("SUMMARY:{}", event.summary));

    if let Some(desc) = &event.description {
        lines.push(format!("DESCRIPTION:{}", desc));
    }

    // Format with TZID for non-UTC, or Z suffix for UTC
    if timezone == "UTC" {
        lines.push(format!("DTSTART:{}", event.start.format("%Y%m%dT%H%M%SZ")));
        lines.push(format!("DTEND:{}", event.end.format("%Y%m%dT%H%M%SZ")));
    } else {
        let start_local = event.start.with_timezone(&tz);
        let end_local = event.end.with_timezone(&tz);
        lines.push(format!(
            "DTSTART;TZID={}:{}",
            timezone,
            start_local.format("%Y%m%dT%H%M%S")
        ));
        lines.push(format!(
            "DTEND;TZID={}:{}",
            timezone,
            end_local.format("%Y%m%dT%H%M%S")
        ));
    }

    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());

    Ok(lines.join("\r\n"))
}

/// Parse iCalendar datetime with optional parameters (TZID, VALUE=DATE, etc.)
fn parse_datetime_with_params(dt_str: &str, params: Option<&str>) -> Result<DateTime<Utc>> {
    // Check for TZID parameter (e.g., "TZID=America/New_York")
    if let Some(params_str) = params {
        if let Some(tzid) = params_str.strip_prefix("TZID=") {
            return parse_datetime_in_tz(dt_str, tzid);
        }
    }

    // No params or no TZID — use existing UTC/floating time parsing
    parse_datetime_utc(dt_str)
}

/// Parse a datetime string as a specific timezone, convert to UTC
fn parse_datetime_in_tz(dt_str: &str, tzid: &str) -> Result<DateTime<Utc>> {
    let tz: Tz = tzid.parse().map_err(|_| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(format!(
            "Unknown timezone: {}",
            tzid
        )))
    })?;

    let naive = parse_naive_datetime(dt_str)?;

    tz.from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| {
            common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
                "Invalid or ambiguous local datetime".to_string(),
            ))
        })
}

/// Parse iCalendar datetime as UTC (handles both YYYYMMDDTHHMMSSZ and floating time)
fn parse_datetime_utc(dt_str: &str) -> Result<DateTime<Utc>> {
    let dt_str = dt_str.trim_end_matches('Z');
    let naive = parse_naive_datetime(dt_str)?;

    Utc.from_local_datetime(&naive).single().ok_or_else(|| {
        common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
            "Invalid datetime".to_string(),
        ))
    })
}

/// Parse YYYYMMDDTHHMMSS into NaiveDateTime
fn parse_naive_datetime(dt_str: &str) -> Result<NaiveDateTime> {
    if dt_str.len() != 15 || dt_str.as_bytes().get(8) != Some(&b'T') {
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

    NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_opt(hour, minute, second))
        .ok_or_else(|| {
            common::KlyntbotError::Calendar(common::CalendarError::ProtocolError(
                "Invalid datetime components".to_string(),
            ))
        })
}

/// Generate a minimal VTIMEZONE component for RFC 5545 compliance.
/// Includes STANDARD and DAYLIGHT subcomponents based on the timezone's rules.
fn generate_vtimezone(tzid: &str, year: i32) -> Vec<String> {
    let tz: Tz = match tzid.parse() {
        Ok(tz) => tz,
        Err(_) => return vec![],
    };

    // Calculate UTC offset for January 1 (standard) and July 1 (potential daylight)
    let jan1 = NaiveDate::from_ymd_opt(year, 1, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let jul1 = NaiveDate::from_ymd_opt(year, 7, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();

    let jan_offset: FixedOffset = tz
        .from_local_datetime(&jan1)
        .earliest()
        .map(|dt| dt.offset().fix())
        .unwrap_or(FixedOffset::east_opt(0).unwrap());
    let jul_offset: FixedOffset = tz
        .from_local_datetime(&jul1)
        .earliest()
        .map(|dt| dt.offset().fix())
        .unwrap_or(FixedOffset::east_opt(0).unwrap());

    let mut lines = vec!["BEGIN:VTIMEZONE".to_string(), format!("TZID:{}", tzid)];

    let format_utc_offset = |secs: i32| -> String {
        let sign = if secs >= 0 { '+' } else { '-' };
        let abs = secs.unsigned_abs();
        let hours = abs / 3600;
        let minutes = (abs % 3600) / 60;
        format!("{}{:02}{:02}", sign, hours, minutes)
    };

    let jan_secs = jan_offset.local_minus_utc();
    let jul_secs = jul_offset.local_minus_utc();

    if jan_secs == jul_secs {
        // No DST — single STANDARD component
        lines.push("BEGIN:STANDARD".to_string());
        lines.push(format!("DTSTART:{}0101T000000", year));
        lines.push(format!("TZOFFSETFROM:{}", format_utc_offset(jan_secs)));
        lines.push(format!("TZOFFSETTO:{}", format_utc_offset(jan_secs)));
        lines.push("END:STANDARD".to_string());
    } else {
        // DST exists — add both STANDARD and DAYLIGHT
        // Northern hemisphere: Jan=standard, Jul=daylight
        // Southern hemisphere: Jan=daylight, Jul=standard
        let (std_secs, dst_secs) = if jan_secs < jul_secs {
            (jan_secs, jul_secs)
        } else {
            (jul_secs, jan_secs)
        };

        lines.push("BEGIN:STANDARD".to_string());
        lines.push(format!("DTSTART:{}1101T020000", year));
        lines.push(format!("TZOFFSETFROM:{}", format_utc_offset(dst_secs)));
        lines.push(format!("TZOFFSETTO:{}", format_utc_offset(std_secs)));
        lines.push("END:STANDARD".to_string());

        lines.push("BEGIN:DAYLIGHT".to_string());
        lines.push(format!("DTSTART:{}0301T020000", year));
        lines.push(format!("TZOFFSETFROM:{}", format_utc_offset(std_secs)));
        lines.push(format!("TZOFFSETTO:{}", format_utc_offset(dst_secs)));
        lines.push("END:DAYLIGHT".to_string());
    }

    lines.push("END:VTIMEZONE".to_string());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CalendarEvent, EventSource};
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_parse_simple_vevent_utc() {
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
    fn test_parse_vevent_with_tzid() {
        let ical_data = "\
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:tz-event-1
SUMMARY:Bangkok Meeting
DTSTART;TZID=Asia/Bangkok:20260215T140000
DTEND;TZID=Asia/Bangkok:20260215T150000
END:VEVENT
END:VCALENDAR";

        let event = parse_vevent(ical_data).unwrap();

        assert_eq!(event.uid, "tz-event-1");
        assert_eq!(event.summary, "Bangkok Meeting");
        // 2pm Bangkok (UTC+7) = 7am UTC
        assert_eq!(
            event.start,
            Utc.with_ymd_and_hms(2026, 2, 15, 7, 0, 0).unwrap()
        );
        assert_eq!(
            event.end,
            Utc.with_ymd_and_hms(2026, 2, 15, 8, 0, 0).unwrap()
        );
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
    fn test_generate_vevent_with_timezone() {
        let event = CalendarEvent {
            uid: "test-event-789".to_string(),
            summary: "Standup Meeting".to_string(),
            description: Some("Daily team sync".to_string()),
            start: Utc.with_ymd_and_hms(2026, 3, 1, 2, 0, 0).unwrap(), // 2am UTC = 9am Bangkok
            end: Utc.with_ymd_and_hms(2026, 3, 1, 2, 15, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: None,
        };

        let ical_data = generate_vevent(&event, "Asia/Bangkok").unwrap();

        assert!(ical_data.contains("BEGIN:VCALENDAR"));
        assert!(ical_data.contains("BEGIN:VTIMEZONE"));
        assert!(ical_data.contains("TZID:Asia/Bangkok"));
        assert!(ical_data.contains("END:VTIMEZONE"));
        assert!(ical_data.contains("UID:test-event-789"));
        assert!(ical_data.contains("SUMMARY:Standup Meeting"));
        assert!(ical_data.contains("DESCRIPTION:Daily team sync"));
        assert!(ical_data.contains("DTSTART;TZID=Asia/Bangkok:20260301T090000"));
        assert!(ical_data.contains("DTEND;TZID=Asia/Bangkok:20260301T091500"));
        assert!(ical_data.contains("END:VEVENT"));
        assert!(ical_data.contains("END:VCALENDAR"));
    }

    #[test]
    fn test_generate_vevent_utc() {
        let event = CalendarEvent {
            uid: "utc-event".to_string(),
            summary: "UTC Meeting".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 3, 1, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: None,
        };

        let ical_data = generate_vevent(&event, "UTC").unwrap();

        assert!(!ical_data.contains("BEGIN:VTIMEZONE"));
        assert!(ical_data.contains("DTSTART:20260301T090000Z"));
        assert!(ical_data.contains("DTEND:20260301T100000Z"));
    }

    #[test]
    fn test_roundtrip_parse_generate_with_timezone() {
        let original = CalendarEvent {
            uid: "roundtrip-tz".to_string(),
            summary: "TZ Roundtrip".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 4, 15, 5, 0, 0).unwrap(), // 5am UTC = noon Bangkok
            end: Utc.with_ymd_and_hms(2026, 4, 15, 6, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: None,
        };

        let ical_data = generate_vevent(&original, "Asia/Bangkok").unwrap();
        let parsed = parse_vevent(&ical_data).unwrap();

        assert_eq!(parsed.uid, original.uid);
        assert_eq!(parsed.summary, original.summary);
        assert_eq!(parsed.start, original.start);
        assert_eq!(parsed.end, original.end);
    }

    #[test]
    fn test_roundtrip_utc() {
        let original = CalendarEvent {
            uid: "roundtrip-utc".to_string(),
            summary: "UTC Roundtrip".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 4, 15, 13, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: None,
        };

        let ical_data = generate_vevent(&original, "UTC").unwrap();
        let parsed = parse_vevent(&ical_data).unwrap();

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

    #[test]
    fn test_vtimezone_no_dst() {
        // Asia/Bangkok has no DST
        let lines = generate_vtimezone("Asia/Bangkok", 2026);
        let vtimezone = lines.join("\r\n");

        assert!(vtimezone.contains("BEGIN:VTIMEZONE"));
        assert!(vtimezone.contains("TZID:Asia/Bangkok"));
        assert!(vtimezone.contains("BEGIN:STANDARD"));
        assert!(vtimezone.contains("TZOFFSETTO:+0700"));
        assert!(!vtimezone.contains("BEGIN:DAYLIGHT"));
        assert!(vtimezone.contains("END:VTIMEZONE"));
    }

    #[test]
    fn test_vtimezone_with_dst() {
        // America/New_York has DST
        let lines = generate_vtimezone("America/New_York", 2026);
        let vtimezone = lines.join("\r\n");

        assert!(vtimezone.contains("BEGIN:VTIMEZONE"));
        assert!(vtimezone.contains("TZID:America/New_York"));
        assert!(vtimezone.contains("BEGIN:STANDARD"));
        assert!(vtimezone.contains("BEGIN:DAYLIGHT"));
        assert!(vtimezone.contains("END:VTIMEZONE"));
    }

    #[test]
    fn test_parse_unknown_tzid_fails() {
        let ical_data = "\
BEGIN:VCALENDAR
BEGIN:VEVENT
UID:bad-tz
SUMMARY:Bad TZ
DTSTART;TZID=Invalid/Zone:20260215T140000
DTEND;TZID=Invalid/Zone:20260215T150000
END:VEVENT
END:VCALENDAR";

        let result = parse_vevent(ical_data);
        assert!(result.is_err());
    }
}
