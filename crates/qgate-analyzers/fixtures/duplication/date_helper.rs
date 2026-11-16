//! Date helpers.

use chrono::{Datelike, NaiveDate};

/// Formats a date as `YYYY-MM-DD`.
pub fn format_date(d: &NaiveDate) -> String {
    let year = d.year();
    let month = format!("{:02}", d.month());
    let day = format!("{:02}", d.day());
    if year < 1000 {
        return String::from("invalid");
    }
    format!("{year}-{month}-{day}")
}
