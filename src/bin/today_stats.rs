use std::rc::Rc;
use std::path::PathBuf;
use std::time::SystemTime;
use chrono::prelude::*;

use netupi::db;
use netupi::time;
use netupi::task::*;

pub fn main() -> anyhow::Result<()>{

    let mut default_config_dir = dirs::config_dir().unwrap_or(PathBuf::new());
    default_config_dir.push("netupi");

    let conn = db::init(default_config_dir)?;
    let db = Rc::new(conn);

    let (tasks, _tags) = db::get_tasks(db.clone())?;
    let records = db::get_time_records(db.clone(),
        &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        &DateTime::from(SystemTime::now()))?;

    let mut task_sums = TaskSums::new();

    for (uid, _) in &tasks {
        let sum = build_time_prefix_sum(&tasks, &records, uid.clone(), &TimeRecordSet::new());
        task_sums.insert(uid.clone(), sum);
    }

    let aggregate = time::get_durations(&task_sums);
    println!("{:>12}", time::format_duration(&aggregate.day));
    Ok(())
}
