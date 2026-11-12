//! Materials for building energy infrastructure.
//!
//! Building power plants takes steel, cement and manufacturing energy
//! (silicon, glass, aluminum, electronics). Rather than folding that into
//! each process's per-kWh emissions as a fixed lifecycle number, we work
//! out how much capacity is built each year and add the materials to the
//! existing Iron and Steel, Concrete and Other Industry sectors. Their
//! emissions then follow the economy: build fast with dirty factories and
//! emissions spike; decarbonize industry (Electric-Arc Furnaces,
//! Carbon-Negative Concrete, clean electricity) and building gets cleaner.
//!
//! Fuel plants (refineries, wells, electrolyzers) are counted per MW of fuel
//! output, and food plants (cellular meat, vertical farms) per unit of
//! yearly output.
//!
//! Only capacity beyond what existed at the start of the game is counted:
//! the materials for maintaining the starting fleet are already part of
//! the industries' baseline demand.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Collection, Id, industries::Industry, production::Process};

/// What it takes to build one megawatt of a process's capacity.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ProcessMaterials {
    /// Average output as a fraction of nameplate capacity.
    pub capacity_factor: f32,
    /// Years before a plant needs replacing.
    pub lifetime: f32,
    /// Tonnes of steel per MW.
    pub steel: f32,
    /// Tonnes of cement per MW (concrete is roughly 1/8 cement).
    pub cement: f32,
    /// Other manufacturing energy per MW, in kWh
    /// (panels, blades, electronics, glass, aluminum).
    pub other_energy: f32,
    /// For processes not measured in kWh (food, in kcal): output per year
    /// of one unit of capacity, which `steel`, `cement` and `other_energy`
    /// are then per. `capacity_factor` is ignored.
    #[serde(default)]
    pub annual_output: Option<f32>,
}

impl ProcessMaterials {
    /// Output per MW (or unit) of capacity per year, in the process's
    /// output units.
    pub fn output_per_mw(&self) -> f32 {
        self.annual_output.unwrap_or(self.capacity_factor * 8760. * 1000.)
    }
}

/// New capacity is built over this many years (a planning cycle), so a
/// big change to the production mix doesn't need all its materials at once.
pub const CONSTRUCTION_YEARS: usize = 5;

/// Global production at the start of the game, used to convert tonnes
/// into each industry's demand units.
const STARTING_STEEL_TONNES: f32 = 1.9e9;
const STARTING_CEMENT_TONNES: f32 = 4.1e9;

pub const STEEL_INDUSTRY: &str = "Iron and Steel";
pub const CEMENT_INDUSTRY: &str = "Concrete";
pub const OTHER_INDUSTRY: &str = "Other Industry";

/// Materials used for building energy infrastructure this year.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BuildOut {
    /// Capacity built this year (MW), new and replacement.
    pub capacity: f32,
    /// Tonnes.
    pub steel: f32,
    /// Tonnes.
    pub cement: f32,
    /// kWh.
    pub other_energy: f32,
}

impl BuildOut {
    /// Materials for building enough of `process` to make `output` more a
    /// year (in its output units), and for replacing that capacity as it
    /// wears out (per year, once built).
    pub fn for_output(process: &Process, output: f32) -> Option<(BuildOut, BuildOut)> {
        let materials = process.materials?;
        let per_mw = materials.output_per_mw() * (1. + process.output_modifier).max(0.01);
        if per_mw <= 0. || output <= 0. {
            return None;
        }
        let capacity = output / per_mw;
        let mut build = BuildOut::default();
        build.add(capacity, &materials);
        let mut upkeep = BuildOut::default();
        if materials.lifetime > 0. {
            upkeep.add(capacity / materials.lifetime, &materials);
        }
        Some((build, upkeep))
    }

    pub fn combine_with(&mut self, other: &BuildOut) {
        self.combine(other, 1.);
    }

    fn add(&mut self, capacity: f32, materials: &ProcessMaterials) {
        self.capacity += capacity;
        self.steel += capacity * materials.steel;
        self.cement += capacity * materials.cement;
        self.other_energy += capacity * materials.other_energy;
    }

    fn combine(&mut self, other: &BuildOut, share: f32) {
        self.capacity += other.capacity * share;
        self.steel += other.steel * share;
        self.cement += other.cement * share;
        self.other_energy += other.other_energy * share;
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Infrastructure {
    /// Output of each process at the start of the game.
    pub starting_output: BTreeMap<Id, f32>,
    /// Output of each process last year.
    pub prev_output: BTreeMap<Id, f32>,
    /// Industry demand units per tonne (steel, cement), fixed at the start.
    pub units_per_tonne: Option<(f32, f32)>,
    /// New construction still under way, one entry per coming year.
    #[serde(default)]
    pub pipeline: Vec<BuildOut>,
    /// The most recent year's build-out.
    pub last: BuildOut,
    /// Materials projects are using this year.
    #[serde(default)]
    pub project_build: BuildOut,
}

impl Infrastructure {
    /// Materials a project uses this year.
    pub fn add_project_build(&mut self, build: &BuildOut, share: f32) {
        self.project_build.combine(build, share);
    }

    /// Materials for something built over the coming years.
    pub fn queue(&mut self, build: &BuildOut) {
        self.pipeline.resize(CONSTRUCTION_YEARS, BuildOut::default());
        for year in self.pipeline.iter_mut() {
            year.combine(build, 1. / CONSTRUCTION_YEARS as f32);
        }
    }

    /// Work out this year's build-out from production, and set the extra
    /// demand on the industries that supply it.
    pub fn step(
        &mut self,
        processes: &Collection<Process>,
        produced: &BTreeMap<Id, f32>,
        industries: &mut Collection<Industry>,
        lic_pop: f32,
    ) -> BuildOut {
        let first_year = self.starting_output.is_empty();
        if first_year {
            self.starting_output = produced.clone();
            let base = |name: &str| {
                industries
                    .iter()
                    .find(|i| i.name == name)
                    .map(|i| i.base_demand(lic_pop))
                    .unwrap_or(0.)
            };
            self.units_per_tonne = Some((
                base(STEEL_INDUSTRY) / STARTING_STEEL_TONNES,
                base(CEMENT_INDUSTRY) / STARTING_CEMENT_TONNES,
            ));
        }

        let mut out = BuildOut::default();
        let mut started = BuildOut::default();
        for process in processes.iter() {
            let Some(materials) = process.materials else {
                continue;
            };
            // Output modifiers (e.g. more efficient solar cells) get more
            // out of each plant, so less has to be built per kWh.
            let per_mw = materials.output_per_mw() * (1. + process.output_modifier).max(0.01);
            if per_mw <= 0. {
                continue;
            }
            let output = produced.get(&process.id).copied().unwrap_or(0.);
            let start = self.starting_output.get(&process.id).copied().unwrap_or(0.);
            let prev = self.prev_output.get(&process.id).copied().unwrap_or(start);

            let new_capacity = (output - prev).max(0.) / per_mw;
            let replaced = if materials.lifetime > 0. {
                (output - start).max(0.) / per_mw / materials.lifetime
            } else {
                0.
            };
            started.add(new_capacity, &materials);
            out.add(replaced, &materials);
        }
        self.prev_output = produced.clone();

        // Spread new construction over the coming years.
        self.pipeline.resize(CONSTRUCTION_YEARS, BuildOut::default());
        for year in self.pipeline.iter_mut() {
            year.combine(&started, 1. / CONSTRUCTION_YEARS as f32);
        }
        let this_year = self.pipeline.remove(0);
        out.combine(&this_year, 1.);
        out.combine(&self.project_build, 1.);
        self.project_build = BuildOut::default();

        let (steel_units, cement_units) = self.units_per_tonne.unwrap_or_default();
        for industry in industries.iter_mut() {
            industry.extra_demand = match industry.name.as_str() {
                STEEL_INDUSTRY => out.steel * steel_units,
                CEMENT_INDUSTRY => out.cement * cement_units,
                OTHER_INDUSTRY => {
                    let energy_per_unit = {
                        let r = industry.adj_resources();
                        r.fuel + r.electricity
                    };
                    if energy_per_unit > 0. {
                        out.other_energy / energy_per_unit
                    } else {
                        0.
                    }
                }
                _ => industry.extra_demand,
            };
        }

        self.last = out;
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::{State, World};

    fn solar_id(state: &State) -> crate::Id {
        state.world.processes.iter().find(|p| p.name == "Solar PV").unwrap().id
    }

    #[test]
    fn building_solar_adds_steel_demand() {
        let mut state = State::new(World::default());
        state.step_year(state.world.temperature);
        let steel = |s: &State| {
            let lic_pop = s.world.lic_population();
            s.world.industries.iter().find(|i| i.name == super::STEEL_INDUSTRY).unwrap().demand(lic_pop)
        };
        let before = steel(&state);

        // Triple solar's share of electricity in one go.
        let solar = solar_id(&state);
        state.change_process_mix_share(&solar, 4);
        state.step_year(state.world.temperature);
        let built = state.infrastructure.last;
        assert!(built.capacity > 1e5, "built {} MW", built.capacity);
        assert!(built.steel > 0.);
        state.step_year(state.world.temperature);
        assert!(steel(&state) > before);
    }

    #[test]
    fn nothing_built_without_growth() {
        let mut state = State::new(World::default());
        for _ in 0..3 {
            state.step_year(state.world.temperature);
        }
        // Demand growth builds a few hundred GW a year, nothing like a
        // deliberate build-out.
        assert!(state.infrastructure.last.capacity < 1e6);
    }

    #[test]
    fn building_food_and_fuel_plants_takes_steel() {
        for name in ["Cellular Meat", "Green Hydrogen"] {
            let mut state = State::new(World::default());
            let id = state.world.processes.iter().find(|p| p.name == name).unwrap().id;
            state.world.processes[&id].locked = false;
            state.step_year(state.world.temperature);
            state.change_process_mix_share(&id, 4);
            state.step_year(state.world.temperature);
            assert!(state.infrastructure.last.steel > 1e6, "{name}: {}", state.infrastructure.last.steel);
        }
    }

    #[test]
    fn building_projects_use_materials() {
        let mut state = State::new(World::default());
        state.step_year(state.world.temperature);
        let id = state.world.projects.iter().find(|p| p.name == "Expand Public Transit").unwrap().id;
        state.start_project(&id);
        state.set_project_points(&id, 5);
        state.step_year(state.world.temperature);
        let built = state.infrastructure.last.cement;
        assert!(built > 1e7, "cement {built}");
    }
}
