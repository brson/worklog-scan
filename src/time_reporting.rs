// run with
//
//     cargo run -- ~/brson.github.com/worklog.md tr 2021-02-01 2021-02-28 200 "Common Orbit LLC" Nervos "Decrypted Sapiens" 1 2021-03-28 2021-04-15 > outputfile.md
//
// print ds-style output by setting OUTPUT=DS


use chrono::*;
use errors::*;
use std::mem;
use std::fmt::Display;
use lazy_static::lazy_static;
use regex::Regex;
use std::env;

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

pub fn do_time_report(entries: &[RawEntry], start: NaiveDate, end: NaiveDate, rate: f64,
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

    print_report(start, end, rate, &dated_timeslices, expenses, self_name, client, invoice_no, issue_date, due_date)
}

fn print_report(start: NaiveDate, end: NaiveDate, rate: f64,
                data: &[(NaiveDate, Hours, Vec<Action>)],
                expenses: Vec<Expense>,
                self_name: String,
                client: Option<String>,
                invoice_no: Option<u32>,
                issue_date: Option<NaiveDate>, due_date: Option<NaiveDate>) -> Result<()> {
    let style = env::var("OUTPUT").unwrap_or("normal".to_string());
    if style == "ds" {
        print_report_ds(start, end, rate, data, expenses, self_name, client, invoice_no, issue_date, due_date)
    } else {
        print_report_normal(start, end, rate, data, expenses, self_name, client, invoice_no, issue_date, due_date)
    }
}

fn print_report_normal(start: NaiveDate, end: NaiveDate, rate: f64,
                       data: &[(NaiveDate, Hours, Vec<Action>)],
                       expenses: Vec<Expense>,
                       self_name: String,
                       client: Option<String>,
                       invoice_no: Option<u32>,
                       issue_date: Option<NaiveDate>, due_date: Option<NaiveDate>) -> Result<()> {

    fn print_table_row_2(v1: impl Display, v2: impl Display) {
        println!("<tr><td>{}</td><td>{}</td></tr>", v1, v2);
    }

    let total_hours = data.iter().fold(0.0, |sum, &(_, hours, _)| sum + hours);
    let total_expenses = expenses.iter().fold(0.0, |total, expense| total + expense.cost);
    let amount_due = rate * total_hours + total_expenses;

    println!("<!doctype html>");
    println!("<meta charset='utf-8'>");
    println!("{}", STYLE);
    println!("");
    println!("<h1>Invoice from {}</h1>", self_name);
    println!();
    println!("<table>");
    print_table_row_2("name:", self_name);
    print_table_row_2("email:", "andersrb@gmail.com");
    if let Some(client) = client {
        print_table_row_2("client:", client);
    }
    if let Some(invoice_no) = invoice_no {
        print_table_row_2("invoice number:", invoice_no);
    }
    print_table_row_2("reporting period:", format!("{} - {}", start, end));
    if let Some(issue_date) = issue_date {
        print_table_row_2("issue date:", issue_date);
    }
    if let Some(due_date) = due_date {
        print_table_row_2("due date:", due_date);
    }
    print_table_row_2("total hours:", format!("{:.1}", total_hours));
    print_table_row_2("hourly rate:", format!("{:} USD", rate));
    if total_expenses > 0.0 {
        print_table_row_2("expenses:", format!("{:.2} USD", total_expenses));
    }
    print_table_row_2("amount due:", format!("{:.2} USD", amount_due));
    println!("</table>");
    println!();

    println!("<h2>Summary</h2>");
    println!();
    println!("<p>");
    println!("TODO fill-me-in");
    println!("</p>");
    println!();

    println!("<h2>Details</h2>");
    println!();
    println!("<table>");
    println!("<tr><th>Date</th><th>Hours</th><th>Detail</th></tr>");

    for &(date, hours, ref actions) in data {
        println!("<tr>");
        println!("<td>{}</td><td>{:2.1}</td>", date, hours);
        println!("<td>");
        for action in actions {
            let action = parse_md_link(action);
            println!("<p>{}</p>", action);
        }
        println!("</td>");
        println!("</tr>");
    }

    println!("</table>");
    println!();

    if total_expenses > 0.0 {
        println!("<h2>Expenses</h2>");
        println!();
        println!("<table>");
        println!("<tr><th>Date</th><th>Cost</th><th>Detail</th></tr>");

        for expense in expenses {
            println!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                     expense.date, expense.cost, expense.what);
        }

        println!("</table>");
        println!();
    }
    
    Ok(())
}

fn print_report_ds(start: NaiveDate, end: NaiveDate, rate: f64,
                   data: &[(NaiveDate, Hours, Vec<Action>)],
                   expenses: Vec<Expense>,
                   self_name: String,
                   client: Option<String>,
                   invoice_no: Option<u32>,
                   issue_date: Option<NaiveDate>, due_date: Option<NaiveDate>) -> Result<()> {

    fn print_table_row_2(v1: impl Display, v2: impl Display) {
        println!("<tr><td>{}</td><td>{}</td></tr>", v1, v2);
    }

    let total_hours = data.iter().fold(0.0, |sum, &(_, hours, _)| sum + hours);
    let total_expenses = expenses.iter().fold(0.0, |total, expense| total + expense.cost);
    let amount_due = rate * total_hours + total_expenses;

    println!("<!doctype html>");
    println!("<meta charset='utf-8'>");
    println!("{}", STYLE);
    println!("");
    println!("<h1>Invoice from {}</h1>", self_name);
    println!();
    println!("<table>");
    print_table_row_2("name:", self_name);
    print_table_row_2("email:", "andersrb@gmail.com");
    if let Some(client) = client {
        print_table_row_2("client:", client);
    }
    if let Some(invoice_no) = invoice_no {
        print_table_row_2("invoice number:", invoice_no);
    }
    print_table_row_2("reporting period:", format!("{} - {}", start, end));
    if let Some(issue_date) = issue_date {
        print_table_row_2("issue date:", issue_date);
    }
    if let Some(due_date) = due_date {
        print_table_row_2("due date:", due_date);
    }
    print_table_row_2("total hours:", format!("{:.1}", total_hours));
    print_table_row_2("hourly rate:", format!("{:} USD", rate));
    if total_expenses > 0.0 {
        print_table_row_2("expenses:", format!("{:.2} USD", total_expenses));
    }
    print_table_row_2("amount due:", format!("{:.2} USD", amount_due));
    println!("</table>");
    println!();

    println!("<h2>TL;DR</h2>");
    println!();
    println!("<p>");
    println!("TODO fill-me-in");
    println!("</p>");
    println!();

    println!("<h2>Details</h2>");
    println!();
    println!("<table>");
    println!("<tr><th>Description</th><th>Hours</th><th>Rate</th><th>Cost</th></tr>");

    for &(date, hours, ref actions) in data {
        println!("<tr>");
        println!("<td>");
        println!("<p>{}</p>", date);
        for action in actions {
            let action = parse_md_link(action);
            println!("<p>{}</p>", action);
        }
        println!("</td>");
        println!("<td>{:2.1}</td><td>{}</td><td>{}</td>",
                 hours, rate, hours * rate);
        println!("</tr>");
    }

    println!("</table>");
    println!();

    if total_expenses > 0.0 {
        println!("<h2>Expenses</h2>");
        println!();
        println!("<table>");
        println!("<tr><th>Date</th><th>Cost</th><th>Detail</th></tr>");

        for expense in expenses {
            println!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                     expense.date, expense.cost, expense.what);
        }

        println!("</table>");
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

td p {
  margin: 0;
}

a, a:visited {
  color: blue;
}
</style>";

fn parse_md_link(text: &str) -> String {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(.*)\[(.*)\]\((.*)\)(.*)$").unwrap();
    }

    let caps = REGEX.captures(text);
    if let Some(caps) = caps {
        let pre = &caps[1];
        let text = &caps[2];
        let link = &caps[3];
        let post = &caps[4];
        format!("{}<a href='{}'>{}</a>{}",
                pre, link, text, post)
    } else {
        text.to_string()
    }
}
