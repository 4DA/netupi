use std::ops::Add;
use chrono::Duration;
use chrono::prelude::*;
use crate::task::*;

pub fn format_duration(dur: &chrono::Duration) -> String {
    let mut empty = 0;
    let days = if dur.num_days() > 0 {
        format!("{}d", dur.num_days())
    } else {empty += 1; "".to_string()};

    let hours = if dur.num_hours() > 0 {
        format!("{}{}h", if empty == 1 {""} else {" "},dur.num_hours() % 24)
    } else {empty += 1;"".to_string()};

    let mins = if dur.num_minutes() > 0 {
        format!("{}{}m", if empty == 2 {""} else {" "},dur.num_minutes() % 60)
    } else {empty += 1;"".to_string()};

    let seconds = if dur.num_seconds() > 0 && dur.num_seconds() % 60 != 0 {
        format!("{}{}s", if empty == 3 {""} else {" "},dur.num_seconds() % 60)
    } else {empty += 1; "".to_string()};

    if empty == 4 {
        "--".to_string()
    } else {
        format!("{}{}{}{}", days, hours, mins, seconds)
    }
}

pub struct AggregateDuration {
    pub day: Duration,
    pub week:  Duration,
    pub month: Duration,
    pub year:  Duration,
    pub total: Duration,
}

impl AggregateDuration {
    fn zero() -> AggregateDuration {
        return AggregateDuration {
        day: Duration::zero(), week: Duration::zero(), month: Duration::zero(),
        year: Duration::zero(), total: Duration::zero(),
        }
    }
}

impl Add for AggregateDuration {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {day: self.day + other.day,
              week: self.week + other.week,
              month: self.month + other.month,
              year: self.year + other.year,
              total: self.total + other.total,
        }
    }
}

pub fn daystart(src: DateTime<Local>) -> DateTime<Utc>
{
    DateTime::<Utc>::from(src.with_hour(0).unwrap()
              .with_minute(0).unwrap()
              .with_second(0).unwrap())
}

pub fn get_duration(sum: &TimePrefixSum, now: &DateTime<Local>) -> AggregateDuration
{
    let day_start: DateTime<Utc> = DateTime::from(now.date().and_hms(0, 0, 0));
    let epoch = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);

    let day = get_total_time(sum, &day_start, &Utc::now());

    let total = get_total_time(sum, &epoch, &Utc::now());

    let week = get_total_time(sum,
        &Local.isoywd(now.year(), now.iso_week().week(), Weekday::Mon)
        .and_hms(0, 0, 0).with_timezone(&Utc), &Utc::now());

    let month = get_total_time(sum,
        &Local.ymd(now.year(), now.month(), 1)
                .and_hms(0, 0, 0).with_timezone(&Utc), &Utc::now());

    let year = get_total_time(sum,
        &Local.ymd(now.year(), 1, 1)
            .and_hms(0, 0, 0).with_timezone(&Utc), &Utc::now());

    return AggregateDuration{day, week, month, year, total};

}

pub fn get_durations(task_sums: &TaskSums) -> AggregateDuration {
    let now = Local::now();

    let mut result = AggregateDuration::zero();

    for (_, sum) in task_sums {
        result = result + get_duration(sum, &now);
    }

    return result;
}
