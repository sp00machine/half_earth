//! Land-use change carbon.
//!
//! Clearing land releases the carbon stored in its vegetation and soil;
//! letting farmland go back to nature slowly pulls that carbon back in.
//! The two are not symmetric: clearing releases most of its carbon at
//! once (the rest leaks out of the soil over a few decades), while
//! regrowth takes many decades to recover what was lost.
//!
//! The game has a single global pool of land, so we can't track which
//! biome is cleared. Instead the carbon density of land rises with how
//! much land is already in use: the first land farmed is low-carbon
//! (grassland, degraded or long-farmed land), the last is forest and peat.
//!
//! Units: areas in m², carbon in grams of carbon (gC), emissions in g CO2.

use serde::{Deserialize, Serialize};

/// Grams of CO2 per gram of carbon.
const CO2_PER_C: f32 = 44. / 12.;

/// Carbon density of the least and most carbon-dense land (gC/m²).
/// 1 tC/ha = 100 gC/m². ~20 tC/ha for dry grassland, ~200 tC/ha for
/// dense forest including soil.
const MIN_DENSITY: f32 = 2_000.;
const MAX_DENSITY: f32 = 20_000.;

/// Share of cleared carbon released immediately (burning and decay of
/// vegetation); the rest is lost from soils over the following decades.
const IMMEDIATE_SHARE: f32 = 0.7;

/// Soil carbon from past clearing leaks out with this time constant (years).
const SOIL_TIME_CONSTANT: f32 = 25.;

/// Regrowing land approaches its full carbon stock with this time
/// constant (years), so it recovers ~50% in 28 years and ~85% in 75.
const REGROWTH_TIME_CONSTANT: f32 = 40.;

/// Gross deforestation at the start of the game (m²/yr), from shifting
/// agriculture, degradation, logging and so on, even with farmland flat.
/// ~10 Mha/yr (FAO Global Forest Resources Assessment, 2015-2020).
const BASE_DEFORESTATION: f32 = 1e11;

/// Carbon density of land lost to background deforestation (gC/m²).
const DEFORESTATION_DENSITY: f32 = 10_000.;

/// Land protection at the start of the game.
const START_PROTECTION: f32 = 0.1;

/// Land that was farmed and is now regrowing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cohort {
    /// m²
    pub area: f32,
    /// Years since this land stopped being farmed.
    pub age: f32,
    /// Carbon stock this land recovers to when fully regrown (gC/m²).
    pub full_stock: f32,
}
impl Cohort {
    /// Carbon stock per m² at a given age.
    fn stock_at(&self, age: f32, speed: f32) -> f32 {
        self.full_stock * (1. - (-age * speed / REGROWTH_TIME_CONSTANT).exp())
    }

    /// Carbon currently stored (gC).
    fn stored(&self, speed: f32) -> f32 {
        self.area * self.stock_at(self.age, speed)
    }
}

/// This year's land-use emissions, by source (g CO2/yr; negative is removal).
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LandEmissions {
    /// Newly cleared land (or regrowing land cleared again).
    pub clearing: f32,
    /// Soil carbon still leaking from past clearing.
    pub soil: f32,
    /// Background deforestation.
    pub deforestation: f32,
    /// Carbon taken up by regrowing land.
    pub regrowth: f32,
}
impl LandEmissions {
    pub fn total(&self) -> f32 {
        self.clearing + self.soil + self.deforestation + self.regrowth
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandCarbon {
    /// Land in use last year (m²); `None` before the first year.
    pub prev_used: Option<f32>,

    /// Farmland that is regrowing, oldest first.
    pub cohorts: Vec<Cohort>,

    /// Soil carbon still to be released from past clearing (gC).
    pub soil_debt: f32,

    /// Change to background deforestation from projects, as a fraction
    /// (-0.3 means 30% less deforestation).
    pub deforestation_modifier: f32,

    /// Change to how fast land regrows, as a fraction
    /// (0.2 means 20% faster).
    pub regrowth_modifier: f32,

    /// The most recent year's emissions.
    pub last: LandEmissions,
}

impl LandCarbon {
    fn regrowth_speed(&self) -> f32 {
        (1. + self.regrowth_modifier).max(0.)
    }

    /// Carbon density of land at a given fraction of all land in use (gC/m²).
    pub fn density_at(fraction: f32) -> f32 {
        let f = fraction.clamp(0., 1.);
        MIN_DENSITY + (MAX_DENSITY - MIN_DENSITY) * f * f
    }

    /// Average density for land between two fractions of total land in use.
    fn mean_density(from: f32, to: f32) -> f32 {
        // Average of a quadratic over the interval.
        let (a, b) = (from.clamp(0., 1.), to.clamp(0., 1.));
        if (b - a).abs() < 1e-9 {
            return Self::density_at(a);
        }
        let integral = |x: f32| MIN_DENSITY * x + (MAX_DENSITY - MIN_DENSITY) * x * x * x / 3.;
        (integral(b) - integral(a)) / (b - a)
    }

    pub fn regrowing_area(&self) -> f32 {
        self.cohorts.iter().map(|c| c.area).sum()
    }

    /// Background deforestation this year (m²).
    pub fn deforestation_area(&self, protected_land: f32) -> f32 {
        let protection = ((1. - protected_land) / (1. - START_PROTECTION)).clamp(0., 1.);
        BASE_DEFORESTATION * protection * (1. + self.deforestation_modifier).max(0.)
    }

    /// Advance one year. `used` is land in use this year and `total`
    /// is all land (m²). Returns this year's emissions.
    pub fn step(&mut self, used: f32, total: f32, protected_land: f32) -> LandEmissions {
        let speed = self.regrowth_speed();
        let mut out = LandEmissions::default();

        let prev = match self.prev_used {
            Some(prev) => prev,
            None => {
                // First year: assume soils are already leaking at the rate
                // that ongoing background deforestation sustains.
                let area = self.deforestation_area(protected_land);
                self.soil_debt = area
                    * DEFORESTATION_DENSITY
                    * (1. - IMMEDIATE_SHARE)
                    * SOIL_TIME_CONSTANT;
                used
            }
        };
        self.prev_used = Some(used);

        let change = used - prev;
        if change > 0. {
            // Clear regrowing land first, most recently abandoned first.
            let mut to_clear = change;
            let mut released = 0.;
            while to_clear > 0. {
                let Some(cohort) = self.cohorts.last_mut() else {
                    break;
                };
                let take = cohort.area.min(to_clear);
                released += take * cohort.stock_at(cohort.age, speed);
                cohort.area -= take;
                to_clear -= take;
                if cohort.area <= 0. {
                    self.cohorts.pop();
                }
            }

            // Then wild land, at the edge of what is already used or regrowing.
            if to_clear > 0. {
                let edge = (prev + self.regrowing_area()) / total;
                let density = Self::mean_density(edge, edge + to_clear / total);
                let carbon = to_clear * density;
                released += carbon * IMMEDIATE_SHARE;
                self.soil_debt += carbon * (1. - IMMEDIATE_SHARE);
            }
            out.clearing = released * CO2_PER_C;
        } else if change < 0. {
            // Freed land starts regrowing toward the density it had
            // before it was farmed.
            let freed = -change;
            let full_stock = Self::mean_density(used / total, prev / total);
            self.cohorts.push(Cohort {
                area: freed,
                age: 0.,
                full_stock,
            });
        }

        // Regrowth uptake this year.
        let mut uptake = 0.;
        for cohort in self.cohorts.iter_mut() {
            let before = cohort.stored(speed);
            cohort.age += 1.;
            uptake += cohort.stored(speed) - before;
        }
        out.regrowth = -uptake * CO2_PER_C;

        // Soil leakage from past clearing.
        let soil = self.soil_debt / SOIL_TIME_CONSTANT;
        self.soil_debt -= soil;

        // Background deforestation.
        let deforested = self.deforestation_area(protected_land) * DEFORESTATION_DENSITY;
        let soil_from_deforestation = deforested * (1. - IMMEDIATE_SHARE);
        self.soil_debt += soil_from_deforestation;
        out.soil = soil * CO2_PER_C;
        out.deforestation = deforested * IMMEDIATE_SHARE * CO2_PER_C;

        self.last = out;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTAL: f32 = 1.04e14;
    const START_USED: f32 = 5e13;

    fn gt(g: f32) -> f32 {
        g * 1e-15
    }

    #[test]
    fn starting_emissions_match_todays_land_use_change() {
        // Global net land-use change emissions are ~4 Gt CO2/yr
        // (Global Carbon Budget).
        let mut land = LandCarbon::default();
        let first = land.step(START_USED, TOTAL, START_PROTECTION);
        let second = land.step(START_USED, TOTAL, START_PROTECTION);
        for year in [first, second] {
            assert!((3.0..5.0).contains(&gt(year.total())), "{}", gt(year.total()));
            assert_eq!(year.clearing, 0.);
            assert_eq!(year.regrowth, 0.);
        }
    }

    #[test]
    fn clearing_is_fast_and_regrowth_slow() {
        let mut land = LandCarbon::default();
        land.step(START_USED, TOTAL, START_PROTECTION);
        let baseline = land.step(START_USED, TOTAL, START_PROTECTION).total();

        // Clear 5 Mkm² in one year, then give it back the next.
        let cleared = land.step(START_USED + 5e12, TOTAL, START_PROTECTION);
        assert!(gt(cleared.clearing) > 10.);
        land.step(START_USED, TOTAL, START_PROTECTION);

        // Cumulative emissions relative to the baseline over the following
        // decades: the round trip is carbon neutral in the long run, but
        // regrowth takes far longer than clearing.
        let released = cleared.total() - baseline;
        let mut cumulative = released;
        let mut years_to_recover = None;
        for year in 1..300 {
            cumulative += land.step(START_USED, TOTAL, START_PROTECTION).total() - baseline;
            if cumulative <= released * 0.25 && years_to_recover.is_none() {
                years_to_recover = Some(year);
            }
        }
        let years = years_to_recover.expect("eventually recovers");
        assert!(years > 40, "recovered 75% in {years} years");
        assert!(cumulative.abs() < released * 0.05, "long run is carbon neutral");
    }

    #[test]
    fn regrowing_land_cleared_again_releases_only_what_it_regrew() {
        let mut land = LandCarbon::default();
        land.step(START_USED, TOTAL, START_PROTECTION);
        land.step(START_USED - 5e12, TOTAL, START_PROTECTION);
        for _ in 0..5 {
            land.step(START_USED - 5e12, TOTAL, START_PROTECTION);
        }
        let reclear = land.step(START_USED, TOTAL, START_PROTECTION);

        let mut fresh = LandCarbon::default();
        fresh.step(START_USED, TOTAL, START_PROTECTION);
        let wild = fresh.step(START_USED + 5e12, TOTAL, START_PROTECTION);
        assert!(reclear.clearing < wild.clearing * 0.3);
        assert!(land.cohorts.is_empty());
    }

    #[test]
    fn protection_and_modifiers_reduce_deforestation() {
        let land = LandCarbon::default();
        let base = land.deforestation_area(START_PROTECTION);
        assert!(land.deforestation_area(0.5) < base * 0.6);
        let protected = LandCarbon {
            deforestation_modifier: -0.5,
            ..Default::default()
        };
        assert!((protected.deforestation_area(START_PROTECTION) - base * 0.5).abs() < 1.);
    }

    #[test]
    fn freeing_a_third_of_farmland_is_a_large_sink() {
        // Sanity check against Hayek et al. (2021), which estimates that
        // ending most animal agriculture and letting the land regrow would
        // remove several hundred Gt CO2 this century.
        let mut land = LandCarbon::default();
        land.step(START_USED, TOTAL, START_PROTECTION);
        let freed_used = START_USED - 2e13;
        let mut removed = 0.;
        for _ in 0..80 {
            removed -= land.step(freed_used, TOTAL, START_PROTECTION).regrowth;
        }
        let removed = gt(removed);
        assert!((200.0..700.0).contains(&removed), "{removed} Gt");
    }
}
