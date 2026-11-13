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

/// Years over which clearing and abandonment catch up with land demand.
/// A bad harvest or a sudden policy change doesn't bulldoze forest the
/// same year: short shortfalls are covered by stocks, fallow land and
/// heavier grazing, and clearing follows sustained demand.
const LAND_RESPONSE_YEARS: f32 = 5.;

/// The most land that can be newly cleared in a year (m²): about three
/// times today's global deforestation, a world-wide clearing drive. A big
/// jump in land demand (e.g. lower yields) is cleared over many years;
/// meanwhile the gap is covered by fallow and degraded land and heavier
/// grazing.
const MAX_CLEARING_RATE: f32 = 3e11;

/// Land protection at the start of the game.
const START_PROTECTION: f32 = 0.1;

/// Protected regrowing land recovers this much faster than unmanaged land:
/// no fire setting, grazing or wood cutting, and often assisted natural
/// regeneration (planting into gaps, seed sources, controlling invasives).
/// With the 40-year time constant, protected land reaches 90% of its old
/// carbon in about 70 years, in line with tropical secondary forests
/// (Poorter et al. 2016, Nature: median 66 years to 90% of old-growth
/// biomass); unmanaged land takes about 90.
const PROTECTED_REGROWTH_SPEED: f32 = 1.3;

/// Share of young, unprotected regrowing land cleared again each year (for
/// shifting cultivation, grazing, firewood) and set back to bare ground.
/// Much young secondary forest doesn't last: in the Brazilian Amazon about
/// half is cleared again within 20 years (Smith et al. 2020). Older
/// regrowth is mostly left alone.
const RECLEARING_RATE: f32 = 0.03;
const RECLEARING_MAX_AGE: f32 = 20.;

/// Regrowing land counts as wild again once it has recovered 95% of its
/// carbon (three time constants of regrowth).
const MATURE_AGE: f32 = 3. * REGROWTH_TIME_CONSTANT;

/// Soil health of farmland at the start of the game (0 to 1). About a
/// third of the world's soils are moderately to highly degraded (FAO
/// Status of the World's Soil Resources, 2015).
pub const START_SOIL_HEALTH: f32 = 0.7;

/// Soil health moves toward what current farming sustains with this time
/// constant (years): soils degrade and recover over decades.
const SOIL_HEALTH_TIME_CONSTANT: f32 = 30.;

/// How hard intensive farming (synthetic fertilizer, monocrops, bare soil
/// between crops) pushes soil health down: farming that is all intensive,
/// with no regenerative practices, sustains health of 1 - this.
const INTENSIVE_SOIL_PRESSURE: f32 = 0.33;

/// Soil carbon on farmland between dead (0) and fully healthy (1) soil,
/// gC/m² (about 8 tC/ha). Restoring the world's farm soils from 0.7 to
/// full health takes up ~45 Gt CO2 over the better part of a century,
/// about 1.5 Gt a year at first, in the range of estimates of soil carbon
/// sequestration potential (Lal 2004; Bossio et al. 2020).
const SOIL_CARBON_PER_HEALTH: f32 = 800.;

/// How much soil health changes farm yields: each 0.1 of health below the
/// starting level means 10% more land for the same harvest, and above it
/// 10% less. Degraded soils hold less water and nutrients; yield losses
/// of 10-40% on degraded land are typical (IPBES 2018).
pub const SOIL_YIELD_EFFECT: f32 = 1.0;

/// Land that was farmed and is now regrowing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cohort {
    /// m²
    pub area: f32,
    /// How far this land has regrown, in years at normal speed. It advances
    /// faster or slower than real time depending on regrowth conditions.
    pub age: f32,
    /// Carbon stock this land recovers to when fully regrown (gC/m²).
    pub full_stock: f32,
    /// Under protection: it regrows faster and is never cleared again.
    #[serde(default)]
    pub protected: bool,
}
impl Cohort {
    /// Carbon stock per m² at a given (regrowth) age.
    fn stock_at(&self, age: f32) -> f32 {
        self.full_stock * (1. - (-age / REGROWTH_TIME_CONSTANT).exp())
    }

    /// Carbon currently stored (gC).
    fn stored(&self) -> f32 {
        self.area * self.stock_at(self.age)
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
    /// Farm soils losing carbon as they degrade, or taking it up as they
    /// recover.
    #[serde(default)]
    pub farm_soil: f32,
}
impl LandEmissions {
    pub fn total(&self) -> f32 {
        self.clearing + self.soil + self.deforestation + self.regrowth + self.farm_soil
    }
}

/// How the world's land is split (m²).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct LandPools {
    pub farmed: f32,
    pub wild: f32,
    pub regrowing: f32,
    pub wild_protected: f32,
    pub regrowing_protected: f32,
}
impl LandPools {
    pub fn protected(&self) -> f32 {
        self.wild_protected + self.regrowing_protected
    }
}

/// What land does this year.
#[derive(Debug, Clone, Copy)]
pub struct LandInputs {
    /// Land that processes want to use (m²).
    pub demand: f32,
    /// All land (m²).
    pub total: f32,
    /// Share of all land to protect.
    pub protected: f32,
    /// Share of all land whose protection goes to regrowing land first
    /// (restoration projects); the rest protects wild land first.
    pub restored: f32,
    pub temperature: f32,
    /// Share of farmland farmed intensively (synthetic fertilizer and so
    /// on), or `None` to leave soil health as it is.
    pub intensive: Option<f32>,
}

fn default_soil_health() -> f32 {
    START_SOIL_HEALTH
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    /// Override for how many years land use takes to follow demand
    /// (`None` uses the default; tests use 1 for an immediate response).
    #[serde(default)]
    pub response_years: Option<f32>,

    /// Override for the most land that can be cleared in a year (m²).
    #[serde(default)]
    pub max_clearing: Option<f32>,

    /// Wild land under protection (m²). Protected regrowing land is in
    /// `cohorts`.
    #[serde(default)]
    pub protected_wild: f32,

    /// Protection that couldn't be placed yet because all unfarmed land
    /// is already protected (m²); it's filled as farmland is freed.
    #[serde(default)]
    pub protection_pending: f32,

    /// Health of farm soils, 0 (dead) to 1 (fully healthy).
    #[serde(default = "default_soil_health")]
    pub soil_health: f32,

    /// Regenerative practices (cover crops, compost, agroforestry...), as
    /// the share of intensive farming's damage to soils they undo.
    #[serde(default)]
    pub soil_regeneration: f32,

    /// Land freed from farming over the game (m²), for events.
    #[serde(default)]
    pub total_freed: f32,
}

impl LandCarbon {
    pub fn regrowth_speed(&self) -> f32 {
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

    /// Estimated total CO2 (g) released by going from `from` to `to` m² of
    /// land in use, counting soil losses that come later. Mirrors `step`:
    /// regrowing land is cleared first, then wild land at the frontier.
    pub fn estimate_clearing(&self, from: f32, to: f32, total: f32) -> f32 {
        let mut to_clear = (to - from).max(0.);
        let mut released = 0.;
        for cohort in self.cohorts.iter().rev().filter(|c| !c.protected) {
            if to_clear <= 0. {
                break;
            }
            let take = cohort.area.min(to_clear);
            released += take * cohort.stock_at(cohort.age);
            to_clear -= take;
        }
        if to_clear > 0. {
            let edge = (from + self.regrowing_area()) / total;
            released += to_clear * Self::mean_density(edge, edge + to_clear / total);
        }
        released * CO2_PER_C
    }

    pub fn regrowing_area(&self) -> f32 {
        self.cohorts.iter().map(|c| c.area).sum()
    }

    /// Background deforestation this year (m²).
    pub fn deforestation_area(&self, protected_land: f32) -> f32 {
        let protection = ((1. - protected_land) / (1. - START_PROTECTION)).clamp(0., 1.);
        BASE_DEFORESTATION * protection * (1. + self.deforestation_modifier).max(0.)
    }

    /// How land is split, given all land (m²).
    pub fn pools(&self, total: f32) -> LandPools {
        let farmed = self.prev_used.unwrap_or(0.);
        let regrowing_protected: f32 = self
            .cohorts
            .iter()
            .filter(|c| c.protected)
            .map(|c| c.area)
            .sum();
        let regrowing = self.regrowing_area() - regrowing_protected;
        let wild_protected = self.protected_wild;
        let wild = (total - farmed - regrowing - regrowing_protected - wild_protected).max(0.);
        LandPools {
            farmed,
            wild,
            regrowing,
            wild_protected,
            regrowing_protected,
        }
    }

    /// Soil health that today's farming sustains in the long run.
    pub fn soil_health_target(&self, intensive: f32) -> f32 {
        let regen = self.soil_regeneration.clamp(0., 1.);
        (1. - INTENSIVE_SOIL_PRESSURE * intensive.clamp(0., 1.) * (1. - regen)).clamp(0., 1.)
    }

    /// How much land farming needs relative to the start of the game, from
    /// soil health alone.
    pub fn soil_land_factor(&self) -> f32 {
        (1. + SOIL_YIELD_EFFECT * (START_SOIL_HEALTH - self.soil_health)).max(0.5)
    }

    /// Protect `amount` m² of unprotected regrowing land, most recently
    /// freed first. Returns what couldn't be protected.
    fn protect_regrowing(&mut self, mut amount: f32) -> f32 {
        let mut split = vec![];
        for cohort in self.cohorts.iter_mut().rev() {
            if amount <= 0. {
                break;
            }
            if cohort.protected {
                continue;
            }
            let take = cohort.area.min(amount);
            amount -= take;
            if take >= cohort.area {
                cohort.protected = true;
            } else {
                cohort.area -= take;
                split.push(Cohort {
                    area: take,
                    protected: true,
                    ..cohort.clone()
                });
            }
        }
        self.cohorts.extend(split);
        self.sort_cohorts();
        amount
    }

    /// Protect `amount` m² of unprotected wild land. Returns what couldn't
    /// be protected.
    fn protect_wild(&mut self, amount: f32, total: f32) -> f32 {
        let free = self.pools(total).wild;
        let take = amount.min(free).max(0.);
        self.protected_wild += take;
        amount - take
    }

    /// Lift protection from `amount` m²: wild land first, then regrowing.
    fn unprotect(&mut self, mut amount: f32) {
        let take = self.protected_wild.min(amount);
        self.protected_wild -= take;
        amount -= take;
        for cohort in self.cohorts.iter_mut() {
            if amount <= 0. {
                break;
            }
            if cohort.protected {
                // Whole cohorts only; close enough for lifting protection.
                cohort.protected = false;
                amount -= cohort.area;
            }
        }
    }

    /// Oldest first (largest age); clearing takes from the end.
    fn sort_cohorts(&mut self) {
        self.cohorts.sort_by(|a, b| {
            b.age
                .partial_cmp(&a.age)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Match protected land to its target, filling restoration-first
    /// protection from regrowing land and the rest from wild land.
    fn place_protection(&mut self, inputs: &LandInputs) {
        let total = inputs.total;
        let target = inputs.protected.clamp(0., 1.) * total;
        let current = self.pools(total).protected();
        if current > target + 1. {
            self.unprotect(current - target);
            self.protection_pending = 0.;
            return;
        }
        // Restoration protection goes to regrowing land first...
        let pools = self.pools(total);
        let restore_target = inputs.restored.clamp(0., inputs.protected.max(0.)) * total;
        let restore_deficit = (restore_target - pools.regrowing_protected).max(0.);
        let left = self.protect_regrowing(restore_deficit);
        self.protect_wild(left, total);

        // ...and the rest to wild land first.
        let deficit = (target - self.pools(total).protected()).max(0.);
        let left = self.protect_wild(deficit, total);
        self.protection_pending = self.protect_regrowing(left);
    }

    /// Advance one year with the starting share of land protected
    /// (none of it restoration), leaving soils as they are.
    pub fn step(
        &mut self,
        demand: f32,
        total: f32,
        protected_land: f32,
        temperature: f32,
    ) -> LandEmissions {
        self.step_with(LandInputs {
            demand,
            total,
            protected: protected_land,
            restored: 0.,
            temperature,
            intensive: None,
        })
    }

    /// Advance one year. Returns this year's emissions.
    pub fn step_with(&mut self, inputs: LandInputs) -> LandEmissions {
        let LandInputs {
            demand,
            total,
            temperature,
            ..
        } = inputs;
        let protected_land = inputs.protected;

        // Heat, drought and fire slow regrowth as the world warms:
        // full speed at +1C, three quarters at +2C, half at +3C and above.
        let warming = (1. - 0.25 * (temperature - 1.)).clamp(0.5, 1.);
        let speed = self.regrowth_speed() * warming;
        let mut out = LandEmissions::default();

        // Land in use follows demand with a lag.
        let response = self.response_years.unwrap_or(LAND_RESPONSE_YEARS).max(1.);
        let used = match self.prev_used {
            Some(prev) => {
                prev + ((demand - prev) / response)
                    .min(self.max_clearing.unwrap_or(MAX_CLEARING_RATE))
            }
            None => demand,
        };

        let prev = match self.prev_used {
            Some(prev) => prev,
            None => {
                // First year: assume soils are already leaking at the rate
                // that ongoing background deforestation sustains.
                let area = self.deforestation_area(protected_land);
                self.soil_debt =
                    area * DEFORESTATION_DENSITY * (1. - IMMEDIATE_SHARE) * SOIL_TIME_CONSTANT;
                self.protected_wild = (protected_land * total).min((total - used).max(0.));
                used
            }
        };
        self.prev_used = Some(used);

        let change = used - prev;
        if change > 0. {
            // Clear unprotected regrowing land first, most recently
            // abandoned first.
            let mut to_clear = change;
            let mut released = 0.;
            for cohort in self.cohorts.iter_mut().rev() {
                if to_clear <= 0. {
                    break;
                }
                if cohort.protected {
                    continue;
                }
                let take = cohort.area.min(to_clear);
                released += take * cohort.stock_at(cohort.age);
                cohort.area -= take;
                to_clear -= take;
            }
            self.cohorts.retain(|c| c.area > 0.);

            // Then wild land, at the edge of what is already used or regrowing.
            if to_clear > 0. {
                let edge = (prev + self.regrowing_area()) / total;
                let density = Self::mean_density(edge, edge + to_clear / total);
                let carbon = to_clear * density;
                released += carbon * IMMEDIATE_SHARE;
                self.soil_debt += carbon * (1. - IMMEDIATE_SHARE);

                // If farming has pushed into protected wild land (protection
                // can't keep land from being farmed that already is), the
                // protection is lost.
                let over = self.pools(total);
                let overflow = -(total
                    - over.farmed
                    - over.regrowing
                    - over.regrowing_protected
                    - over.wild_protected)
                    .min(0.);
                self.protected_wild = (self.protected_wild - overflow).max(0.);
            }
            out.clearing = released * CO2_PER_C;
        } else if change < 0. {
            // Freed land starts regrowing toward the density it had
            // before it was farmed.
            let freed = -change;
            self.total_freed += freed;
            let full_stock = Self::mean_density(used / total, prev / total);
            self.cohorts.push(Cohort {
                area: freed,
                age: 0.,
                full_stock,
                protected: false,
            });
        }

        // Protection goes where its projects send it.
        self.place_protection(&inputs);

        // Some unprotected regrowing land is cleared again and starts over.
        let mut recleared_area = 0.;
        let mut recleared_stock = 0.;
        let mut recleared_carbon = 0.;
        for cohort in self
            .cohorts
            .iter_mut()
            .filter(|c| !c.protected && c.age < RECLEARING_MAX_AGE)
        {
            let lost = cohort.area * RECLEARING_RATE;
            recleared_carbon += lost * cohort.stock_at(cohort.age);
            recleared_stock += lost * cohort.full_stock;
            recleared_area += lost;
            cohort.area -= lost;
        }
        if recleared_area > 0. {
            self.cohorts.push(Cohort {
                area: recleared_area,
                age: 0.,
                full_stock: recleared_stock / recleared_area,
                protected: false,
            });
        }

        // Regrowth uptake this year.
        let mut uptake = 0.;
        for cohort in self.cohorts.iter_mut() {
            let before = cohort.stored();
            let rate = if cohort.protected {
                PROTECTED_REGROWTH_SPEED
            } else {
                1.
            };
            cohort.age += speed * rate;
            uptake += cohort.stored() - before;
        }
        // Fully regrown land counts as wild again; the last bit of carbon
        // it would still take up is counted now.
        let mut matured_protected = 0.;
        self.cohorts.retain(|c| {
            if c.age >= MATURE_AGE {
                uptake += c.area * c.full_stock - c.stored();
                if c.protected {
                    matured_protected += c.area;
                }
                false
            } else {
                true
            }
        });
        out.regrowth = -uptake * CO2_PER_C;
        self.protected_wild += matured_protected;
        self.sort_cohorts();

        // Soil leakage from past clearing.
        let soil = self.soil_debt / SOIL_TIME_CONSTANT;
        self.soil_debt -= soil;

        // Background deforestation.
        let deforested = self.deforestation_area(protected_land) * DEFORESTATION_DENSITY;
        let soil_from_deforestation = deforested * (1. - IMMEDIATE_SHARE);
        self.soil_debt += soil_from_deforestation;
        out.soil = soil * CO2_PER_C;
        out.deforestation = (deforested * IMMEDIATE_SHARE + recleared_carbon) * CO2_PER_C;

        // Farm soils degrade or recover toward what farming sustains.
        if let Some(intensive) = inputs.intensive {
            let target = self.soil_health_target(intensive);
            let change = (target - self.soil_health) / SOIL_HEALTH_TIME_CONSTANT;
            self.soil_health = (self.soil_health + change).clamp(0., 1.);
            out.farm_soil = -change * used * SOIL_CARBON_PER_HEALTH * CO2_PER_C;
        }

        self.last = out;
        out
    }
}

impl Default for LandCarbon {
    fn default() -> Self {
        Self {
            prev_used: None,
            cohorts: vec![],
            soil_debt: 0.,
            deforestation_modifier: 0.,
            regrowth_modifier: 0.,
            last: LandEmissions::default(),
            response_years: None,
            max_clearing: None,
            protected_wild: 0.,
            protection_pending: 0.,
            soil_health: START_SOIL_HEALTH,
            soil_regeneration: 0.,
            total_freed: 0.,
        }
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
        let mut land = LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        };
        let first = land.step(START_USED, TOTAL, START_PROTECTION, 1.);
        let second = land.step(START_USED, TOTAL, START_PROTECTION, 1.);
        for year in [first, second] {
            assert!(
                (3.0..5.0).contains(&gt(year.total())),
                "{}",
                gt(year.total())
            );
            assert_eq!(year.clearing, 0.);
            assert_eq!(year.regrowth, 0.);
        }
    }

    #[test]
    fn clearing_is_fast_and_regrowth_slow() {
        let mut land = LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        };
        land.step(START_USED, TOTAL, START_PROTECTION, 1.);
        let baseline = land.step(START_USED, TOTAL, START_PROTECTION, 1.).total();

        // Clear 5 Mkm² in one year, then give it back the next.
        let cleared = land.step(START_USED + 5e12, TOTAL, START_PROTECTION, 1.);
        assert!(gt(cleared.clearing) > 10.);
        land.step(START_USED, TOTAL, START_PROTECTION, 1.);

        // Cumulative emissions relative to the baseline over the following
        // decades: the round trip is carbon neutral in the long run, but
        // regrowth takes far longer than clearing.
        let released = cleared.total() - baseline;
        let mut cumulative = released;
        let mut years_to_recover = None;
        for year in 1..300 {
            cumulative += land.step(START_USED, TOTAL, START_PROTECTION, 1.).total() - baseline;
            if cumulative <= released * 0.25 && years_to_recover.is_none() {
                years_to_recover = Some(year);
            }
        }
        let years = years_to_recover.expect("eventually recovers");
        assert!(years > 40, "recovered 75% in {years} years");
        assert!(
            cumulative.abs() < released * 0.05,
            "long run is carbon neutral"
        );
    }

    #[test]
    fn regrowing_land_cleared_again_releases_only_what_it_regrew() {
        let mut land = LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        };
        land.step(START_USED, TOTAL, START_PROTECTION, 1.);
        land.step(START_USED - 5e12, TOTAL, START_PROTECTION, 1.);
        for _ in 0..5 {
            land.step(START_USED - 5e12, TOTAL, START_PROTECTION, 1.);
        }
        let reclear = land.step(START_USED, TOTAL, START_PROTECTION, 1.);

        let mut fresh = LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        };
        fresh.step(START_USED, TOTAL, START_PROTECTION, 1.);
        let wild = fresh.step(START_USED + 5e12, TOTAL, START_PROTECTION, 1.);
        assert!(reclear.clearing < wild.clearing * 0.3);
        assert!(land.cohorts.is_empty());
    }

    #[test]
    fn clearing_estimate_matches_what_happens() {
        let mut land = LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        };
        land.step(START_USED, TOTAL, START_PROTECTION, 1.);
        let estimate = land.estimate_clearing(START_USED, START_USED + 3e12, TOTAL);
        let debt_before = land.soil_debt;
        let year = land.step(START_USED + 3e12, TOTAL, START_PROTECTION, 1.);
        // Immediate release plus the soil carbon that leaks out later.
        let deforestation_soil =
            year.deforestation / CO2_PER_C / IMMEDIATE_SHARE * (1. - IMMEDIATE_SHARE);
        let soil_added = land.soil_debt - debt_before - deforestation_soil + year.soil / CO2_PER_C;
        let actual = year.clearing + soil_added * CO2_PER_C;
        assert!(
            (estimate - actual).abs() / actual < 0.01,
            "{estimate} vs {actual}"
        );
    }

    #[test]
    fn short_shocks_are_mostly_absorbed() {
        // A one-year spike in land demand clears far less than a lasting one.
        let run = |years: usize| {
            let mut land = LandCarbon::default();
            land.step(START_USED, TOTAL, START_PROTECTION, 1.);
            let mut cleared = 0.;
            for year in 0..20 {
                let demand = if year < years {
                    START_USED + 5e12
                } else {
                    START_USED
                };
                cleared += land.step(demand, TOTAL, START_PROTECTION, 1.).clearing;
            }
            cleared
        };
        assert!(run(1) < run(20) * 0.3);
    }

    #[test]
    fn warming_slows_regrowth() {
        let regrown = |temperature: f32| {
            let mut land = LandCarbon {
                response_years: Some(1.),
                max_clearing: Some(f32::MAX),
                ..Default::default()
            };
            land.step(START_USED, TOTAL, START_PROTECTION, temperature);
            let mut total = 0.;
            for _ in 0..20 {
                total -= land
                    .step(START_USED - 5e12, TOTAL, START_PROTECTION, temperature)
                    .regrowth;
            }
            total
        };
        assert!(regrown(3.) < regrown(1.) * 0.6);
        assert!(regrown(2.) < regrown(1.));
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
        let mut land = LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        };
        land.step(START_USED, TOTAL, START_PROTECTION, 1.);
        let freed_used = START_USED - 2e13;
        let mut removed = 0.;
        for _ in 0..80 {
            removed -= land.step(freed_used, TOTAL, START_PROTECTION, 1.).regrowth;
        }
        let removed = gt(removed);
        assert!((200.0..700.0).contains(&removed), "{removed} Gt");
    }

    #[test]
    fn big_jumps_in_land_demand_are_cleared_over_years() {
        let total = 1e14;
        let mut land = LandCarbon::default();
        land.step(0.4 * total, total, 0.1, 1.);
        let first = land.step(0.5 * total, total, 0.1, 1.);
        // At most the clearing cap is newly cleared in a year...
        assert!(land.prev_used.unwrap() - 0.4 * total <= MAX_CLEARING_RATE * 1.001);
        assert!(gt(first.clearing) < 15.);
        // ...but it keeps going until demand is met.
        for _ in 0..60 {
            land.step(0.5 * total, total, 0.1, 1.);
        }
        assert!((land.prev_used.unwrap() - 0.5 * total).abs() < 0.01 * total);
    }

    fn inputs(demand: f32, protected: f32, restored: f32) -> LandInputs {
        LandInputs {
            demand,
            total: TOTAL,
            protected,
            restored,
            temperature: 1.,
            intensive: None,
        }
    }

    fn fast() -> LandCarbon {
        LandCarbon {
            response_years: Some(1.),
            max_clearing: Some(f32::MAX),
            ..Default::default()
        }
    }

    #[test]
    fn pools_add_up_to_all_land() {
        let mut land = fast();
        land.step_with(inputs(START_USED, 0.1, 0.));
        land.step_with(inputs(START_USED - 5e12, 0.3, 0.1));
        for _ in 0..10 {
            land.step_with(inputs(START_USED - 5e12, 0.3, 0.1));
        }
        let p = land.pools(TOTAL);
        let sum = p.farmed + p.wild + p.regrowing + p.wild_protected + p.regrowing_protected;
        assert!((sum - TOTAL).abs() / TOTAL < 1e-3, "{p:?}");
        assert!((p.protected() - 0.3 * TOTAL).abs() / TOTAL < 1e-3, "{p:?}");
    }

    #[test]
    fn restoration_protects_regrowing_land_and_preserves_protect_wild_land() {
        let freed = 5e12;
        let mut restore = fast();
        restore.step_with(inputs(START_USED, 0.1, 0.));
        restore.step_with(inputs(
            START_USED - freed,
            0.1 + freed / TOTAL,
            freed / TOTAL,
        ));
        let p = restore.pools(TOTAL);
        assert!((p.regrowing_protected - freed).abs() < 1e9, "{p:?}");
        assert!((p.wild_protected - 0.1 * TOTAL).abs() / TOTAL < 1e-3);

        let mut preserve = fast();
        preserve.step_with(inputs(START_USED, 0.1, 0.));
        preserve.step_with(inputs(START_USED - freed, 0.1 + freed / TOTAL, 0.));
        let p = preserve.pools(TOTAL);
        assert_eq!(p.regrowing_protected, 0.);
        assert!((p.wild_protected - 0.1 * TOTAL - freed).abs() / TOTAL < 1e-3);
    }

    #[test]
    fn protected_regrowth_is_faster_and_never_cleared_again() {
        let run = |restored: f32| {
            let mut land = fast();
            land.step_with(inputs(START_USED, 0.1, 0.));
            for _ in 0..40 {
                land.step_with(inputs(START_USED - 5e12, 0.1 + restored, restored));
            }
            let stored: f32 = land.cohorts.iter().map(|c| c.stored()).sum();
            // Then farmland demand comes back: only unprotected land is cleared.
            land.step_with(inputs(START_USED, 0.1 + restored, restored));
            (stored, land.pools(TOTAL).regrowing_protected)
        };
        let (unmanaged, _) = run(0.);
        let (protected, still_regrowing) = run(5e12 / TOTAL);
        assert!(protected > unmanaged * 1.2, "{protected} vs {unmanaged}");
        assert!((still_regrowing - 5e12).abs() < 1e10);
    }

    #[test]
    fn protection_beyond_unfarmed_land_waits_for_land_to_be_freed() {
        let mut land = fast();
        land.step_with(inputs(START_USED, 0.1, 0.));
        // Ask to protect all unfarmed land and 5 Mkm² more.
        let share = (TOTAL - START_USED + 5e12) / TOTAL;
        land.step_with(inputs(START_USED, share, 5e12 / TOTAL));
        assert!((land.protection_pending - 5e12).abs() / 5e12 < 1e-2);
        land.step_with(inputs(START_USED - 5e12, share, 5e12 / TOTAL));
        assert!(land.protection_pending < 1e9);
        assert!((land.pools(TOTAL).regrowing_protected - 5e12).abs() < 1e10);
    }

    #[test]
    fn intensive_farming_wears_soils_down_and_regeneration_restores_them() {
        let run = |intensive: f32, regeneration: f32| {
            let mut land = LandCarbon {
                soil_regeneration: regeneration,
                ..fast()
            };
            let mut soil = 0.;
            for _ in 0..30 {
                soil += land
                    .step_with(LandInputs {
                        intensive: Some(intensive),
                        ..inputs(START_USED, 0.1, 0.)
                    })
                    .farm_soil;
            }
            (land.soil_health, gt(soil))
        };
        let (degraded, released) = run(1., 0.);
        let (restored, taken_up) = run(1., 1.);
        let (organic, _) = run(0., 0.);
        assert!(degraded < START_SOIL_HEALTH && released > 0.);
        assert!(restored > START_SOIL_HEALTH && taken_up < 0.);
        assert!((restored - organic).abs() < 1e-4);
        // About 1.5 Gt a year taken up at first when restoring all farm soils.
        assert!((15.0..40.0).contains(&-taken_up), "{taken_up}");
    }
}
