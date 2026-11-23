use windows::core::{w, PCWSTR};
use windows::Win32::{
    Globalization::{
        GetDateFormatEx, GetTimeFormatEx, DATE_LONGDATE, DATE_SHORTDATE, TIME_NOSECONDS,
    },
    System::SystemInformation::GetLocalTime,
};

pub struct TaskbarClock {
    pub time: String,
    pub date: String,
    pub short_date: String,
    pub month: String,
    pub first_weekday: i32,
    pub days: i32,
    pub today: i32,
}

pub fn taskbar_clock() -> TaskbarClock {
    let local = unsafe { GetLocalTime() };
    let mut text = [0u16; 128];
    let user_locale = PCWSTR::null();
    let count = unsafe {
        GetTimeFormatEx(
            user_locale,
            TIME_NOSECONDS,
            Some(&local),
            PCWSTR::null(),
            Some(&mut text),
        )
    };
    let time = formatted(&text, count);
    let count = unsafe {
        GetDateFormatEx(
            user_locale,
            DATE_SHORTDATE,
            Some(&local),
            PCWSTR::null(),
            Some(&mut text),
            PCWSTR::null(),
        )
    };
    let short_date = formatted(&text, count);
    let count = unsafe {
        GetDateFormatEx(
            user_locale,
            DATE_LONGDATE,
            Some(&local),
            PCWSTR::null(),
            Some(&mut text),
            PCWSTR::null(),
        )
    };
    let date = formatted(&text, count);
    let count = unsafe {
        GetDateFormatEx(
            user_locale,
            Default::default(),
            Some(&local),
            w!("MMMM yyyy"),
            Some(&mut text),
            PCWSTR::null(),
        )
    };
    let year = local.wYear;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match local.wMonth {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    let today = i32::from(local.wDay);
    TaskbarClock {
        time,
        short_date,
        date,
        month: formatted(&text, count),
        first_weekday: (i32::from(local.wDayOfWeek) - today + 1).rem_euclid(7),
        days,
        today,
    }
}

fn formatted(buffer: &[u16], count: i32) -> String {
    if count <= 1 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..(count - 1) as usize])
}
