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
//! time. Lithium mined but not yet needed goes into a stockpile (up to a
//! few years of output), so a steady period builds up a buffer for a
//! rapid build-out later. If there isn't enough lithium for the storage
//! the grid needs, the energy that needed storing goes undelivered.
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
/// Mines expand steadily while they're running near capacity, and faster
/// when demand is eating into the stockpile.
const MINING_BASE_GROWTH: f32 = 0.05;
const MINING_GROWTH: f32 = 0.15;
const MAX_MINING: f32 = 4e12;

/// Mines keep producing until the stockpile holds this many years of
/// mining capacity, then slow to what's needed.
const STOCKPILE_YEARS: f32 = 3.;

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

/// Lithium (g) in the batteries of vehicles using this much electricity
/// a year (kWh).
pub fn ev_lithium_for(ev_electricity: f32) -> f32 {
    ev_electricity.max(0.) * EV_CAPACITY_PER_KWH_USED * LITHIUM_PER_KWH
}

/// Grid battery capacity (kWh) needed for a wind and solar share.
fn grid_capacity_for(electricity: f32, intermittent_share: f32) -> f32 {
    electricity * intermittent_share * stored_share(intermittent_share) / CYCLES_PER_YEAR
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
    /// Share of the grid battery capacity needed that couldn't be built.
    pub shortfall: f32,
    /// Lithium needed in electric vehicle batteries (g).
    #[serde(default)]
    pub ev_lithium_needed: f32,
    /// Lithium needed in grid batteries (g).
    #[serde(default)]
    pub grid_lithium_needed: f32,
    /// Share of the vehicle batteries needed that couldn't be built; those
    /// vehicles keep running on fuel.
    #[serde(default)]
    pub ev_shortfall: f32,
}

/// What a planned wind and solar share would need in storage and lithium
/// over a planning cycle, against the lithium that will be available.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct StorageOutlook {
    /// Planned wind and solar share of electricity.
    pub intermittent_share: f32,
    /// Grid battery capacity needed (kWh), lithium and otherwise.
    pub grid_capacity: f32,
    /// Lithium needed for grid batteries (g).
    pub grid_lithium: f32,
    /// Lithium needed for vehicle batteries (g).
    pub ev_lithium: f32,
    /// Lithium available over the cycle: batteries in use, the stockpile
    /// and mining, less what wears out and isn't recycled (g).
    pub available: f32,
    /// Lithium the grid can draw on, since it gets priority (g).
    pub for_grid: f32,
    /// The highest wind and solar share the available lithium supports.
    pub supported_share: f32,
    /// Share of the vehicle batteries needed that lithium left after the
    /// grid would cover.
    pub ev_coverage: f32,
}
impl StorageOutlook {
    /// Whether the grid's storage would fall short.
    pub fn is_short(&self) -> bool {
        self.grid_lithium > self.for_grid * 1.01
    }
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
    /// Lithium mined but not yet in batteries (g).
    #[serde(default)]
    pub stockpile: f32,
    /// Lithium in grid batteries (g); the rest of `lithium_in_use` is in
    /// vehicles.
    #[serde(default)]
    pub grid_in_use: f32,
    #[serde(default)]
    pub pools_split: bool,
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
            stockpile: 0.,
            grid_in_use: 0.,
            pools_split: false,
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

    /// Lithium available for batteries over the coming `years`: batteries
    /// in use, the stockpile, and mining at today's capacity, less what
    /// wears out and isn't recycled.
    pub fn available_lithium(&self, years: f32) -> f32 {
        let worn_out = self.lithium_in_use
            * (years / BATTERY_LIFETIME).min(1.)
            * (1. - self.recycling.clamp(0., 1.));
        self.lithium_in_use + self.stockpile + self.mining_capacity * years - worn_out
    }

    /// Lithium the grid can draw on over the coming `years`: everything
    /// available except what stays in vehicle batteries still in service
    /// (the grid gets priority on new lithium).
    pub fn available_for_grid(&self, years: f32) -> f32 {
        let ev_in_use = (self.lithium_in_use - self.grid_in_use).max(0.);
        let ev_still_in_service = ev_in_use * (1. - (years / BATTERY_LIFETIME).min(1.));
        (self.available_lithium(years) - ev_still_in_service).max(0.)
    }

    /// What `intermittent_share` of `electricity` would need over the
    /// coming `years`, with vehicles needing `ev_lithium`.
    pub fn outlook(
        &self,
        electricity: f32,
        intermittent_share: f32,
        ev_lithium: f32,
        tech: StorageTech,
        years: f32,
    ) -> StorageOutlook {
        let mix = StorageMix::new(tech);
        let grid_capacity = grid_capacity_for(electricity, intermittent_share);
        let available = self.available_lithium(years);
        let for_grid = self.available_for_grid(years);
        let supported_share = if mix.lithium <= 0. || electricity <= 0. {
            1.
        } else {
            let capacity = for_grid / (LITHIUM_PER_KWH * mix.lithium);
            if grid_capacity_for(electricity, 1.) <= capacity {
                1.
            } else {
                let (mut lo, mut hi) = (STORAGE_THRESHOLD, 1.);
                for _ in 0..30 {
                    let mid = (lo + hi) / 2.;
                    if grid_capacity_for(electricity, mid) <= capacity {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                lo
            }
        };
        let grid_lithium = grid_capacity * mix.lithium * LITHIUM_PER_KWH;
        let ev_coverage = if ev_lithium > 0. {
            ((available - grid_lithium) / ev_lithium).clamp(0., 1.)
        } else {
            1.
        };
        StorageOutlook {
            intermittent_share,
            grid_capacity,
            grid_lithium,
            ev_lithium,
            available,
            for_grid,
            supported_share,
            ev_coverage,
        }
    }

    /// The highest wind and solar share of electricity that storage can
    /// support over the coming `years`, with vehicles' needs as now.
    pub fn supported_share(&self, electricity: f32, tech: StorageTech, years: f32) -> f32 {
        self.outlook(electricity, 0., self.last.ev_lithium_needed, tech, years)
            .supported_share
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
        let ev_needed = ev_lithium_for(ev_electricity);
        let grid_needed = grid_capacity * mix.lithium * LITHIUM_PER_KWH;

        // Batteries already in use when the game starts (today's electric
        // vehicles and grid storage).
        if !self.initialized {
            self.lithium_in_use = grid_needed + ev_needed;
            self.grid_in_use = grid_needed;
            self.initialized = true;
            self.pools_split = true;
        }
        // Older saves tracked one pool.
        if !self.pools_split {
            self.grid_in_use = self.lithium_in_use.min(grid_needed);
            self.pools_split = true;
        }

        // Batteries that wear out this year, and how much of their lithium
        // is recovered.
        let ev_in_use = (self.lithium_in_use - self.grid_in_use).max(0.);
        let grid_kept = self.grid_in_use * (1. - 1. / BATTERY_LIFETIME);
        let ev_kept = ev_in_use * (1. - 1. / BATTERY_LIFETIME);
        let recovered = self.lithium_in_use / BATTERY_LIFETIME * self.recycling.clamp(0., 1.);
        let new_grid = (grid_needed - grid_kept).max(0.);
        let new_ev = (ev_needed - ev_kept).max(0.);
        let new = new_grid + new_ev;

        // Mines produce what's needed plus enough to fill the stockpile.
        let target = new - recovered - self.stockpile + STOCKPILE_YEARS * self.mining_capacity;
        let mined = target
            .clamp(0., self.mining_capacity)
            .min(lithium_available.max(0.));

        // The grid gets lithium first: a shortfall there means blackouts,
        // while vehicles without batteries just keep burning fuel.
        let mut supply = self.stockpile + recovered + mined;
        let grid_used = new_grid.min(supply);
        supply -= grid_used;
        let ev_used = new_ev.min(supply);
        self.stockpile = supply - ev_used;
        self.grid_in_use = grid_kept + grid_used;
        self.lithium_in_use = self.grid_in_use + ev_kept + ev_used;

        let shortfall = if grid_needed > 0. {
            (1. - self.grid_in_use / grid_needed).clamp(0., 1.)
        } else {
            0.
        };
        let ev_shortfall = if ev_needed > 0. {
            (1. - (ev_kept + ev_used) / ev_needed).clamp(0., 1.)
        } else {
            0.
        };

        // New mines open over time: steadily while mines run near capacity,
        // faster when demand outpaces mining.
        let modifier = (1. + self.mining_modifier).max(0.);
        let growth = if new > recovered + self.mining_capacity * 0.8 {
            MINING_GROWTH * modifier
        } else if mined > 0.8 * self.mining_capacity {
            MINING_BASE_GROWTH * modifier
        } else {
            0.
        };
        self.mining_capacity = (self.mining_capacity * (1. + growth)).min(MAX_MINING);
        let demand = new;

        let losses = stored * mix.losses_per_kwh() * (1. + self.losses_modifier).max(0.);
        self.last = StorageReport {
            stored,
            losses,
            curtailed: stored * shortfall * CURTAILMENT_SHARE,
            lithium_demand: demand,
            lithium_mined: mined,
            shortfall,
            ev_lithium_needed: ev_needed,
            grid_lithium_needed: grid_needed,
            ev_shortfall,
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

    #[test]
    fn steady_years_build_a_stockpile_for_later() {
        let mut steady = Storage::default();
        steady.step(ELEC, 0.1, 5e11, StorageTech::default(), 1e14);
        for _ in 0..10 {
            steady.step(ELEC, 0.25, 5e11, StorageTech::default(), 1e14);
        }
        assert!(steady.stockpile > 2. * STARTING_MINING, "{}", steady.stockpile);
        assert!(steady.mining_capacity > STARTING_MINING * 1.2);

        // Jumping to 60% wind and solar goes better after the steady decade.
        let mut rushed = Storage::default();
        rushed.step(ELEC, 0.1, 5e11, StorageTech::default(), 1e14);
        let a = steady.step(ELEC, 0.6, 5e11, StorageTech::default(), 1e14).shortfall;
        let b = rushed.step(ELEC, 0.6, 5e11, StorageTech::default(), 1e14).shortfall;
        assert!(a < b * 0.6, "{a} vs {b}");
    }

    #[test]
    fn grid_gets_lithium_before_vehicles() {
        let tech = StorageTech::default();
        // Electrifying every vehicle at once wants far more lithium than
        // mines can supply...
        let mut storage = Storage::default();
        storage.step(ELEC, 0.3, 5e11, tech, 1e14);
        let report = storage.step(ELEC, 0.3, 7e12, tech, 1e14);
        assert!(report.ev_shortfall > 0.5, "{}", report.ev_shortfall);
        // ...but the grid still gets its batteries.
        assert!(report.shortfall < 0.05, "{}", report.shortfall);

        let outlook = storage.outlook(ELEC, 0.5, 7e12 * 0.02 * 100., tech, 5.);
        assert!(outlook.ev_coverage < 0.5);
        let without = storage.outlook(ELEC, 0.5, 0., tech, 5.);
        assert_eq!(outlook.supported_share, without.supported_share);
    }
}
