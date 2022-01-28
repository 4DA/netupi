use std::rc::Rc;
use std::any::type_name;

use std::thread;
use std::io::BufReader;

use anyhow;
use chrono::{DateTime, Utc, NaiveDateTime, Duration};
use druid::im::{HashMap};

use rodio::{Decoder, OutputStream, Sink};

use crate::task::*;

#[allow(unused)]
pub fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}

/// parse csv file with format finish time,duration,name
/// for example: 2021-10-05-19-18,50,Work

pub fn get_csv_entries(path: &str, task_map: &TaskMap)
                       -> anyhow::Result<(TaskMap, TimeRecordMap)>
{
    let mut result_tasks = TaskMap::new();
    let mut result_records = TimeRecordMap::new();

    let mut name2uid = HashMap::<String, String>::new();

    for (uid, task) in task_map {
        name2uid.insert(task.name.clone(), uid.clone());
    }

    let mut rdr = csv::Reader::from_path(path)?;
    for result in rdr.records() {
        let record = result?;

        let to = DateTime::<Utc>::from_utc(
            NaiveDateTime::parse_from_str(&record[0], "%Y-%m-%d-%H-%M")?, Utc);

        let from = to.checked_sub_signed(Duration::minutes(record[1].parse::<i64>()?)).unwrap();
        let name = record[2].to_string();

        let uid = if let Some(uid) = name2uid.get(&name).cloned() {
            uid.clone()
        }
        else {
            let task = Task::new_simple(name);
            let uid = task.uid.clone();
            name2uid.insert(task.name.clone(), task.uid.clone());
            result_tasks.insert(task.uid.clone(), task);
            uid
        };

        result_records.insert(from.clone(), TimeRecord{from: Rc::new(from), to: Rc::new(to), uid});
    }

    Ok((result_tasks, result_records))
}

pub fn play_sound(bytes: &'static [u8], volume: f32) {
    thread::spawn(move || {
        let bytes = std::io::Cursor::new(bytes.clone());
        // Get a output stream handle to the default physical sound device
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        // Load a sound from a file, using a path relative to Cargo.toml
        let buf = BufReader::new(bytes);
        // Decode that sound file into a source
        let source = Decoder::new(buf).unwrap();

        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(volume);
        sink.append(source);

        // The sound plays in a separate thread. This call will block the current thread until the sink
        // has finished playing all its queued sounds.
        // sink.sleep_until_end();

        // The sound plays in a separate audio thread,
        // so we need to keep the main thread alive while it's playing.
        std::thread::sleep(std::time::Duration::from_secs(sink.len() as u64));
    });
}
