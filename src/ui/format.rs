use chrono::{DateTime, Datelike, Local, Timelike};

#[must_use]
pub fn format_relative_time(iso_str: &str) -> String {
    let dt = match DateTime::parse_from_rfc3339(iso_str) {
        Ok(dt) => dt.with_timezone(&Local),
        Err(_) => return iso_str.to_string(),
    };

    let now = Local::now();
    let duration = now.signed_duration_since(dt);
    let seconds = duration.num_seconds();

    if seconds < 0 {
        return "just now".to_string();
    }

    if seconds < 3600 {
        let mins = seconds / 60;
        return format!("{mins} minutes ago");
    }

    let today = now.date_naive();
    let dt_date = dt.date_naive();

    if dt_date == today {
        let hours = duration.num_hours();
        let mins = duration.num_minutes() % 60;
        return format!("{hours} hours {mins} minutes ago");
    }

    if dt_date == today.pred_opt().unwrap_or(today) {
        return format!("Yesterday {:02}:{:02}", dt.hour(), dt.minute());
    }

    let days_ago = duration.num_days();
    if days_ago <= 3 {
        return format!("On {}, {:02}:{:02}", dt.weekday(), dt.hour(), dt.minute());
    }

    if days_ago <= 6 {
        return format!("On {}", dt.weekday());
    }

    format!("{days_ago} days ago")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn test_format_relative_time() {
        let now = Local::now();

        // < 60 mins
        let t1 = (now - Duration::minutes(5)).to_rfc3339();
        assert_eq!(format_relative_time(&t1), "5 minutes ago");

        // Earlier today
        let t2 = (now - Duration::hours(2) - Duration::minutes(10)).to_rfc3339();
        // This depends on whether it crossed midnight, but for test usually fine
        if (now - Duration::hours(2)).date_naive() == now.date_naive() {
            assert_eq!(format_relative_time(&t2), "2 hours 10 minutes ago");
        }

        // Yesterday
        let yesterday = now - Duration::days(1);
        let t3 = Local.from_local_datetime(&yesterday.naive_local()).unwrap().to_rfc3339();
        let expected_yesterday =
            format!("Yesterday {:02}:{:02}", yesterday.hour(), yesterday.minute());
        assert_eq!(format_relative_time(&t3), expected_yesterday);

        // 3 days ago
        let three_days = now - Duration::days(3);
        let t4 = Local.from_local_datetime(&three_days.naive_local()).unwrap().to_rfc3339();
        let expected_3d = format!(
            "On {}, {:02}:{:02}",
            three_days.weekday(),
            three_days.hour(),
            three_days.minute()
        );
        assert_eq!(format_relative_time(&t4), expected_3d);

        // 6 days ago
        let six_days = now - Duration::days(6);
        let t5 = Local.from_local_datetime(&six_days.naive_local()).unwrap().to_rfc3339();
        let expected_6d = format!("On {}", six_days.weekday());
        assert_eq!(format_relative_time(&t5), expected_6d);

        // 10 days ago
        let ten_days = now - Duration::days(10);
        let t6 = Local.from_local_datetime(&ten_days.naive_local()).unwrap().to_rfc3339();
        assert_eq!(format_relative_time(&t6), "10 days ago");
    }
}
