use std::ops::Add;
use std::rc::Rc;
use std::time::SystemTime;

use druid::{Data, Lens};
use chrono::prelude::*;
use chrono::Duration;
use druid::im::{OrdSet, OrdMap};
use serde::{Serialize, Serializer, Deserialize};
use serde::ser::{SerializeSeq};

// uid stuff
use uuid::v1::{Timestamp, Context};
use uuid::Uuid;

pub type TagSet        = OrdSet<String>;
pub type TaskMap       = OrdMap<String, Task>;
pub type TimeRecordMap = OrdMap<DateTime<Utc>, TimeRecord>;
pub type TimePrefixSum = OrdMap<DateTime<Utc>, TimePrefix>;
pub type TaskSums      = OrdMap::<String, TimePrefixSum>;

#[derive(Debug, Clone, Data, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    NeedsAction,
    Completed,
    InProcess,
    Archived
}

impl TaskStatus {
    #[allow(unused)]
    fn to_string(&self) -> &str {
        match &self {
            TaskStatus::NeedsAction => "Needs action",
            TaskStatus::Completed   => "Completed",
            TaskStatus::InProcess   => "In process",
            TaskStatus::Archived    => "Archived",
            _ => {panic!("Unknown status {:?}", self);}
        }
    }
}

#[derive(Debug, Clone, Data, PartialEq)]
pub enum CuaPriority {
    Unspecified,
    Low,
    Normal,
    High,
}

impl From<u32> for CuaPriority {
    fn from(value: u32) -> Self {
        match value {
            1..=4 => CuaPriority::High,
            5 => CuaPriority::Normal,
            6..=9 => CuaPriority::Low,
            _ => CuaPriority::Unspecified,
        }
    }
}

impl From<CuaPriority> for u32 {
    fn from(pri: CuaPriority) -> Self {
        match pri {
            CuaPriority::High => 1,
            CuaPriority::Normal => 5,
            CuaPriority::Low => 9,
            CuaPriority::Unspecified => 0,
        }
    }
}

#[derive(Debug, Clone, Data, Lens)]
pub struct Task {
    pub uid: String,
    pub seq: u32,
    pub name: String,
    pub description: String,
    pub tags: TagSet,
    pub priority: u32,
    pub task_status: TaskStatus,
    pub work_duration: Rc<Duration>,
    pub break_duration: Rc<Duration>,
    pub color: u32,
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
               priority: u32, task_status: TaskStatus,
               work_duration: chrono::Duration,
               break_duration: chrono::Duration,
               seq: u32) -> Task
    {
        return Task{name, description, uid, tags, priority, task_status,
                    work_duration: Rc::new(work_duration),
                    break_duration: Rc::new(break_duration),
                    seq, color: 0};
    }

    pub fn new_simple(name: String) -> Task {
        Task::new(name, "".to_string(), generate_uid(), OrdSet::new(),
                  5, TaskStatus::NeedsAction,
                  chrono::Duration::minutes(50),
                  chrono::Duration::minutes(10), 0)
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

pub fn generate_uid() -> String {
    let context = Context::new(42);
    let epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let ts = Timestamp::from_unix(&context, epoch.as_secs(), epoch.subsec_nanos());
    let uuid = Uuid::new_v1(ts, &[1, 2, 3, 4, 5, 6]).expect("failed to generate UUID");
    return uuid.to_string();
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
    // handle case when  record is older than newest entry in sum_map

    if sum_map.is_empty() {
        let epoch_0 = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
        sum_map.insert(epoch_0, TimePrefix::new(&Duration::zero()));
    }

    let last = match sum_map.get_max() {
        Some(ref max) => max.1.clone() + TimePrefix::new(&record.duration()),
        None => TimePrefix::new(&chrono::Duration::zero()),
    };

    sum_map.insert(*record.from, last);
}

pub fn build_time_prefix_sum(_tasks: &TaskMap, records: &TimeRecordMap, filter: String)
                             -> TimePrefixSum
{
    let mut result = TimePrefixSum::new();
    let epoch_0 = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
    result.insert(epoch_0, TimePrefix::new(&Duration::zero()));

    let mut psum = Duration::zero();

    for (_k, v) in records {
        if filter.eq(&v.uid) {
            psum = psum + v.to.signed_duration_since(*v.from);
            result.insert(*v.from, TimePrefix::new(&psum));
        }
    }

    return result;
}
