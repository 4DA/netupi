use std::rc::Rc;
use std::iter::FromIterator;

use druid::im::{vector, Vector, ordset, OrdSet, OrdMap, HashMap};
use serde::ser::{Serialize, Serializer, SerializeSeq, SerializeMap};

use rusqlite::{
    params,
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
    Connection, ToSql, Transaction, Result, NO_PARAMS
};

use anyhow::{anyhow, Context};

use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use crate::task::*;


/// Wrapper over `chrono::DateTime<Utc>`. In SQL, it's stored as an integer number of seconds since
/// January 1, 1970.
struct UnixTimestamp(DateTime<Utc>);

impl ToSql for UnixTimestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.timestamp()))
    }
}

impl FromSql for UnixTimestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let t = value.as_i64()?;
        let t = NaiveDateTime::from_timestamp(t, 0);
        let t = DateTime::<Utc>::from_utc(t, Utc);
        let t = UnixTimestamp(t);
        Ok(t)
    }
}

pub fn init() -> anyhow::Result<Connection>{
    let conn = Connection::open("time_tracker.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
             uid text primary key,
             name text not null,
             description text,
             tags text,
             priority integer,
             status text,
             seq integer
         )",
        NO_PARAMS,
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS time_records (
             ts_from INTEGER PRIMARY KEY,
             ts_to INTEGER,
             uid TEXT NOT NULL
         )",
        NO_PARAMS,
    )?;

    Ok(conn)
}

pub fn add_task(conn: Rc<Connection>, task: &Task) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO tasks (uid, name, description, tags, priority, status, seq) values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        &[&task.uid, &task.name, &task.description,
          &serde_json::to_string(&Wrapper::new(&task.tags)).unwrap(),
          &task.priority.to_string(), &serde_json::to_string(&task.task_status).unwrap(),
          &task.seq.to_string()],
    )?;

    println!("insert ok | t: {:?}", &task);

    Ok(())
}

pub fn update_task(conn: Rc<Connection>, task: &Task) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE tasks SET name = ?1, description = ?2, tags = ?3, priority = ?4, status = ?5, seq = ?6 WHERE uid = ?7;",
        &[&task.name, &task.description,
          &serde_json::to_string(&Wrapper::new(&task.tags)).unwrap(),
          &task.priority.to_string(), &serde_json::to_string(&task.task_status).unwrap(),
          &task.seq.to_string(), &task.uid],
    )?;

    println!("update ok | t: {:?}", &task);

    Ok(())
}

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

    let sqtasks = stmt.query_map(NO_PARAMS, |row| {
        let stat_str: String = row.get(5)?;
        let tag_str: String = row.get(3)?;
        let arr = serde_json::from_str::<Vec<String>>(&tag_str).unwrap();
        let mut tag_set = OrdSet::new();

        for x in arr {
            tag_set = tag_set.update(x);
        }

        Ok(Task {
            name         : row.get(1)?,
            description  : row.get(2)?,
            uid          : row.get(0)?,
            tags         : tag_set,
            priority     : row.get::<usize, u32>(4)?,
            task_status  : serde_json::from_str::<TaskStatus>(&stat_str).unwrap(),
            seq          : row.get::<usize, u32>(6)?,
            time_records : Vector::new()
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
        params![UnixTimestamp(*record.from), UnixTimestamp(*record.to), record.uid],
    )?;

    println!("time record insert ok | t: {:?}", &record);

    Ok(())
}
