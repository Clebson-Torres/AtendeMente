use serde::{Deserialize, Serialize};
use chrono::Datelike;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringAppointment {
    pub starts_at: String,
    pub ends_at: String,
}

/// Build recurring appointments from a base date/time
pub fn build_recurring_appointments(
    starts_at: &str,
    ends_at: &str,
    frequency: &str,
    until_date: Option<&str>,
    occurrences: Option<i32>,
) -> Vec<RecurringAppointment> {
    let hard_limit = occurrences
        .map(|o| o.max(2).min(52))
        .unwrap_or(52)
        as usize;
    let until_boundary = until_date.and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
    });

    let start_dt = match chrono::NaiveDateTime::parse_from_str(starts_at, "%Y-%m-%dT%H:%M:%S") {
        Ok(dt) => dt,
        Err(_) => match chrono::NaiveDateTime::parse_from_str(starts_at, "%Y-%m-%dT%H:%M") {
            Ok(dt) => dt,
            Err(_) => return vec![],
        },
    };
    let end_dt = match chrono::NaiveDateTime::parse_from_str(ends_at, "%Y-%m-%dT%H:%M:%S") {
        Ok(dt) => dt,
        Err(_) => match chrono::NaiveDateTime::parse_from_str(ends_at, "%Y-%m-%dT%H:%M") {
            Ok(dt) => dt,
            Err(_) => return vec![],
        },
    };

    let duration = end_dt - start_dt;
    let mut results = Vec::new();
    let mut index = 0;

    while results.len() < hard_limit {
        let next_start = if frequency == "monthly" {
            add_months(start_dt, index)
        } else {
            let interval_days = if frequency == "biweekly" { 14 } else { 7 };
            start_dt
                .checked_add_signed(chrono::Duration::days(interval_days as i64 * index as i64))
                .unwrap_or(start_dt)
        };
        let next_end = next_start + duration;

        if let Some(boundary) = until_boundary {
            if next_start.date() > boundary {
                break;
            }
        }

        results.push(RecurringAppointment {
            starts_at: next_start.format("%Y-%m-%dT%H:%M:%S").to_string(),
            ends_at: next_end.format("%Y-%m-%dT%H:%M:%S").to_string(),
        });

        index += 1;

        if until_boundary.is_none() {
            if let Some(occ) = occurrences {
                if results.len() >= occ as usize {
                    break;
                }
            }
        }
    }

    results
}

fn add_months(dt: chrono::NaiveDateTime, months: i32) -> chrono::NaiveDateTime {
    let total_months = dt.month() as i32 - 1 + months;
    let new_year = dt.year() + total_months / 12;
    let new_month = ((total_months % 12) + 1) as u32;
    let max_day = num_days_in_month(new_year, new_month);
    let new_day = dt.day().min(max_day);
    let new_date = chrono::NaiveDate::from_ymd_opt(new_year, new_month, new_day)
        .unwrap_or(dt.date());
    chrono::NaiveDateTime::new(new_date, dt.time())
}

fn num_days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

pub fn get_series_label(frequency: &str) -> &str {
    match frequency {
        "biweekly" => "Quinzenal",
        "monthly" => "Mensal",
        _ => "Semanal",
    }
}

pub fn get_series_summary(
    frequency: &str,
    start_time: &str,
    end_time: &str,
    starts_on: &str,
    ends_on: Option<&str>,
    occurrences_count: Option<i32>,
) -> String {
    let base = format!(
        "{} · {} · {} às {}",
        get_series_label(frequency),
        format_date_br(starts_on),
        start_time,
        end_time
    );

    if let Some(end) = ends_on {
        return format!("{} · até {}", base, format_date_br(end));
    }

    if let Some(count) = occurrences_count {
        return format!("{} · {} sessões", base, count);
    }

    base
}

fn format_date_br(date: &str) -> String {
    if date.len() >= 10 {
        let (y, m, d) = (&date[..4], &date[5..7], &date[8..10]);
        format!("{}/{}/{}", d, m, y)
    } else {
        date.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_recurring_weekly() {
        let results = build_recurring_appointments(
            "2024-01-08T09:00:00",
            "2024-01-08T10:00:00",
            "weekly",
            None,
            Some(4),
        );
        assert_eq!(results.len(), 4);
        assert_eq!(&results[0].starts_at[..10], "2024-01-08");
        assert_eq!(&results[1].starts_at[..10], "2024-01-15");
    }

    #[test]
    fn test_build_recurring_biweekly() {
        let results = build_recurring_appointments(
            "2024-01-08T09:00:00",
            "2024-01-08T10:00:00",
            "biweekly",
            None,
            Some(3),
        );
        assert_eq!(results.len(), 3);
        assert_eq!(&results[1].starts_at[..10], "2024-01-22");
    }

    #[test]
    fn test_recurrence_until_date() {
        let results = build_recurring_appointments(
            "2024-01-01T09:00:00",
            "2024-01-01T10:00:00",
            "weekly",
            Some("2024-01-20"),
            None,
        );
        assert_eq!(results.len(), 3); // days 1, 8, 15
    }

    #[test]
    fn test_recurrence_hard_limit_52() {
        let results = build_recurring_appointments(
            "2024-01-01T09:00:00",
            "2024-01-01T10:00:00",
            "weekly",
            None,
            Some(100),
        );
        assert!(results.len() <= 52);
    }

    #[test]
    fn test_recurrence_min_2() {
        let results = build_recurring_appointments(
            "2024-01-08T09:00:00",
            "2024-01-08T10:00:00",
            "weekly",
            None,
            Some(2),
        );
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_build_recurring_monthly() {
        let results = build_recurring_appointments(
            "2024-01-15T09:00:00",
            "2024-01-15T10:00:00",
            "monthly",
            None,
            Some(3),
        );
        assert_eq!(results.len(), 3);
        assert_eq!(&results[0].starts_at[..10], "2024-01-15");
        assert_eq!(&results[1].starts_at[..10], "2024-02-15");
        assert_eq!(&results[2].starts_at[..10], "2024-03-15");
    }

    #[test]
    fn test_recurrence_monthly_until_date() {
        let results = build_recurring_appointments(
            "2024-01-31T09:00:00",
            "2024-01-31T10:00:00",
            "monthly",
            Some("2024-04-01"),
            None,
        );
        assert_eq!(results.len(), 3);
        // Jan 31, Feb 29 (2024 leap), Mar 31
        assert_eq!(&results[1].starts_at[..10], "2024-02-29");
        assert_eq!(&results[2].starts_at[..10], "2024-03-31");
    }
}
