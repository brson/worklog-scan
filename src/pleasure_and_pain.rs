#![allow(unused)]

use std::mem;
use std::cell::RefCell;

use errors::*;
use RawEntry;

pub fn analyze_prediction(entries: &[Entry]) -> Result<()> {

    let stats = basic_stats(entries);
    let weekly = weekly_stats(entries);

    println!("Pleasure predicting");
    println!("===================");

    println!();
    println!("Weekly");
    println!("------");
    println!();

    for week_stats in weekly {
        let ref stats = week_stats.stats;
        println!("{}", week_stats.week);
        println!("predictions: {}", stats.predictions);
        println!("median prediction: {} pr_pl / {} pr_pn : {} ac_pl / {} ac_pn",
                 stats.med_pr_pl, stats.med_pr_pn, stats.med_ac_pl, stats.med_ac_pn);
        println!("mean prediction: {} pr_pl / {} pr_pn : {} ac_pl / {} ac_pn",
                 stats.mean_pr_pl, stats.mean_pr_pn, stats.mean_ac_pl, stats.mean_ac_pn);
        println!();
    }
    
    println!();
    println!("Totals");
    println!("------");
    println!();
    println!("predictions: {}", stats.predictions);
    println!("median prediction: {} pr_pl / {} pr_pn : {} ac_pl / {} ac_pn",
             stats.med_pr_pl, stats.med_pr_pn, stats.med_ac_pl, stats.med_ac_pn);
    println!("mean prediction: {} pr_pl / {} pr_pn : {} ac_pl / {} ac_pn",
             stats.mean_pr_pl, stats.mean_pr_pn, stats.mean_ac_pl, stats.mean_ac_pn);

    Ok(())
}

struct BasicStats {
    predictions: usize,
    med_pr_pl: u8,
    med_pr_pn: u8,
    med_ac_pl: u8,
    med_ac_pn: u8,
    mean_pr_pl: f32,
    mean_pr_pn: f32,
    mean_ac_pl: f32,
    mean_ac_pn: f32,
}

fn basic_stats(entries: &[Entry]) -> BasicStats {
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

    if predictions == 0 {
        return BasicStats {
            predictions,
            med_pr_pl: 0,
            med_pr_pn: 0,
            med_ac_pl: 0,
            med_ac_pn: 0,
            mean_pr_pl: 0.0,
            mean_pr_pn: 0.0,
            mean_ac_pl: 0.0,
            mean_ac_pn: 0.0,
        };
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

    BasicStats {
        predictions,
        med_pr_pl,
        med_pr_pn,
        med_ac_pl,
        med_ac_pn,
        mean_pr_pl,
        mean_pr_pn,
        mean_ac_pl,
        mean_ac_pn,
    }
}

struct WeeklyStats {
    week: String,
    stats: BasicStats,
}

fn weekly_stats(entries: &[Entry]) -> Vec<WeeklyStats> {
    let mut ranges = vec![];

    let mut cur_week = String::new();

    let mut idx1 = 0;
    let mut idx2 = 0;

    // Find the ranges of entries that represent each week
    for (idx, entry) in entries.iter().enumerate() {
        let this_week = week_of(&entry.date);
        if this_week != cur_week {
            if idx1 != 0 && idx2 != 0 {
                ranges.push(idx1 .. idx2);
            }

            cur_week = this_week;
            idx1 = idx;
            idx2 = idx;
        } else {
            idx2 += 1;
        }
    }

    if idx1 != idx2 {
        ranges.push(idx1 .. idx2);
    }

    ranges.into_iter().rev().map(|range| {
        let entries = &entries[range];
        let week = week_of(&entries[0].date);
        let stats = basic_stats(entries);
        WeeklyStats { week, stats }
    }).collect()
}

fn week_of(date: &str) -> String {
    use chrono::*;
    let default = NaiveDate::from_ymd(2017, 1, 1);
    let dt: ParseResult<NaiveDate> = NaiveDate::parse_from_str(date, "%Y-%m-%d");
    let dt = dt.unwrap_or(default);
    let dt = Local.from_local_date(&dt).unwrap();
    let (_, week, _) = dt.isoweekdate();
    let year = dt.year();
    format!("{}, wk {}", year, week)
}

#[derive(Debug)]
struct Prediction(u8, u8, u8, u8); // pr-pl / pr-pn : ac-pl / ac-pn
#[derive(Debug)]
struct Time(u8, u8); // h, m

#[derive(Debug)]
pub struct Entry {
    date: String,
    desc: String,
    url: Option<String>,
    pred: Option<Prediction>,
    time: Option<Time>,
}

pub fn raw_to_entries(raws: &[RawEntry]) -> Vec<Entry> {
    let date = RefCell::new("2099-01-01".to_string());

    let new_entry = || Entry {
        date: date.borrow().clone(),
        desc: String::new(),
        url: None,
        pred: None,
        time: None,
    };

    let mut next_entry = new_entry();

    let mut entries: Vec<_> = raws.iter().filter_map(|raw| {
        match *raw {
            RawEntry::Junk(_) => None,
            RawEntry::NewDay(ref d) => {
                *date.borrow_mut() = d.clone();
                next_entry = new_entry();
                next_entry.desc = "New day".to_string();
                None
            }
            RawEntry::Action(ref s) => {
                let entry = mem::replace(&mut next_entry, new_entry());
                next_entry.desc = s.clone();
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
            RawEntry::ClockIn(..) | RawEntry::ClockOut(..) => None,
            RawEntry::Expense(..) => None,
        }
    }).collect();

    entries.push(next_entry);

    entries
}
