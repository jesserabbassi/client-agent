/// Validate a same-day preview in UTC and calculate whole-hour prices in cents.
pub(crate) fn quote(
    date: &str,
    start: i32,
    end: i32,
    rate: i32,
    now: u64,
) -> Result<i32, &'static str> {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return Err("Enter a date as YYYY-MM-DD.");
    }
    let year: i64 = date[..4].parse().map_err(|_| "Invalid year.")?;
    let month: i64 = date[5..7].parse().map_err(|_| "Invalid month.")?;
    let day: i64 = date[8..].parse().map_err(|_| "Invalid day.")?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return Err("Choose a valid calendar date."),
    };
    if year < 1970 || day < 1 || day > days {
        return Err("Choose a valid calendar date.");
    }
    if !(0..24).contains(&start) || end <= start || end > 24 {
        return Err("End time must be after start time on the same day.");
    }
    // Gregorian calendar date to days since 1970-01-01.
    let y = year - i64::from(month <= 2);
    let era = y / 400;
    let yoe = y - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let epoch_days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    let start_seconds = epoch_days * 86400 + i64::from(start) * 3600;
    if start_seconds <= 0 || start_seconds as u64 <= now {
        return Err("Choose a future date and start time (UTC).");
    }
    if rate <= 0 {
        return Err("Station pricing is unavailable.");
    }
    rate.checked_mul(end - start)
        .ok_or("Price is out of range.")
}

#[cfg(test)]
mod tests {
    use super::quote;
    #[test]
    fn validates_calendar_and_time_ranges() {
        assert_eq!(quote("2028-02-29", 14, 18, 300, 0), Ok(1200));
        for date in [
            "2027-02-29",
            "2100-02-29",
            "2028-04-31",
            "2028-00-01",
            "2028-01-00",
            "bad",
            "２０２８-01-01",
        ] {
            assert!(quote(date, 14, 18, 300, 0).is_err());
        }
        for (start, end) in [(-1, 4), (24, 25), (14, 14), (18, 14), (14, 25)] {
            assert!(quote("2028-01-01", start, end, 300, 0).is_err());
        }
        assert_eq!(quote("2028-01-01", 23, 24, 300, 0), Ok(300));
        assert!(quote("1970-01-01", 1, 2, 300, 3600).is_err());
        assert!(quote("2028-01-01", 1, 2, 0, 0).is_err());
        assert!(quote("2028-01-01", 0, 24, i32::MAX, 0).is_err());
    }
}
