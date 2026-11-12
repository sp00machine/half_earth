//! Energy storage and lithium.
//!
//! A grid with a lot of wind and solar has to shift energy from when it's
//! generated to when it's needed. Below ~20% wind and solar this barely
//! matters; above it the need climbs steeply. Storage loses some energy on
//! every round trip, which shows up as extra electricity demand.
//!
//! Most storage, and nearly all electric vehicles, use lithium-ion
//! batteries. Lithium comes from the world's reserves, but mines can only
//! be opened so fast: supply starts near today's output and ramps up over
//! time. If there isn't enough lithium for the storage the grid needs,
//! energy that couldn't be stored is partly wasted, raising demand further.
//!
//! Units: energy in kWh, lithium in grams.

use serde::{Deserialize, Serialize};

/// Wind and solar share of electricity below which storage is negligible:
/// existing flexibility (hydro, gas peakers, interconnectors, demand
/// response) absorbs the variation.
const STORAGE_THRESHOLD: f32 = 0.2;

/// How steeply the need for storage climbs past the threshold.
const STORAGE_EXPONENT: f32 = 1.2;

/// Share of wind and solar output that passes through storage at 100%
/// wind and solar.
const MAX_STORED_SHARE: f32 = 0.45;

/// Effective full cycles per year for grid storage. Batteries cycle about
/// daily, but have to be sized for bad days (cloudy, still weather) rather
/// than the average one, so each kWh of capacity moves less than 365 kWh
/// a year.
const CYCLES_PER_YEAR: f32 = 200.;

/// Lithium per kWh of battery capacity (~0.1 kg).
const LITHIUM_PER_KWH: f32 = 100.;

/// Battery capacity per kWh of electricity used by electric vehicles
/// per year (a ~60 kWh battery for ~3,000 kWh/yr of driving).
const EV_CAPACITY_PER_KWH_USED: f32 = 0.02;

/// Years before batteries need replacing (grid and vehicles averaged).
const BATTERY_LIFETIME: f32 = 13.;

/// Lithium mining at the start of the game (g/yr, ~180 kt) and how fast
/// new mines can open (fractional growth per year), up to a ceiling.
const STARTING_MINING: f32 = 1.8e11;
const MINING_GROWTH: f32 = 0.15;
const MAX_MINING: f32 = 4e12;

/// Share of energy that needed storage which goes undelivered if the
/// storage can't be built.
const CURTAILMENT_SHARE: f32 = 1.0;

/// Round-trip efficiency of each storage technology.
const LITHIUM_EFFICIENCY: f32 = 0.9;
const SODIUM_EFFICIENCY: f32 = 0.88;
const PUMPED_HYDRO_EFFICIENCY: f32 = 0.8;
const LONG_DURATION_EFFICIENCY: f32 = 0.45;

/// Share of grid storage each alternative takes over once available.
const SODIUM_SHARE: f32 = 0.4;
const PUMPED_HYDRO_SHARE: f32 = 0.15;
const LONG_DURATION_SHARE: f32 = 0.3;

/// Which storage technologies are available.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct StorageTech {
    pub sodium_ion: bool,
    pub pumped_hydro: bool,
    pub long_duration: bool,
}

/// How grid storage is split between technologies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StorageMix {
    pub lithium: f32,
    pub sodium: f32,
    pub pumped_hydro: f32,
    pub long_duration: f32,
}
impl StorageMix {
    pub fn new(tech: StorageTech) -> Self {
        let pumped_hydro = if tech.pumped_hydro { PUMPED_HYDRO_SHARE } else { 0. };
        let long_duration = if tech.long_duration { LONG_DURATION_SHARE } else { 0. };
        let batteries = 1. - pumped_hydro - long_duration;
        let sodium = if tech.sodium_ion { batteries * SODIUM_SHARE } else { 0. };
        StorageMix {
            lithium: batteries - sodium,
            sodium,
            pumped_hydro,
            long_duration,
        }
    }

    /// Energy lost per kWh stored.
    fn losses_per_kwh(&self) -> f32 {
        let loss = |eff: f32| 1. / eff - 1.;
        self.lithium * loss(LITHIUM_EFFICIENCY)
            + self.sodium * loss(SODIUM_EFFICIENCY)
            + self.pumped_hydro * loss(PUMPED_HYDRO_EFFICIENCY)
            + self.long_duration * loss(LONG_DURATION_EFFICIENCY)
    }
}

/// Share of wind and solar output that has to go through storage.
pub fn stored_share(intermittent_share: f32) -> f32 {
    let over = ((intermittent_share - STORAGE_THRESHOLD) / (1. - STORAGE_THRESHOLD)).clamp(0., 1.);
    MAX_STORED_SHARE * over.powf(STORAGE_EXPONENT)
}

/// This year's storage and lithium.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StorageReport {
    /// Energy passed through storage (kWh/yr).
    pub stored: f32,
    /// Energy lost to round trips (kWh/yr).
    pub losses: f32,
    /// Electricity undelivered for lack of storage (kWh/yr).
    pub curtailed: f32,
    /// Lithium needed this year (g).
    pub lithium_demand: f32,
    /// Lithium actually mined this year (g).
    pub lithium_mined: f32,
    /// Share of needed battery capacity that couldn't be built.
    pub shortfall: f32,
    /// Lithium needed in electric vehicle batteries (g).
    #[serde(default)]
    pub ev_lithium_needed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Storage {
    /// Lithium in batteries currently in use (g).
    pub lithium_in_use: f32,
    /// Lithium that can be mined this year (g).
    pub mining_capacity: f32,
    /// Share of lithium recovered from retired batteries.
    pub recycling: f32,
    /// Change to how fast lithium mining can grow (fraction).
    pub mining_modifier: f32,
    /// Change to round-trip losses (fraction).
    pub losses_modifier: f32,
    pub last: StorageReport,
    /// Whether the batteries already in use at the start have been counted.
    #[serde(default)]
    pub initialized: bool,
}
impl Default for Storage {
    fn default() -> Self {
        Self {
            lithium_in_use: 0.,
            mining_capacity: STARTING_MINING,
            recycling: 0.,
            mining_modifier: 0.,
            losses_modifier: 0.,
            last: StorageReport::default(),
            initialized: false,
        }
    }
}

impl Storage {
    /// Extra electricity demand from storage round-trip losses (kWh).
    pub fn extra_demand(&self, electricity: f32, intermittent_share: f32, tech: StorageTech) -> f32 {
        let stored = electricity * intermittent_share * stored_share(intermittent_share);
        let mix = StorageMix::new(tech);
        stored * mix.losses_per_kwh() * (1. + self.losses_modifier).max(0.)
    }

    /// Electricity that can't be delivered because the storage it needs
    /// doesn't exist: wind and solar power arrives when it isn't needed and
    /// is missing when it is (kWh). This shows up as a real shortage.
    pub fn unserved(&self, electricity: f32, intermittent_share: f32) -> f32 {
        let stored = electricity * intermittent_share * stored_share(intermittent_share);
        stored * self.last.shortfall * CURTAILMENT_SHARE
    }

    /// The highest wind and solar share of electricity that storage can
    /// support over the coming `years`: batteries already built that aren't
    /// in vehicles, plus about half of what mines can supply in that time
    /// (vehicles take the rest). Storage that doesn't need lithium is
    /// assumed unlimited.
    pub fn supported_share(&self, electricity: f32, tech: StorageTech, years: f32) -> f32 {
        let mix = StorageMix::new(tech);
        if mix.lithium <= 0. || electricity <= 0. {
            return 1.;
        }
        let lithium = (self.lithium_in_use - self.last.ev_lithium_needed).max(0.)
            + 0.5 * self.mining_capacity * years;
        let capacity = lithium / (LITHIUM_PER_KWH * mix.lithium);
        let needed = |share: f32| electricity * share * stored_share(share) / CYCLES_PER_YEAR;
        if needed(1.) <= capacity {
            return 1.;
        }
        let (mut lo, mut hi) = (STORAGE_THRESHOLD, 1.);
        for _ in 0..30 {
            let mid = (lo + hi) / 2.;
            if needed(mid) <= capacity {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Advance one year. `lithium_available` is what's left in reserves.
    pub fn step(
        &mut self,
        electricity: f32,
        intermittent_share: f32,
        ev_electricity: f32,
        tech: StorageTech,
        lithium_available: f32,
    ) -> StorageReport {
        let mix = StorageMix::new(tech);
        let stored = electricity * intermittent_share * stored_share(intermittent_share);
        let grid_capacity = stored / CYCLES_PER_YEAR;
        let ev_capacity = ev_electricity.max(0.) * EV_CAPACITY_PER_KWH_USED;
        let needed = (grid_capacity * mix.lithium + ev_capacity) * LITHIUM_PER_KWH;

        // Batteries already in use when the game starts (today's electric
        // vehicles and grid storage).
        if !self.initialized {
            self.lithium_in_use = needed;
            self.initialized = true;
        }

        // Batteries that wear out this year, and how much of their lithium
        // is recovered.
        let retired = self.lithium_in_use / BATTERY_LIFETIME;
        let recovered = retired * self.recycling.clamp(0., 1.);
        let in_use = self.lithium_in_use - retired;

        let new = (needed - in_use).max(0.);
        let from_recycling = recovered.min(new);
        let demand = new - from_recycling;
        let mined = demand.min(self.mining_capacity).min(lithium_available.max(0.));
        self.lithium_in_use = (in_use + from_recycling + mined).min(needed.max(in_use));

        // Lithium is shared between vehicles and the grid; only the grid's
        // part of any shortfall affects electricity (wasted power, brownouts).
        let shortfall = if needed > 0. && grid_capacity * mix.lithium > 0. {
            (1. - self.lithium_in_use / needed).clamp(0., 1.)
        } else {
            0.
        };

        // New mines open over time, faster when demand outstrips supply.
        let growth = MINING_GROWTH * (1. + self.mining_modifier).max(0.);
        if demand > 0.8 * self.mining_capacity {
            self.mining_capacity = (self.mining_capacity * (1. + growth)).min(MAX_MINING);
        }

        let losses = stored * mix.losses_per_kwh() * (1. + self.losses_modifier).max(0.);
        self.last = StorageReport {
            stored,
            losses,
            curtailed: stored * shortfall * CURTAILMENT_SHARE,
            lithium_demand: demand,
            lithium_mined: mined,
            shortfall,
            ev_lithium_needed: ev_capacity * LITHIUM_PER_KWH,
        };
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELEC: f32 = 3e13; // 30 PWh

    #[test]
    fn little_storage_below_threshold() {
        assert_eq!(stored_share(0.2), 0.);
        assert!(stored_share(0.5) < 0.15);
        assert!(stored_share(0.9) > 0.3);
    }

    #[test]
    fn high_renewables_need_lithium_and_mines_ramp_up() {
        let mut storage = Storage::default();
        // Start from today's grid and a small existing EV fleet.
        storage.step(ELEC, 0.1, 5e11, StorageTech::default(), 1e14);
        let mut mined_first = None;
        for year in 0..30 {
            let report = storage.step(ELEC, 0.8, 5e12, StorageTech::default(), 1e14);
            if year == 0 {
                mined_first = Some(report.lithium_mined);
                // Can't build it all at once.
                assert!(report.shortfall > 0.5);
            }
        }
        assert!(storage.mining_capacity > mined_first.unwrap() * 5.);
        assert!(storage.last.shortfall < 0.05, "{}", storage.last.shortfall);
        // ~2 Mt of lithium in grid storage (~6 hours of average demand) plus
        // ~10 Mt in a fully electric vehicle fleet (~2 billion 60 kWh cars).
        assert!((8e12..15e12).contains(&storage.lithium_in_use), "{}", storage.lithium_in_use);
    }

    #[test]
    fn alternatives_cut_lithium_but_some_cost_efficiency() {
        let all = StorageTech { sodium_ion: true, pumped_hydro: true, long_duration: true };
        let mix = StorageMix::new(all);
        let base = StorageMix::new(StorageTech::default());
        assert!(mix.lithium < base.lithium * 0.5);
        assert!(mix.losses_per_kwh() > base.losses_per_kwh());
    }

    #[test]
    fn recycling_reduces_mining() {
        let run = |recycling: f32| {
            let mut storage = Storage { recycling, ..Default::default() };
            storage.step(ELEC, 0.1, 5e11, StorageTech::default(), 1e14);
            let mut mined = 0.;
            for _ in 0..40 {
                mined += storage.step(ELEC, 0.8, 5e12, StorageTech::default(), 1e14).lithium_mined;
            }
            mined
        };
        assert!(run(0.8) < run(0.) * 0.7);
    }

    #[test]
    fn running_out_of_lithium_causes_shortfall() {
        let mut storage = Storage::default();
        storage.step(ELEC, 0.1, 5e11, StorageTech::default(), 0.);
        for _ in 0..20 {
            storage.step(ELEC, 0.8, 5e12, StorageTech::default(), 0.);
        }
        assert!(storage.last.shortfall > 0.95, "{}", storage.last.shortfall);
        assert!(storage.unserved(ELEC, 0.8) > 0.);
    }
}
