// extern crate netupi;

// use std::time::SystemTime;
// use chrono::{DateTime, Utc, NaiveDateTime};
use std::rc::Rc;
use std::path::PathBuf;
use anyhow::{anyhow, Context};
use std::env;

// use netupi::task::*;
use netupi::utils::*;
use netupi::db;

pub fn main() -> anyhow::Result<()>{
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        return Err(anyhow!("Usage: import_csv <filename>"));
    }

    let mut default_config_dir = dirs::config_dir().unwrap_or(PathBuf::new());
    default_config_dir.push("netupi");

    let conn = db::init(default_config_dir)?;
    let db = Rc::new(conn);

    let (tasks, _tags) = db::get_tasks(db.clone())?;
    // let records = db::get_time_records(db.clone(),
    //     &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
    //     &DateTime::from(SystemTime::now()))?;

    let (imported_tasks, imported_records) = get_csv_entries(&args[1], &tasks)
        .with_context(|| format!("Importing '{}' failed", &args[1]))?;

    for (_, task) in imported_tasks {
        db::add_task(db.clone(), &task)?;
    }

    for (_, record) in imported_records {
        db::add_time_record(db.clone(), &record)?;
    }

    Ok(())
}

