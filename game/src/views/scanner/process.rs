use std::collections::BTreeMap;

use enum_map::EnumMap;
use hes_engine::{Id, Output, Process, State};

use crate::{
    consts,
    state::{GameState, Tutorial},
    views::scanner::ScanResult,
};

use super::Scannable;

fn is_subtractable(process: &Process, mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>) -> bool {
    let change = mix_changes[process.output].get(&process.id).unwrap_or(&0);
    process.mix_share as isize + *change != 0
}

/// Planned shares of the wind and solar processes other than this one.
pub fn planned_intermittent_others(
    process: &Process,
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> usize {
    state
        .world
        .processes
        .iter()
        .filter(|p| p.id != process.id && state.is_intermittent_electricity(&p.id))
        .map(|p| (p.mix_share as isize + process_mix_changes[p.output].get(&p.id).unwrap_or(&0)).max(0))
        .sum::<isize>() as usize
}

/// Planned wind and solar share of electricity, counting planned changes.
pub fn planned_intermittent_share(
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> f32 {
    state
        .world
        .processes
        .iter()
        .filter(|p| state.is_intermittent_electricity(&p.id))
        .map(|p| (p.mix_share as isize + process_mix_changes[p.output].get(&p.id).unwrap_or(&0)).max(0))
        .sum::<isize>() as f32
        / 20.
}

/// Electricity demand after planned production changes: e.g. shifting
/// fuel to green hydrogen adds the electricity hydrogen takes to make.
pub fn planned_electricity(
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> f32 {
    let demand = state.output_demand.total();
    let change: f32 = state
        .world
        .processes
        .iter()
        .filter_map(|p| {
            let change = *process_mix_changes[p.output].get(&p.id)?;
            let output = change as f32 / 20. * demand[p.output];
            Some(output * p.adj_resources().electricity)
        })
        .sum();
    (demand[Output::Electricity] + change).max(0.)
}

/// Materials a year for building the capacity that planned changes add,
/// while it's being built.
pub fn planned_construction_per_year(
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> hes_engine::materials::BuildOut {
    let mut build = hes_engine::materials::BuildOut::default();
    for p in state.world.processes.iter() {
        let change = *process_mix_changes[p.output].get(&p.id).unwrap_or(&0);
        if change <= 0 {
            continue;
        }
        let output = change as f32 / 20. * state.output_demand.of(p.output);
        if let Some((b, _)) = hes_engine::materials::BuildOut::for_output(p, output) {
            build.combine_with(&b);
        }
    }
    let mut per_year = hes_engine::materials::BuildOut::default();
    let years = hes_engine::materials::CONSTRUCTION_YEARS as f32;
    per_year.steel = build.steel / years;
    per_year.cement = build.cement / years;
    per_year.other_energy = build.other_energy / years;
    per_year
}

/// Fuels (uranium, oil, gas, coal, lithium...) that would run out within
/// three planning cycles at the planned mix: each with the years left and
/// the planned processes that need it.
pub fn planned_feedstock_warnings(
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> Vec<(hes_engine::Feedstock, f32, Vec<String>)> {
    use hes_engine::Feedstock;
    const WARN_YEARS: f32 = 15.;
    let demand = state.output_demand.total();
    let mut rates: BTreeMap<String, (Feedstock, f32, Vec<String>)> = BTreeMap::new();
    for p in state.world.processes.iter() {
        let feedstock = p.feedstock.0;
        if matches!(feedstock, Feedstock::Soil | Feedstock::Other) {
            continue;
        }
        let share = p.mix_share as isize + process_mix_changes[p.output].get(&p.id).unwrap_or(&0);
        if share <= 0 {
            continue;
        }
        let output = share as f32 / 20. * demand[p.output];
        let entry = rates
            .entry(format!("{feedstock:?}"))
            .or_insert((feedstock, 0., vec![]));
        entry.1 += output * p.adj_feedstock_amount();
        entry.2.push(p.name.clone());
    }
    // Batteries draw on lithium too.
    if let Some(entry) = rates.get_mut("Lithium") {
        entry.1 += state.storage.last.lithium_mined;
    }
    let mut warnings: Vec<_> = rates
        .into_values()
        .filter_map(|(feedstock, rate, names)| {
            let years = if rate > 0. { state.feedstocks.available[feedstock] / rate } else { f32::INFINITY };
            (years < WARN_YEARS).then_some((feedstock, years, names))
        })
        .collect();
    warnings.sort_by(|a, b| a.1.total_cmp(&b.1));
    warnings
}

/// The process's maximum share, counting planned changes to wind and solar
/// and to electricity demand.
pub fn planned_max_share(
    process: &Process,
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> usize {
    if state.is_intermittent_electricity(&process.id) {
        let others = planned_intermittent_others(process, state, process_mix_changes);
        let electricity = planned_electricity(state, process_mix_changes);
        state.process_max_share_planned(&process.id, Some(others), Some(electricity))
    } else {
        state.process_max_share(&process.id)
    }
}

fn is_addable(
    process: &Process,
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
    points: isize,
) -> bool {
    let max_share = planned_max_share(process, state, process_mix_changes);
    let change = process_mix_changes[process.output]
        .get(&process.id)
        .unwrap_or(&0);
    // Compare the resulting share (not just the change) with the maximum.
    let within_max = (process.mix_share as isize + *change) < max_share as isize;
    points != 0 && within_max && storage_allows(process, state, process_mix_changes)
}

/// Wind and solar together can't grow past what storage can support,
/// counting planned changes to all of them.
fn storage_allows(
    process: &Process,
    state: &State,
    process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
) -> bool {
    use hes_engine::ProcessFeature;
    if process.output != Output::Electricity
        || !process.features.contains(&ProcessFeature::IsIntermittent)
    {
        return true;
    }
    let planned: isize = state
        .world
        .processes
        .iter()
        .filter(|p| p.output == Output::Electricity && p.features.contains(&ProcessFeature::IsIntermittent))
        .map(|p| p.mix_share as isize + process_mix_changes[p.output].get(&p.id).unwrap_or(&0))
        .sum();
    let supported =
        state.intermittent_cap_shares_for(planned_electricity(state, process_mix_changes)) as isize;
    planned < supported
}

fn remove_point(
    points: &mut isize,
    process: &Process,
    mix_changes: &mut EnumMap<Output, BTreeMap<Id, isize>>,
) {
    let change = mix_changes[process.output].entry(process.id).or_default();
    if process.mix_share as isize + *change > 0 {
        *points += 1;
        *change -= 1;
    }
}

// Returns the point change.
fn add_point(
    points: &mut isize,
    process: &Process,
    max_share: usize,
    mix_changes: &mut EnumMap<Output, BTreeMap<Id, isize>>,
) {
    if *points > 0 {
        let change = mix_changes[process.output].entry(process.id).or_default();
        if (process.mix_share as isize + *change) < max_share as isize {
            *points -= 1;
            *change += 1;
        }
    }
}

impl Scannable for Process {
    fn add_scan_time(&self) -> f32 {
        consts::PROCESS_CARD_SCAN_TIME
    }

    fn add_scan_done(&self, state: &mut GameState) -> ScanResult {
        let addable = is_addable(
            self,
            &state.core,
            &state.ui.process_mix_changes,
            state.ui.process_points,
        );
        if addable {
            if state.ui.tutorial == Tutorial::Processes {
                state.ui.tutorial.advance();
            }

            let max_share =
                planned_max_share(self, &state.core, &state.ui.process_mix_changes);

            add_point(
                &mut state.ui.process_points,
                self,
                max_share,
                &mut state.ui.process_mix_changes,
            );

            if state.ui.process_points > 0 {
                ScanResult::SuccessContinue
            } else {
                ScanResult::SuccessStop
            }
        } else {
            ScanResult::Rejected
        }
    }

    fn is_add_allowed(&self, state: &GameState) -> bool {
        is_addable(
            self,
            &state.core,
            &state.ui.process_mix_changes,
            state.ui.process_points,
        )
    }

    fn rem_scan_time(&self) -> f32 {
        consts::PROCESS_CARD_WITHDRAW_TIME
    }

    fn rem_scan_done(&self, state: &mut GameState) -> ScanResult {
        remove_point(
            &mut state.ui.process_points,
            self,
            &mut state.ui.process_mix_changes,
        );

        // If still subtractable, continue scanning
        if is_subtractable(self, &state.ui.process_mix_changes) {
            ScanResult::SuccessContinue
        } else {
            ScanResult::SuccessStop
        }
    }

    fn is_rem_allowed(&self, state: &GameState) -> bool {
        is_subtractable(self, &state.ui.process_mix_changes)
    }
}
