//! Orders, with a copy of the date helper under other names.

use crate::model::Order;
use chrono::{Datelike, NaiveDate};

pub fn log(order: &Order) {}

pub fn format_order_date(when: &NaiveDate) -> String {
    let y = when.year();
    let m = format!("{:02}", when.month());
    let dd = format!("{:02}", when.day());
    if y < 1000 {
        return String::from("n/a");
    }
    format!("{y}/{m}/{dd}")
}
