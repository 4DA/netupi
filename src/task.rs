use std::ops::Add;
use std::rc::Rc;
use druid::lens::{self, LensExt};
use druid::{Data, Lens};
use chrono::prelude::*;
use chrono::Duration;
use druid::im::{vector, Vector, ordset, OrdSet, OrdMap, HashMap};
use serde::{Serialize, Serializer, Deserialize};
use serde::ser::{SerializeSeq, SerializeMap};

pub type TagSet        = OrdSet<String>;
pub type TaskMap       = OrdMap<String, Task>;
pub type TimeRecordMap = OrdMap<DateTime<Utc>, TimeRecord>;
pub type TimePrefixSum = OrdMap<DateTime<Utc>, TimePrefix>;
pub type TaskSums      = OrdMap::<String, TimePrefixSum>;

#[derive(Debug, Clone, Data, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    NEEDS_ACTION,
    COMPLETED,
    IN_PROCESS,
    CANCELLED,
    ARCHIVED
}

impl TaskStatus {
    fn to_string(&self) -> &str {
        match &self {
            TaskStatus::NEEDS_ACTION => "Needs action",
            TaskStatus::COMPLETED    => "Completed",
            TaskStatus::IN_PROCESS   => "In process",
            TaskStatus::CANCELLED    => "Cancelled",
            TaskStatus::CANCELLED    => "Archived",
            _ => {panic!("Unknown status {:?}", self);}
        }
    }
}

#[derive(Debug, Clone, Data, Lens)]
pub struct Task {
    pub name: String,
    pub description: String,
    pub uid: String,
    pub tags: TagSet,
    pub priority: u32,
    pub task_status: TaskStatus,
    pub seq: u32,
}

#[derive(Debug, Clone, Data)]
pub struct TimeRecord {
    pub from: Rc<DateTime<Utc>>,
    pub to: Rc<DateTime<Utc>>,
    pub uid: String
}

impl TimeRecord {
    fn duration(&self) -> chrono::Duration {
        self.to.signed_duration_since(*self.from)
    }
}

impl Task {
    pub fn new(name: String, description: String,
           uid: String, tags: OrdSet<String>,
           priority: u32, task_status: TaskStatus, seq: u32) -> Task {
        return Task{name, description, uid, tags, priority, task_status, seq};
    }
}

#[derive(Debug, Clone, Data, PartialEq)]
pub struct TimePrefix {
    duration: Rc<chrono::Duration>,
}

impl TimePrefix {
    pub fn new(duration: &chrono::Duration) -> TimePrefix {
        TimePrefix{duration: Rc::new(duration.clone())}
    }
}

impl Add for TimePrefix {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        TimePrefix{duration: Rc::new(*self.duration + *other.duration)}
    }
}


pub struct Wrapper{
    inner: OrdSet<String>
}

impl Wrapper {
    pub fn new(os: &OrdSet<String>) -> Wrapper {
        return Wrapper{inner: os.clone()};
    }
}

impl Serialize for Wrapper
where
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.inner.len()))?;
        for e in &self.inner {
            seq.serialize_element(&e)?;
        }
        seq.end()
    }
}

pub enum PrefixSumFilter {
    ALL,
    TASK(String)
}

pub fn get_total_time(prefix_sum: &TimePrefixSum, from: &DateTime::<Utc>)
                      -> chrono::Duration
{
    let (before, after) = prefix_sum.split(from);
    match (before.get_max(), after.get_max()) {
        (Some(min), Some(max)) => *max.1.duration - *min.1.duration,
        (None, Some(max)) => *max.1.duration,
            _ => Duration::zero(),
    }
}

pub fn get_total_time_from_sums(sums: &TaskSums, from: &DateTime::<Utc>) -> chrono::Duration
{
    let mut result = Duration::zero();

    for (_, s) in sums {
        result = result + get_total_time(&s, from)
    }

    return result;
}

pub fn add_record_to_sum(sum_map: &mut TimePrefixSum, record: &TimeRecord) {
    let last = match sum_map.get_max() {
        Some(ref max) => max.1.clone() + TimePrefix::new(&record.duration()),
        None => TimePrefix::new(&chrono::Duration::zero()),
    };

    sum_map.insert(*record.from, last);
}

pub fn build_time_prefix_sum(tasks: &TaskMap, records: &TimeRecordMap, filter: String)
                             -> TimePrefixSum
{
    let mut result = TimePrefixSum::new();
    let epoch_0 = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
    result.insert(epoch_0, TimePrefix::new(&Duration::zero()));

    let mut psum = Duration::zero();

    for (k, v) in records {
        if filter.eq(&v.uid) {
            psum = psum + v.to.signed_duration_since(*v.from);
            result.insert(*v.from, TimePrefix::new(&psum));
        }
    }

    return result;
}
