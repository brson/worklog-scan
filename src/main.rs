#[macro_use]
extern crate error_chain;
extern crate regex;
extern crate chrono;
#[macro_use]
extern crate lazy_static;

use std::env;
use std::fs::File;
use std::io::{BufReader, BufRead};
use errors::*;
use regex::Regex;
use chrono::*;

mod pleasure_and_pain;
use pleasure_and_pain as pp;

mod time_reporting;
use time_reporting as tr;

#[allow(deprecated)]
mod errors {
    error_chain! {
        foreign_links {
            Io(::std::io::Error);
        }
    }
}

fn main() {
    run().unwrap();
}

#[derive(Debug, Eq, PartialEq)]
enum Mode {
    PleasureAndPain,
    TimeReporting(NaiveDate, NaiveDate, String, Option<String>, Option<String>, Option<u32>, Option<NaiveDate>, Option<NaiveDate>),
}

fn run() -> Result<()> {
    let file = env::args().skip(1).next();

    if let Some(ref file) = file {
        let mode = env::args().skip(2).next();
        let mode = if let Some(ref mode) = mode {
            if mode == "pp" {
                Mode::PleasureAndPain
            } else if mode == "tr" {
                let start = env::args().skip(3).next();
                let end = env::args().skip(4).next();
                match (start, end) {
                    (Some(ref start), Some(ref end)) => {
                        let start: ParseResult<NaiveDate> = NaiveDate::parse_from_str(start, "%Y-%m-%d");
                        let end: ParseResult<NaiveDate> = NaiveDate::parse_from_str(end, "%Y-%m-%d");
                        let start = start.map_err(|e| e.to_string())?;
                        let end = end.map_err(|e| e.to_string())?;
                        let self_name = env::args().skip(5).next().expect("self-name");
                        let project = env::args().skip(6).next();
                        let client = env::args().skip(7).next();
                        let client = client.clone().or_else(|| project.clone());
                        let invoice_no = env::args().skip(8).next().map(|s| s.parse().expect("invoice-no"));
                        let issue_date = env::args().skip(9).next().map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").expect("issue-date"));
                        let due_date = env::args().skip(10).next().map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").expect("issue-date"));
                        Mode::TimeReporting(start, end, self_name, project, client, invoice_no, issue_date, due_date)
                    }
                    _ => {
                        bail!("no start or end for time report");
                    }
                }
            } else {
                bail!("unknown mode");
            }
        } else {
            bail!("no mode");
        };
        process_file(file, mode)    } else {
        bail!("no file");
    }
}

fn process_file(file: &str, mode: Mode) -> Result<()> {
    let lines = BufReader::new(File::open(file)?)
        .lines()
        .filter_map(|l| l.ok());
    let lines = lines.collect::<Vec<_>>();

    let raw_entries = lines.iter().map(|ref s| line_to_raw_entry(s));
    let raw_entries: Vec<_> = raw_entries.collect();

    match mode {
        Mode::PleasureAndPain => {
            let entries = pp::raw_to_entries(&raw_entries);
            pp::analyze_prediction(&entries)?;
        }
        Mode::TimeReporting(start, end, self_name, project, client, invoice_no, issue_date, due_date) => {
            tr::do_time_report(&raw_entries, start, end, self_name, project, client, invoice_no, issue_date, due_date)?;
        }
    }

    Ok(())
}

// Determine what each individual line represents
fn line_to_raw_entry(line: &str) -> RawEntry {
    let line_lcase = line.to_lowercase();

    if line_lcase.contains("clockin") {
        panic!("use 'clock in', not 'clockin'");
    }

    if line_lcase.contains("clockout") {
        panic!("use 'clock out', not 'clockout'");
    }

    if line_lcase.contains("clock in") {
        let company = parse_company(line);
        return RawEntry::ClockIn(company);
    }

    if line_lcase.contains("clock out") {
        let company = parse_company(line);
        return RawEntry::ClockOut(company);
    }

    if line.starts_with("# ") {
        let line = &line[2..];
        if let Some(date) = parse_date(line) {
            return RawEntry::NewDay(date);
        }
    }

    if !line.starts_with("- ") {
        return RawEntry::Junk(line.to_string());
    }

    let line = &line[2..];
    let line = line.trim();

    if let Some(time) = parse_time(line) {
        return RawEntry::Time(time.0, time.1);
    }

    if let Some(ppp) = parse_prediction(line) {
        return RawEntry::Prediction(ppp.0, ppp.1, ppp.2, ppp.3);
    }

    if let Some((cost, what)) = parse_expense(line) {
        return RawEntry::Expense(cost, what);
    }

    return RawEntry::Action(line.to_string());
}

fn parse_company(line: &str) -> Option<String> {
    let open_paren_idx = line.rfind("(");
    let close_paren_idx = line.rfind(")");
    match (open_paren_idx, close_paren_idx) {
        (Some(open_paren_idx), Some(close_paren_idx)) => {
            let region = &line[open_paren_idx + 1 .. close_paren_idx];
            let region = region.trim();
            Some(region.to_string())
        }
        _ => None,
    }
}

#[derive(Debug)]
pub enum RawEntry {
    Junk(String),
    NewDay(String),
    Action(String), // text, url
    Time(u8, u8), // hour (0-23), minute
    // predictud pleasure, pain, actual pleasure, pain
    Prediction(u8, u8, u8, u8),
    ClockIn(Option<String>),
    ClockOut(Option<String>),
    Expense(f64, String),
}

fn parse_date(s: &str) -> Option<String> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(\d{4})-(\d{2})-(\d{2})").expect("");
    }
    if REGEX.is_match(s) {
        Some(s.to_string())
    } else {
        None
    }
}

fn parse_time(s: &str) -> Option<(u8, u8)> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(\d{1,2}):(\d{2}) (AM|PM)").expect("");
    }
    if let Some(caps) = REGEX.captures(s) {
        let mut hour: u8 = str::parse(&caps[1]).expect("");
        let minute: u8 = str::parse(&caps[2]).expect("");
        let am_pm = &caps[3];

        if hour == 12 {
            hour = 0;
        }

        if am_pm == "PM" {
            hour += 12;
        }

        Some((hour, minute))
    } else {
        None
    }
}

fn parse_prediction(s: &str) -> Option<(u8, u8, u8, u8)> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(\d+)/(\d+):(\d+)/(\d+)").expect("");
    }
    if let Some(caps) = REGEX.captures(s) {
        let pr_pl: u8 = str::parse(&caps[1]).expect("");
        let pr_pn: u8 = str::parse(&caps[2]).expect("");
        let ac_pl: u8 = str::parse(&caps[3]).expect("");
        let ac_pn: u8 = str::parse(&caps[4]).expect("");
        Some((pr_pl, pr_pn, ac_pl, ac_pn))
    } else {
        None
    }
}

fn parse_expense(s: &str) -> Option<(f64, String)> {
    if !s.to_ascii_lowercase().starts_with("expense:") {
        return None;
    }

    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^Expense: *\$((?:\d|\.)*),(.*)").expect("");
    }
    if let Some(caps) = REGEX.captures(s) {
        let cost: f64 = str::parse(&caps[1]).expect("float expense cost");
        let what: String = caps[2].to_string();
        return Some((cost, what));
    } else {
        panic!("malformed expense: {}", s);
    }
}

