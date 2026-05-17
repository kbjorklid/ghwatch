use chrono::{DateTime, Local};

#[must_use]
pub fn format_relative_time(iso_str: &str) -> String {
    let dt = match DateTime::parse_from_rfc3339(iso_str) {
        Ok(dt) => dt.with_timezone(&Local),
        Err(_) => return iso_str.to_string(),
    };

    let now = Local::now();
    let duration = now.signed_duration_since(dt);
    let seconds = duration.num_seconds().max(0);

    if seconds < 60 {
        return "now".to_string();
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = minutes / 60;
    let rem_minutes = minutes % 60;
    if hours < 24 {
        return format!("{hours}h {rem_minutes}m");
    }

    let days = hours / 24;
    let rem_hours = hours % 24;
    if days < 7 {
        return format!("{days}d {rem_hours}h");
    }

    format!("{days}d")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_format_relative_time_now() {
        let now = Local::now();
        let t = (now - Duration::seconds(30)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "now");
    }

    #[test]
    fn test_format_relative_time_future_shows_now() {
        let now = Local::now();
        let t = (now + Duration::seconds(10)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "now");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let now = Local::now();
        let t = (now - Duration::minutes(5)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "5m");
    }

    #[test]
    fn test_format_relative_time_hours_and_minutes() {
        let now = Local::now();
        let t = (now - Duration::hours(2) - Duration::minutes(10)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "2h 10m");
    }

    #[test]
    fn test_format_relative_time_days_and_hours() {
        let now = Local::now();
        let t = (now - Duration::days(3) - Duration::hours(5)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "3d 5h");
    }

    #[test]
    fn test_format_relative_time_many_days() {
        let now = Local::now();
        let t = (now - Duration::days(10)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "10d");
    }

    #[test]
    fn test_format_relative_time_exactly_7_days() {
        let now = Local::now();
        let t = (now - Duration::days(7)).to_rfc3339();
        assert_eq!(format_relative_time(&t), "7d");
    }
}
