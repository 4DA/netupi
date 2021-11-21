use druid::{Color, WidgetId, Selector};

use core::time::Duration;

pub static TASK_COLOR_BG: Color                 = Color::rgb8(80, 73, 69);
pub static APP_BORDER: Color                    = Color::rgb8(60, 56, 54);
pub static TASK_ACTIVE_COLOR_BG: Color          = Color::rgb8(250, 189, 47);
pub static TASK_REST_COLOR_BG: Color            = Color::rgb8(131, 162, 152);
pub static TASK_PAUSE_COLOR_BG: Color           = Color::rgb8(211, 134, 155);
pub static TASK_FOCUS_BORDER: Color             = Color::rgb8(124, 111, 100);
pub static UI_TIMER_INTERVAL: Duration = Duration::from_secs(1);

pub const COMMAND_TASK_NEW:    Selector            = Selector::new("tcmenu.task_new");
pub const COMMAND_TASK_START:  Selector<String>    = Selector::new("tcmenu.task_start");
pub const COMMAND_TASK_STOP:   Selector            = Selector::new("tcmenu.task_stop");
pub const COMMAND_TASK_PAUSE:   Selector           = Selector::new("tcmenu.task_pause");
pub const COMMAND_TASK_RESUME:   Selector<String>  = Selector::new("tcmenu.task_resume");
pub const COMMAND_TASK_ARCHIVE: Selector<String>   = Selector::new("tcmenu.task_archive");
pub const COMMAND_TASK_COMPLETED: Selector<String> = Selector::new("tcmenu.task_completed");

pub const COMMAND_TLIST_REQUEST_FOCUS: Selector    = Selector::new("tlist_request_focus");
pub const COMMAND_DETAILS_REQUEST_FOCUS: Selector  = Selector::new("tdetails_request_focus");


pub const TASK_FOCUS_CURRENT: &str   = "Current";
pub const TASK_FOCUS_COMPLETED: &str = "Completed";
pub const TASK_FOCUS_ALL: &str       = "All";

pub const TASK_NAME_EDIT_WIDGET: WidgetId = WidgetId::reserved(9247);

pub type BellBytes = &'static [u8; 5016];

#[cfg(target_os = "macos")]
pub static SOUND_TASK_FINISH: BellBytes = std::include_bytes!("../res/bell.ogg");
#[cfg(target_os = "linux")]
pub static SOUND_TASK_FINISH: BellBytes = std::include_bytes!("../res/bell.ogg");
#[cfg(target_os = "windows")]
pub const SOUND_TASK_FINISH: BellBytes = std::include_bytes!("..\\res\\bell.ogg");


pub fn get_work_interval(_uid: &String) -> chrono::Duration {
    chrono::Duration::minutes(50)
    // chrono::Duration::seconds(10)
}

pub fn get_rest_interval(_uid: &String) -> chrono::Duration {
    chrono::Duration::minutes(10)
    // chrono::Duration::seconds(10)
}

pub fn format_duration(dur: chrono::Duration) -> String {
    let mut empty = 0;
    let days = if dur.num_days() > 0 {
        format!("{}d", dur.num_days())
    } else {empty += 1; "".to_string()};

    let hours = if dur.num_hours() > 0 {
        format!(" {}h", dur.num_hours() % 24)
    } else {empty += 1;"".to_string()};

    let mins = if dur.num_minutes() > 0 {
        format!(" {}m", dur.num_minutes() % 60)
    } else {empty += 1;"".to_string()};

    let seconds = if dur.num_seconds() > 0 && dur.num_seconds() % 60 != 0 {
        format!(" {}s", dur.num_seconds() % 60)
    } else {empty += 1; "".to_string()};

    if empty == 4 {
        " 0s".to_string()
    } else {
        format!("{}{}{}{}", days, hours, mins, seconds)
    }
}


