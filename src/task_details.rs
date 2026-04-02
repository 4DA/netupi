use chrono::Duration;
use chrono::prelude::*;

use crate::task::*;
use crate::time;

pub fn get_day_time2(sum: &TimePrefixSum, day: i64) -> String {
    let now = time::daystart(Local::now());

    let date = now.checked_sub_signed(Duration::days(day)).unwrap();
    let prev_date = now.checked_sub_signed(Duration::days(day + 1)).unwrap();
    let duration = get_total_time(sum, &prev_date, &date);
    let weekday = date.naive_local().weekday().to_string();
    let day = format!("{}, {}", &weekday, &date.format("%d %b").to_string());

    let mut result = String::new();
    result.push_str(&day);
    result.push_str("    ");
    result.push_str(&format!("{:>8}", &time::format_duration(&duration)));
    return result;
}
