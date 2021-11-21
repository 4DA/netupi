# netupi
minimalistic time tracker with simple pomodoro timer

![screenshot](screenshot.png)

# Usage
- up/down arrow : select task
- spacebar : start tracking, pause, switch to other task
- escae : stop tracking
- right mouse button : task context menu

# Program data
Program settings and tasks are stored user's config directory:
- Linux : `$XDG_CONFIG_HOME or $HOME/.config` (`/home/alice/.config`)
- macOS	`$HOME/Library/Application Support`	(`/Users/Alice/Library/Application Support`)
- Windows	`{FOLDERID_RoamingAppData}` (`C:\Users\Alice\AppData\Roaming`)

# Configuration
No configuration for the moment. Work/rest duration are currently
fixed to 50m/10m, because it is what works for me.

# Building
1. install [rust](https://www.rust-lang.org/tools/install)
2. make sure you have sqlite3
3. cargo build --release

# Installing
- TBD
