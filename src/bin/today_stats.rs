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
    let (records, records_killed) = db::get_time_records(db.clone(),
        &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        &DateTime::from(SystemTime::now()))?;

    let mut task_sums = TaskSums::new();

    for (uid, _) in &tasks {
        let sum = build_time_prefix_sum(&tasks, &records, uid.clone(), &records_killed);
        task_sums.insert(uid.clone(), sum);
    }

    let aggregate = time::get_durations(&task_sums);
    let today_time = time::format_duration(&aggregate.day);

    // find last tracked task today
    let now_local: DateTime<Local> = Local::now();
    let day_start: DateTime<Utc> = DateTime::from(now_local.date().and_hms(0, 0, 0));

    let last_task = records.iter().rev()
        .filter(|(ts, _)| **ts >= day_start)
        .filter(|(ts, _)| !records_killed.contains(ts))
        .find_map(|(_, rec)| tasks.get(&rec.uid));

    if let Some(task) = last_task {
        let name = if task.name.len() > 10 {
            format!("{:.10}", task.name)
        } else {
            task.name.clone()
        };
        println!("{} {}", name, today_time.trim());
    } else {
        println!("{}", today_time.trim());
    }
    Ok(())
}
