use std::rc::Rc;

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
        "create table if not exists tasks (
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
        "create table if not exists time_records (
             time integer primary key,
             duration integer not null,
             uid text not null
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
