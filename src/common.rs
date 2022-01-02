use druid::{Color, FontDescriptor, FontFamily, FontWeight, WidgetId, Selector};
use core::time::Duration;

pub static TASK_COLOR_BG: Color                 = Color::rgb8(80, 73, 69);
pub static APP_BORDER: Color                    = Color::rgb8(60, 56, 54);
pub static TASK_ACTIVE_COLOR_BG: Color          = Color::rgb8(250, 189, 47);
pub static TASK_REST_COLOR_BG: Color            = Color::rgb8(131, 162, 152);
pub static TASK_PAUSE_COLOR_BG: Color           = Color::rgb8(211, 134, 155);
pub static TASK_FOCUS_BORDER: Color             = Color::rgb8(124, 111, 100);
pub static COLOR_ACTIVE: Color                  = Color::rgb8(255, 255, 255);
pub static DELETING_TASK_BORDER: Color          = Color::rgb8(204, 36, 29);
pub static RESTORED_TASK_BORDER: Color          = Color::rgb8(184, 187, 38);

pub static UI_TIMER_INTERVAL: Duration = Duration::from_secs(1);

pub static FONT_LOG_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::MONOSPACE).with_size(14.0);
pub static FONT_CAPTION_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::SYSTEM_UI)
    .with_weight(FontWeight::BOLD).with_size(18.0);

pub const COMMAND_TASK_NEW:    Selector            = Selector::new("tcmenu.task_new");
pub const COMMAND_TASK_START:  Selector<String>    = Selector::new("tcmenu.task_start");
pub const COMMAND_TASK_STOP:   Selector            = Selector::new("tcmenu.task_stop");
pub const COMMAND_TASK_PAUSE:   Selector           = Selector::new("tcmenu.task_pause");
pub const COMMAND_TASK_RESUME:   Selector<String>  = Selector::new("tcmenu.task_resume");
pub const COMMAND_TASK_ARCHIVE: Selector<String>   = Selector::new("tcmenu.task_archive");
pub const COMMAND_TASK_COMPLETED: Selector<String> = Selector::new("tcmenu.task_completed");

pub const COMMAND_TLIST_REQUEST_FOCUS: Selector    = Selector::new("tlist_request_focus");

pub const COMMAND_EDIT_REQUEST_FOCUS: Selector<WidgetId>  = Selector::new("tedit_request_focus");

pub const TASK_NAME_EDIT_WIDGET: WidgetId = WidgetId::reserved(1000);

pub type BellBytes = &'static [u8; 5016];

#[cfg(target_os = "macos")]
pub static SOUND_TASK_FINISH: BellBytes = std::include_bytes!("../res/bell.ogg");
#[cfg(target_os = "linux")]
pub static SOUND_TASK_FINISH: BellBytes = std::include_bytes!("../res/bell.ogg");
#[cfg(target_os = "windows")]
pub const SOUND_TASK_FINISH: BellBytes = std::include_bytes!("../res/bell.ogg");
