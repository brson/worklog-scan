// run with
//
//     cargo run -- ~/brson.github.com/worklog.md tr 2021-02-01 2021-02-28 "Common Orbit LLC" Nervos "Decrypted Sapiens" 1 2021-03-28 2021-04-15 > outputfile.md
//
// convert md output to html with
//
//     comrak -e table -e autolink --unsafe

use chrono::*;
use errors::*;
use std::mem;

use RawEntry;

type MinuteOfDay = u32;
type Minutes = u32;
type Action = String;
type Hours = f64;

struct Expense {
    date: NaiveDate,
    cost: f64,
    what: String,
}

pub fn do_time_report(entries: &[RawEntry], start: NaiveDate, end: NaiveDate,
                      self_name: String, project: Option<String>, client: Option<String>,
                      invoice_no: Option<u32>, issue_date: Option<NaiveDate>, due_date: Option<NaiveDate>) -> Result<()> {
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

    let mut expenses = vec![];
    
    for (date, entries) in dated_entries.into_iter() {
        let mut clock_in: Option<MinuteOfDay> = None;
        let mut actions: Vec<String> = vec![];
        for (i, entry) in entries.iter().enumerate() {
            match *entry {
                RawEntry::ClockIn(ref c) if *c == project => {
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
                RawEntry::ClockOut(ref c) if *c == project => {
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
                RawEntry::ClockIn(..) | RawEntry::ClockOut(..) => { }
                RawEntry::Action(ref s) => {
                    if clock_in.is_some() {
                        actions.push(s.to_string());
                    }
                }
                RawEntry::Expense(cost, ref what) => {
                    expenses.push(Expense { date, cost, what: what.to_string() });
                }
                RawEntry::NewDay(..) => unreachable!(),
                RawEntry::Junk(..) |
                RawEntry::Time(..) |
                RawEntry::Prediction(..) => { }
            }
        }

        if clock_in.is_some() {
            bail!("clock-in without clock-out on {:?}", date);
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

    print_report(start, end, &dated_timeslices, expenses, self_name, client, invoice_no, issue_date, due_date)
}

fn print_report(start: NaiveDate, end: NaiveDate,
                data: &[(NaiveDate, Hours, Vec<Action>)],
                expenses: Vec<Expense>,
                self_name: String,
                client: Option<String>,
                invoice_no: Option<u32>,
                issue_date: Option<NaiveDate>, due_date: Option<NaiveDate>) -> Result<()> {

    let total_hours = data.iter().fold(0.0, |sum, &(_, hours, _)| sum + hours);
    let hourly_rate = 200.0;
    let total_expenses = expenses.iter().fold(0.0, |total, expense| total + expense.cost);
    let amount_due = hourly_rate * total_hours + total_expenses;

    println!("<meta charset='utf-8'>");
    println!("{}", STYLE);
    println!("");
    println!("# Invoice for {}", self_name);
    println!();
    println!("name: {}  ", self_name);
    println!("email: andersrb@gmail.com  ");
    if let Some(client) = client {
        println!("client: {}  ", client);
    }
    if let Some(invoice_no) = invoice_no {
        println!("invoice number: {}  ", invoice_no);
    }
    println!("reporting period: {} - {}  ", start, end);
    if let Some(issue_date) = issue_date {
        println!("issue date: {}  ", issue_date);
    }
    if let Some(due_date) = due_date {
        println!("due date: {}  ", due_date);
    }
    println!("total hours: {:.1}  ", total_hours);
    println!("hourly rate: ${:}  ", hourly_rate);
    if total_expenses > 0.0 {
        println!("expenses: ${:.2}  ", total_expenses);
    }
    println!("amount due: ${:.2}  ", amount_due);
    println!();
    println!("## TL;DR");
    println!();
    println!("TODO fill-me-in");
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

    if total_expenses > 0.0 {
        println!("## Expenses");
        println!();
        println!("| Date | Cost | Detail |");
        println!("|:----:|:----:|--------|");

        for expense in expenses {
            println!("| {} | {} | {} ",
                     expense.date, expense.cost, expense.what
            );
        }

        println!();
    }
    
    Ok(())
}

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
