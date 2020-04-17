use chrono::{DateTime, Utc};
use colored::*;
use failure::{bail, Error};
use log::{debug, info, warn};
use std::convert::TryFrom;
use std::path::Path;
use std::time::Duration;

use crate::month::Month;
use crate::storage::*;

pub fn start<P: AsRef<Path>>(storage: P) -> Result<(), Error> {
    let mut store = WorkStorage::from_file(&storage)?;
    if let Ok(s) = store.try_start() {
        bail!("You already started on {} at {}h", s.start.date().format("%d/%m/%Y"), s.start.time().format("%H:%M"));
    } else {
        let now = Duration::new(0, 0);
        let date: DateTime<Utc> = Utc::now();
        store.add_set(WorkSet::new(WorkType::Start, now, date));
    }

    debug!("store: {:?}", store);
    store.write(&storage)?;
    Ok(())
}

pub fn stop<P: AsRef<Path>>(storage: P) -> Result<(), Error> {
    if !storage.as_ref().exists() {
        bail!(
            "There is no time storage {:?}, start working first. It creates the file if necessary",
            storage.as_ref()
        );
    }
    let mut store = WorkStorage::from_file(&storage)?;
    let s = store.try_start()?;
    let now: DateTime<Utc> = Utc::now();
    let duration: Duration = now.signed_duration_since(s.start).to_std()?;
    if duration > Duration::new(24 * 60 * 60, 0) {
        warn!(
            "{}, you worked more than a day? It's been {}:{}h",
            store.name(),
            duration.as_secs() / 3600,
            duration.as_secs() / 60 - duration.as_secs() / 3600
        );
    }

    store.del_start();
    store.add_set(WorkSet::new(WorkType::Work, duration, s.start));
    store.write(&storage)?;
    info!(
        "You worked {}:{}h today. Enjoy your evening \u{1F389}",
        duration.as_secs() / 3600,
        duration.as_secs() / 60 - duration.as_secs() / 3600
    );
    Ok(())
}

pub fn stats<P: AsRef<Path>>(storage: P, month: Option<Month>) -> Result<(), Error> {
    if !storage.as_ref().exists() {
        bail!(
            "There is no time storage {:?}, start working first. It creates the file if necessary",
            storage.as_ref()
        );
    }
    let store = WorkStorage::from_file(&storage)?;
    match month {
        Some(m) => monthly_stats(&storage, m),
        None => {
            if store.work_sets.len() < 6 {
                info!("{}", store.stats());
                Ok(())
            } else {
                all_monthly_stats(&storage)
            }
        }
    }
}

fn all_monthly_stats<P: AsRef<Path>>(storage: P) -> Result<(), Error> {
    let store = WorkStorage::from_file(storage)?;
    let months = store.months();
    let weeks = store.weeks();
    info!("Here are your stats, {}:", store.name());
    for m in months {
        let work_per_m =
            store.filter(|w| Month::from(w.start.date().format("%B").to_string()) == m);
        println!(
            "{} {: >2}{}",
            "Month".green(),
            Month::try_from(m)?.to_string().green(),
            ":".green()
        );
        for w in &weeks {
            let work_per_w: Duration = work_per_m
                .filter(|s| &s.start.date().format("%W").to_string() == w)
                .work_sets
                .iter()
                .fold(Duration::new(0, 0), |acc, d| acc + d.duration);
            if work_per_w.as_nanos() > 0 {
                let h = work_per_w.as_secs() / 3600;
                let min = work_per_w.as_secs() / 60 - h * 60;
                println!(" Week {}: {: >4}:{:02}h", w, h, min);
            }
        }
    }
    if let Ok(s) = store.try_start() {
        println!(" {}", s);
    }
    if let Ok(b) = store.try_break() {
        println!(" {}", b);
    }
    Ok(())
}

fn monthly_stats<P: AsRef<Path>>(storage: P, month: Month) -> Result<(), Error> {
    let store = WorkStorage::from_file(storage)?;
    let weeks = store.weeks();
    let work_per_m =
        store.filter(|w| Month::from(w.start.date().format("%B").to_string()) == month);
    if work_per_m.work_sets.is_empty() {
        warn!("{}, you did not work in {}!", store.name(), month);
        return Ok(());
    }

    info!("Here are your stats for {}, {}:", month, store.name());
    for w in &weeks {
        let work_per_w: Duration = work_per_m
            .filter(|s| &s.start.date().format("%W").to_string() == w)
            .work_sets
            .iter()
            .fold(Duration::new(0, 0), |acc, d| acc + d.duration);
        if work_per_w.as_nanos() > 0 {
            let h = work_per_w.as_secs() / 3600;
            let min = work_per_w.as_secs() / 60 - h * 60;
            println!(" Week {}: {: >4}:{:02}h", w, h, min);
        }
    }
    if let Ok(s) = store.try_start() {
        println!(" {}", s);
    }
    if let Ok(b) = store.try_break() {
        println!(" {}", b);
    }
    Ok(())
}

pub fn take_break<P: AsRef<Path>>(storage: P) -> Result<(), Error> {
    let mut store = WorkStorage::from_file(&storage)?;
    if store.try_start().is_err() {
        bail!("You want to take a break, but you didn't start yet");
    }
    match store.try_break() {
        Ok(b) => {
            let now: DateTime<Utc> = Utc::now();
            let duration: Duration = now.signed_duration_since(b.start).to_std()?;
            if b.duration != Duration::new(0, 0) {
                debug!("There is already a break, starting another one.");
                let dur = Duration::new(0, 0);
                store.add_set(WorkSet::new(WorkType::Break, dur, now));

                debug!("store: {:?}", store);
                store.write(&storage)?;
                return Ok(());
            }

            if duration > Duration::new(24 * 60 * 60, 0) {
                warn!(
                    "{}, your break of {}:{}h is quite long. Did you fall asleep?",
                    store.name(),
                    duration.as_secs() / 3600,
                    duration.as_secs() / 60 - duration.as_secs() / 3600
                );
            }
            store.del_break();
            store.add_set(WorkSet::new(WorkType::Break, duration, now));
            store.write(&storage)?;
            Ok(())
        }
        Err(_) => {
            let dur = Duration::new(0, 0);
            let date: DateTime<Utc> = Utc::now();
            store.add_set(WorkSet::new(WorkType::Break, dur, date));

            debug!("store: {:?}", store);
            store.write(&storage)?;
            Ok(())
        }
    }
}