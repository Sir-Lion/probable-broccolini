use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Deserialize, Clone)]
struct Item {
    label: String,
    first: String,
    #[serde(default)]
    every: i64, // days between repeats, 0 = one-off
    amounts: Vec<f64>, // + money in, - money out; cycles through the list
    until: Option<String>,
    count: Option<i64>,
}

#[derive(Deserialize)]
struct Purchase {
    label: String,
    total: f64,
    count: i64,
    first: String,
    rest: String,
    every: i64,
}

#[derive(Deserialize)]
struct Input {
    start_balance: f64,
    start: String,
    horizon: i64,
    items: Vec<Item>,
    purchase: Option<Purchase>,
}

#[derive(Serialize, Clone)]
struct Entry {
    label: String,
    amount: f64,
}

#[derive(Serialize)]
struct Day {
    date: String,
    net: f64,
    balance: f64,
    items: Vec<Entry>,
}

#[derive(Serialize)]
struct Summary {
    days: Vec<Day>,
    min: f64,
    min_date: String,
    end: f64,
}

#[derive(Serialize)]
struct Output {
    with: Summary,
    without: Summary,
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn parse(s: &str) -> Option<i64> {
    let mut p = s.split('-');
    let y = p.next()?.parse().ok()?;
    let m = p.next()?.parse().ok()?;
    let d = p.next()?.parse().ok()?;
    Some(days_from_civil(y, m, d))
}

fn fmt(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn expand(it: &Item, start: i64, end: i64) -> Vec<(i64, f64)> {
    let mut out = Vec::new();
    let first = match parse(&it.first) {
        Some(d) => d,
        None => return out,
    };
    if it.amounts.is_empty() {
        return out;
    }
    let until = it.until.as_deref().and_then(parse).unwrap_or(i64::MAX);
    let limit = it.count.unwrap_or(i64::MAX);
    let mut k: i64 = 0;
    loop {
        let d = first + k * it.every.max(0);
        if d > end || d > until || k >= limit {
            break;
        }
        if d >= start {
            out.push((d, it.amounts[(k as usize) % it.amounts.len()]));
        }
        if it.every <= 0 {
            break;
        }
        k += 1;
    }
    out
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn run(inp: &Input, with_purchase: bool) -> Summary {
    let start = parse(&inp.start).unwrap_or(0);
    let horizon = inp.horizon.clamp(1, 400);
    let end = start + horizon;
    let mut items = inp.items.clone();

    if with_purchase {
        if let Some(p) = &inp.purchase {
            let n = p.count.max(1);
            let a = -(p.total / n as f64 * 100.0).round() / 100.0;
            items.push(Item {
                label: p.label.clone(),
                first: p.first.clone(),
                every: 0,
                amounts: vec![a],
                until: None,
                count: None,
            });
            if n > 1 {
                items.push(Item {
                    label: p.label.clone(),
                    first: p.rest.clone(),
                    every: p.every,
                    amounts: vec![a],
                    until: None,
                    count: Some(n - 1),
                });
            }
        }
    }

    let mut ev: Vec<Vec<Entry>> = (0..=horizon).map(|_| Vec::new()).collect();
    for it in &items {
        for (d, a) in expand(it, start, end) {
            ev[(d - start) as usize].push(Entry {
                label: it.label.clone(),
                amount: a,
            });
        }
    }

    let mut bal = inp.start_balance;
    let mut min = f64::MAX;
    let mut min_date = String::new();
    let mut days = Vec::new();
    for (i, list) in ev.iter().enumerate() {
        let net: f64 = list.iter().map(|e| e.amount).sum();
        bal += net;
        let date = fmt(start + i as i64);
        if bal < min {
            min = bal;
            min_date = date.clone();
        }
        days.push(Day {
            date,
            net: round2(net),
            balance: round2(bal),
            items: list.clone(),
        });
    }
    Summary {
        days,
        min: round2(min),
        min_date,
        end: round2(bal),
    }
}

#[wasm_bindgen]
pub fn simulate(input: &str) -> String {
    match serde_json::from_str::<Input>(input) {
        Ok(inp) => {
            let out = Output {
                with: run(&inp, true),
                without: run(&inp, false),
            };
            serde_json::to_string(&out).unwrap_or_default()
        }
        Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
    }
}
