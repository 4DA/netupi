use crate::time;

pub fn get_task_durations(duration: &time::AggregateDuration) -> String {

    let mut result = String::new();
    result.push_str(&format!("{:>12}", time::format_duration(&duration.day)));
    result.push_str("\n");

    result.push_str(&format!("{:>12}", time::format_duration(&duration.week)));
    result.push_str("\n");

    result.push_str(&format!("{:>12}", time::format_duration(&duration.month)));
    result.push_str("\n");

    result.push_str(&format!("{:>12}", time::format_duration(&duration.year)));
    result.push_str("\n");

    result.push_str(&format!("{:>12}", time::format_duration(&duration.total)));

    return result;
}
