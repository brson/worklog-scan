use chrono::*;
use errors::*;
use std::mem;

use RawEntry;

type MinuteOfDay = u32;
type Minutes = u32;
type Action = String;
type Hours = f64;

pub fn do_time_report(entries: &[RawEntry], start: NaiveDate, end: NaiveDate) -> Result<()> {
    // Split entries by date, while recording dates of each subslice
    let mut dates = vec![String::new()];
    let mut entry_days: Vec<_> = entries.split(|e| {
        match *e {
            RawEntry::NewDay(ref s) => {
                dates.push(s.to_string());
                true
            }
            _ => false
        }
    }).collect();

    assert_eq!(dates.len(), entry_days.len());

    // The first entry is junk
    dates.remove(0);
    entry_days.remove(0);

    // Convert dates to NaiveDate
    let dates: Result<Vec<_>> = dates.iter().map(|s| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| Error::from(e.to_string()))
    }).collect();
    let dates = dates?;

    let dated_entries: Vec<(NaiveDate, &[RawEntry])> = dates.into_iter().zip(entry_days.into_iter()).collect();

    // Filter dates that are out of range
    let dated_entries: Vec<_> = dated_entries.into_iter().filter(|&(date, _)| {
        date >= start && date <= end
    }).collect();

    // Worklog goes from newest dates to oldest. For reporting that
    // needs to be reversed.
    let mut dated_entries = dated_entries;
    dated_entries.reverse();

    // Dates containing clocked-in actions
    let mut dated_timeslices: Vec<(NaiveDate, Minutes, Vec<Action>)> = vec![];

    let timestamp_to_minute_of_day = |h, m| (h as u32) * 60 + (m as u32);
    
    for (date, entries) in dated_entries.into_iter() {
        let mut clock_in: Option<MinuteOfDay> = None;
        let mut actions: Vec<String> = vec![];
        for (i, entry) in entries.iter().enumerate() {
            match *entry {
                RawEntry::ClockIn => {
                    if clock_in.is_some() {
                        bail!("clock-in without clock-out on {:?}", date);
                    }
                    match entries.get(i + 1) {
                        Some(&RawEntry::Time(h, m)) => {
                            clock_in = Some(timestamp_to_minute_of_day(h, m));
                        }
                        _ => bail!("clock-in not followed by timestamp on {:?}", date)
                    }
                }
                RawEntry::ClockOut => {
                    match clock_in {
                        Some(clock_in_) => {
                            match entries.get(i - 1) {
                                Some(&RawEntry::Time(h, m)) => {
                                    let clock_out = timestamp_to_minute_of_day(h, m);
                                    if clock_out <= clock_in_ {
                                        bail!("clock-out less than clock-in on {:?}", date);
                                    }

                                    // Push actions onto dated timeslices
                                    let minutes = clock_out - clock_in_;
                                    let new_actions = mem::replace(&mut actions, vec![]);
                                    dated_timeslices.push((date, minutes, new_actions));
                                    clock_in = None;
                                }
                                _ => bail!("clock-out not preceded by timestamp on {:?}", date)
                            }
                        }
                        _ => bail!("clock-out without clock-in on {:?}", date)
                    }
                }
                RawEntry::Action(ref s) => {
                    if clock_in.is_some() {
                        actions.push(s.to_string());
                    }
                }
                RawEntry::NewDay(..) => unreachable!(),
                RawEntry::Junk(..) |
                RawEntry::Time(..) |
                RawEntry::Prediction(..) => { }
            }
        }
    }

    // Round timeslices to half-hours

    let dated_timeslices: Vec<(NaiveDate, Hours, Vec<Action>)> =
        dated_timeslices.into_iter().map(|(date, minutes, actions)| {
            let minutes = minutes as f64;
            let hours = minutes / 60.0;
            // For Reddit at least I need reports accurate to the half-hour
            let halfhours = hours * 2.0;
            let rounded_halfhours = halfhours.round();
            let hours = rounded_halfhours / 2.0;
            (date, hours, actions)
        }).collect();

    print_report(start, end, &dated_timeslices)
}

fn print_report(start: NaiveDate, end: NaiveDate,
                data: &[(NaiveDate, Hours, Vec<Action>)]) -> Result<()> {

    let total_hours = data.iter().fold(0.0, |sum, &(_, hours, _)| sum + hours);

    println!("# Timesheet for Brian Anderson");
    println!();
    println!("name: Brian Anderson  ");
    println!("email: andersrb@gmail.com / v.brian.anderson@reddit.com  ");
    println!("manager: Chris Slowe <chris@reddit.com>");
    println!("reporting period: {} - {}  ", start, end);
    println!("total hours: {:.1}  ", total_hours);
    println!();
    println!("## Details");
    println!();
    println!("| Date | Hours | Detail |");
    println!("|:----:|:-----:|--------|");

    for &(date, hours, ref actions) in data {
        print!("| {} | {:2.1} | ", date, hours);
        let linebreak_actions = actions.join(" <br> ");
        println!("{} |", linebreak_actions);
    }

    println!();
    println!("## Methodology");
    println!();
    println!("{}", METHODOLOGY);
    println!("");
    println!("{}", STYLE);
    
    Ok(())
}

static METHODOLOGY: &str =
    "This report is automatically derived from my worklog, which \
    records every task I do, however minute. Each row in the report \
    represents a period during which I was 'clocked in', rounded to \
    the nearest half-hour (up or down).";

static STYLE: &str =
    "
<style>
* {
  font-family: sans-serif;
  line-height: 1.3em;
}

body {
  padding: 1em;
}

table {
  border-collapse: collapse;
}

th, td {
  border: 1px solid black;
  padding: 0.2em 1em 0.2em 1em;
  vertical-align: top;
}

a, a:visited {
  color: blue;
}
</style>";
