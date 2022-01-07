use std::rc::Rc;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::fs;

use druid::im::{OrdSet};

use rusqlite::{
    params,
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
    Connection, ToSql
};

use chrono::Duration;

use anyhow;

use chrono::{DateTime, Utc, TimeZone};

use crate::task::*;

struct DurationWrapper(chrono::Duration);

impl ToSql for DurationWrapper {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.num_milliseconds()))
    }
}

impl FromSql for DurationWrapper {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(DurationWrapper(Duration::milliseconds(value.as_i64()?)))
    }
}

struct TimeWrapper(DateTime<Utc>);

impl ToSql for TimeWrapper {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.timestamp_millis()))
    }
}

impl FromSql for TimeWrapper {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let t = Utc.timestamp_millis(value.as_i64()?);
        let t = TimeWrapper(t);
        Ok(t)
    }
}

pub fn init(mut path_buf: PathBuf) -> anyhow::Result<Connection>
{
    let dir = path_buf.to_str().unwrap();
    fs::create_dir_all(dir)?;

    path_buf.push("netupi.db");

    let file_path = path_buf.to_str().unwrap();

    let conn = Connection::open(file_path)?;

    println!("opened db: {:?}", file_path);

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
             uid text primary key,
             seq integer not null,
             name text not null,
             description text not null,
             tags text not null,
             priority integer not null,
             status text not null,
             work_duration integer not null,
             break_duration integer not null,
             color integer not null
         )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS time_records (
             ts_from INTEGER PRIMARY KEY,
             ts_to INTEGER,
             uid TEXT NOT NULL
         )",
        [],
    )?;

    Ok(conn)
}

pub fn add_task(conn: Rc<Connection>, task: &Task) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO tasks (uid, name, description, tags, priority, status, work_duration, break_duration, color, seq) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![&task.uid, &task.name, &task.description,
                &serde_json::to_string(&Wrapper::new(&task.tags)).unwrap(),
                &task.priority.to_string(), &serde_json::to_string(&task.task_status).unwrap(),
                &DurationWrapper(*task.work_duration),
                &DurationWrapper(*task.break_duration),
                task.color.as_rgba_u32(),
                &task.seq.to_string(),
        ],
    )?;

    println!("insert ok | t: {:?}", &task);

    Ok(())
}

pub fn update_task(conn: Rc<Connection>, task: &Task) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE tasks SET name = ?1, description = ?2, tags = ?3, priority = ?4, status = ?5, work_duration = ?6, break_duration = ?7, seq = ?8, color = ?9 WHERE uid = ?10;",
        params![&task.name, &task.description,
                &serde_json::to_string(&Wrapper::new(&task.tags)).unwrap(),
                &task.priority.to_string(), &serde_json::to_string(&task.task_status).unwrap(),
                &DurationWrapper(*task.work_duration), &DurationWrapper(*task.break_duration),
                &task.seq.to_string(), task.color.as_rgba_u32(), &task.uid],
    )?;

    println!("update ok | t: {:?}", &task);

    Ok(())
}

#[allow(unused)]
pub fn delete_task(conn: Rc<Connection>, uid: &String) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM tasks WHERE uid = ?1;",
        &[uid],
    )?;

    println!("delete ok | t: {:?}", uid);

    Ok(())
}

pub fn get_tasks(conn: Rc<Connection>) -> anyhow::Result<(TaskMap, TagSet)>
{
    let mut stmt = conn.prepare(
        "SELECT * FROM tasks;",
    )?;

    let sqtasks = stmt.query_map([], |row| {
        let tag_str: String = row.get(4)?;
        let arr = serde_json::from_str::<Vec<String>>(&tag_str).unwrap();
        let stat_str: String = row.get(6)?;
        let work_duration: DurationWrapper = row.get(7)?;
        let rest_duration: DurationWrapper = row.get(8)?;
        let mut tag_set = OrdSet::new();

        for x in arr {
            tag_set = tag_set.update(x);
        }

        Ok(Task {
            uid            : row.get(0)?,
            seq            : row.get::<usize, u32>(1)?,
            name           : row.get(2)?,
            description    : row.get(3)?,
            tags           : tag_set,
            priority       : row.get::<usize, u32>(5)?,
            task_status    : serde_json::from_str::<TaskStatus>(&stat_str)
                             .unwrap_or(TaskStatus::NeedsAction),
            work_duration  : Rc::new(work_duration.0),
            break_duration : Rc::new(rest_duration.0),
            color          : druid::Color::from_rgba32_u32(row.get::<usize, u32>(9)?)
                             .with_alpha(1.0),
        })
    })?;

    let mut tasks = TaskMap::new();
    let mut tags = TagSet::new();

    for x in sqtasks {
        if let Ok(t) = x {
            for tag in &t.tags {
                tags.insert(tag.clone());
            }

            tasks = tasks.update(t.uid.clone(), t);
        }
    }

    Ok((tasks, tags))
}

pub fn add_time_record(conn: Rc<Connection>, record: &TimeRecord) -> anyhow::Result<()>
{
    conn.execute(
        "INSERT INTO time_records (ts_from, ts_to, uid) VALUES (?1, ?2, ?3)",
        params![TimeWrapper(*record.from), TimeWrapper(*record.to), record.uid],
    )?;

    println!("time record insert ok | t: {:?}", &record);

    Ok(())
}

pub fn remove_time_record(conn: Rc<Connection>, record: &TimeRecord) -> anyhow::Result<()>
{
    conn.execute(
        "DELETE FROM time_records WHERE ts_from = ?1",
        params![TimeWrapper(*record.from)],
    )?;

    println!("remove ok | t: {:?}", &record);

    Ok(())
}

pub fn get_time_records(conn: Rc<Connection>, from: &DateTime<Utc>, to: &DateTime<Utc>)
                        -> anyhow::Result<TimeRecordMap>
{
    let mut stmt = conn.prepare("SELECT * FROM time_records WHERE ts_from >= ?1 AND ts_to < ?2")?;

    let rows = stmt.query_map(params![TimeWrapper(*from), TimeWrapper(*to)],
        |row| {
            let ts_from: TimeWrapper = row.get(0)?;
            let ts_to: TimeWrapper = row.get(1)?;

            Ok(TimeRecord {
                from: Rc::new(ts_from.0),
                to: Rc::new(ts_to.0),
                uid: row.get(2)?
            })
        })?;

    Ok(TimeRecordMap::from_iter(rows.map(|x| (*(x.as_ref().unwrap().from).clone(), x.unwrap()))))
}
