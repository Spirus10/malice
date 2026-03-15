use chrono::{self, Datelike, Timelike};
use colored::*;

pub fn bad(msg: &str) {
    let current_date = chrono::offset::Local::now();
    let year = current_date.year();
    let month = current_date.month();
    let day = current_date.day();

    let hour = current_date.hour();
    let minute = current_date.minute();
    let second = current_date.second();

    println!(
        "{} {}, {} : {}",
        "[BAD]".red(),
        format!("{}/{}/{}", month, day, year).blue(),
        format!("{}:{}:{}", hour, minute, second).purple(),
        format!("{}", msg)
    );
}
