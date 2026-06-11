use serde::{Deserialize, Serialize};

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
    let interval_days = if frequency == "biweekly" { 14 } else { 7 };
    let hard_limit = occurrences
        .map(|o| o.max(2).min(52))
        .unwrap_or(52)
        as usize;
    let until_boundary = until_date.and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
    });

    let start_dt = match chrono::NaiveDateTime::parse_from_str(starts_at, "%Y-%m-%dT%H:%M:%S") {
        Ok(dt) => dt,
        Err(_) => match chrono::NaiveDateTime::parse_from_str(
            &format!("{}T00:00:00", &starts_at[..10]),
            "%Y-%m-%dT%H:%M:%S",
        ) {
            Ok(dt) => dt,
            Err(_) => return vec![],
        },
    };
    let end_dt = match chrono::NaiveDateTime::parse_from_str(ends_at, "%Y-%m-%dT%H:%M:%S") {
        Ok(dt) => dt,
        Err(_) => match chrono::NaiveDateTime::parse_from_str(
            &format!("{}T00:00:00", &ends_at[..10]),
            "%Y-%m-%dT%H:%M:%S",
        ) {
            Ok(dt) => dt,
            Err(_) => return vec![],
        },
    };

    let duration = end_dt - start_dt;
    let mut results = Vec::new();
    let mut index = 0;

    while results.len() < hard_limit {
        let next_start = start_dt
            .checked_add_signed(chrono::Duration::days(interval_days * index))
            .unwrap_or(start_dt);
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

pub fn get_series_label(frequency: &str) -> &str {
    match frequency {
        "biweekly" => "Quinzenal",
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
            "2024-01-01T09:00:00",
            "2024-01-01T10:00:00",
            "weekly",
            None,
            Some(2),
        );
        assert!(results.len() >= 2);
    }
}
