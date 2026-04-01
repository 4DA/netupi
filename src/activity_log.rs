use std::time::SystemTime;

use chrono::prelude::*;

use crate::task::*;
use crate::time;

pub fn format_time_record(task: &Task, record: &TimeRecord) -> String {
    let name =  format!("{:wid$}", task.name, wid = 11);

    let duration = time::format_duration(
        &record.to.signed_duration_since(*record.from));

    let now: DateTime<Local> = DateTime::from(SystemTime::now());
    let when: DateTime<Local> = DateTime::<Local>::from(*record.from);

    let time = if now.year() > when.year() {
        when.format("%d %b, %y, %H:%M").to_string()
    } else if now.ordinal() > when.ordinal() {
        when.format("%d %b, %H:%M").to_string()
    } else {
        when.format("%H:%M").to_string()
    };

    format!("{} {:<10} {:<10}", name, duration, time)
}
