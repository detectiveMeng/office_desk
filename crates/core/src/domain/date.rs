//! 日期工具：一周从周一开始。

use chrono::{Datelike, Duration, NaiveDate};

/// 本周一
pub fn start_of_week(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_monday() as i64)
}

/// 本周日
pub fn end_of_week(day: NaiveDate) -> NaiveDate {
    start_of_week(day) + Duration::days(6)
}

/// 明天
pub fn tomorrow(day: NaiveDate) -> NaiveDate {
    day + Duration::days(1)
}

/// d 是否落在以 week_start 为起点的这一周（含首尾）
pub fn in_week(d: NaiveDate, week_start: NaiveDate) -> bool {
    d >= week_start && d <= end_of_week(week_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn week_starts_monday() {
        // 2026-10-21 是周三，本周一应为 10-19
        let wed = day(2026, 10, 21);
        assert_eq!(start_of_week(wed), day(2026, 10, 19));
        assert_eq!(end_of_week(wed), day(2026, 10, 25));
    }

    #[test]
    fn in_week_range() {
        let ws = day(2026, 10, 19);
        assert!(in_week(day(2026, 10, 19), ws));
        assert!(in_week(day(2026, 10, 25), ws));
        assert!(!in_week(day(2026, 10, 26), ws));
        assert!(!in_week(day(2026, 10, 18), ws));
    }
}