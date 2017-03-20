#[macro_use]
extern crate error_chain;
extern crate regex;

use std::cell::RefCell;
use std::mem;
use std::env;
use std::fs::File;
use std::io::{BufReader, BufRead};
use errors::*;
use regex::Regex;

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

fn run() -> Result<()> {
    let file = env::args().skip(1).next();

    if let Some(ref file) = file {
        process_file(file)
    } else {
        bail!("no error");
    }
}

fn process_file(file: &str) -> Result<()> {
    let lines = BufReader::new(File::open(file)?)
        .lines()
        .filter_map(|l| l.ok());

    let raw_entries = lines.map(|ref s| line_to_raw_entry(s));
    let raw_entries: Vec<_> = raw_entries.collect();

    let entries = raw_to_entries(raw_entries);

    analyze_prediction(&entries)?;

    Ok(())
}

fn analyze_prediction(entries: &[Entry]) -> Result<()> {
    let mut predictions = 0;
    let mut total_pr_pl = 0;
    let mut total_pr_pn = 0;
    let mut total_ac_pl = 0;
    let mut total_ac_pn = 0;

    let mut pr_pls = vec![];
    let mut pr_pns = vec![];
    let mut ac_pls = vec![];
    let mut ac_pns = vec![];

    for entry in entries {
        if let Some(ref pr) = entry.pred {
            let Prediction(pr_pl, pr_pn, ac_pl, ac_pn) = *pr;
            predictions += 1;
            total_pr_pl += pr_pl as u64;
            total_pr_pn += pr_pn as u64;
            total_ac_pl += ac_pl as u64;
            total_ac_pn += ac_pn as u64;
            pr_pls.push(pr_pl);
            pr_pns.push(pr_pn);
            ac_pls.push(ac_pl);
            ac_pns.push(ac_pn);
        }
    }

    pr_pls.sort();
    pr_pns.sort();
    ac_pls.sort();
    ac_pns.sort();

    let med = pr_pls.len() / 2;
    let med_pr_pl = pr_pls[med];
    let med_pr_pn = pr_pns[med];
    let med_ac_pl = ac_pls[med];
    let med_ac_pn = ac_pns[med];

    let mean_pr_pl = total_pr_pl as f32 / predictions as f32;
    let mean_pr_pn = total_pr_pn as f32 / predictions as f32;
    let mean_ac_pl = total_ac_pl as f32 / predictions as f32;
    let mean_ac_pn = total_ac_pn as f32 / predictions as f32;

    println!("Pleasure predicting");
    println!("===================");
    println!("");
    println!("predictions: {}", predictions);
    println!("median prediction: {} pr_pl / {} pr_pn : {} ac_pl / {} ac_pn",
             med_pr_pl, med_pr_pn, med_ac_pl, med_ac_pn);
    println!("mean prediction: {} pr_pl / {} pr_pn : {} ac_pl / {} ac_pn",
             mean_pr_pl, mean_pr_pn, mean_ac_pl, mean_ac_pn);

    Ok(())
}

// Determine what each individual line represents
fn line_to_raw_entry(line: &str) -> RawEntry {
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

    if let Some(time) = parse_time(line) {
        return RawEntry::Time(time.0, time.1);
    }

    if let Some(ppp) = parse_prediction(line) {
        return RawEntry::Prediction(ppp.0, ppp.1, ppp.2, ppp.3);
    }

    return RawEntry::Action(line.to_string());
}

#[derive(Debug)]
enum RawEntry {
    Junk(String),
    NewDay(String),
    Action(String), // text, url
    Time(u8, u8), // hour (0-23), minute
    // predictud pleasure, pain, actual pleasure, pain
    Prediction(u8, u8, u8, u8),
}

fn parse_date(s: &str) -> Option<String> {
    let regex = Regex::new(r"^(\d{4})-(\d{2})-(\d{2})").expect("");
    if regex.is_match(s) {
        Some(s.to_string())
    } else {
        None
    }
}

fn parse_time(s: &str) -> Option<(u8, u8)> {
    let regex = Regex::new(r"^(\d{1,2}):(\d{2}) (AM|PM)").expect("");
    if let Some(caps) = regex.captures(s) {
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
    let regex = Regex::new(r"^(\d+)/(\d+):(\d+)/(\d+)").expect("");
    if let Some(caps) = regex.captures(s) {
        let pr_pl: u8 = str::parse(&caps[1]).expect("");
        let pr_pn: u8 = str::parse(&caps[2]).expect("");
        let ac_pl: u8 = str::parse(&caps[3]).expect("");
        let ac_pn: u8 = str::parse(&caps[4]).expect("");
        Some((pr_pl, pr_pn, ac_pl, ac_pn))
    } else {
        None
    }
}

#[derive(Debug)]
struct Prediction(u8, u8, u8, u8); // pr-pl / pr-pn : ac-pl / ac-pn
#[derive(Debug)]
struct Time(u8, u8); // h, m

#[derive(Debug)]
struct Entry {
    date: String,
    desc: String,
    url: Option<String>,
    pred: Option<Prediction>,
    time: Option<Time>,
}

fn raw_to_entries(raws: Vec<RawEntry>) -> Vec<Entry> {
    let date = RefCell::new("2099-01-01".to_string());

    let new_entry = || Entry {
        date: date.borrow().clone(),
        desc: String::new(),
        url: None,
        pred: None,
        time: None,
    };

    let mut next_entry = new_entry();

    let mut entries: Vec<_> = raws.into_iter().filter_map(|raw| {
        match raw {
            RawEntry::Junk(_) => None,
            RawEntry::NewDay(d) => {
                *date.borrow_mut() = d;
                next_entry = new_entry();
                next_entry.desc = "New day".to_string();
                None
            }
            RawEntry::Action(s) => {
                let entry = mem::replace(&mut next_entry, new_entry());
                next_entry.desc = s;
                Some(entry)
            }
            RawEntry::Time(h, m) => {
                next_entry.time = Some(Time(h, m));
                None
            }
            RawEntry::Prediction(pl0, pn0, pl1, pn1) => {
                next_entry.pred = Some(Prediction(pl0, pn0, pl1, pn1));
                None
            }
        }
    }).collect();

    entries.push(next_entry);

    entries
}
