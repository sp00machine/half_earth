use enum_map::EnumMap;
use hes_engine::{
    ByproductMap, Effect, Industry, Output, Process, Resource, ResourceMap, State, WorldVariable,
    mean_demand_outlook_change, mean_income_outlook_change,
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{
    display::{self, FloatExt, icons::HasIcon, intensity},
    vars::*,
    views::FactorsCard,
};
use rust_i18n::t;

fn effects_factor<E: AsRef<Effect>>(var: Var, effects: &[E], state: &State) -> f32 {
    let effects = effects.iter().map(|e| e.as_ref());
    match var {
        Var::Emissions => effects
            .filter_map(|e| match e {
                Effect::WorldVariable(WorldVariable::Emissions, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::Water => effects
            .filter_map(|e| match e {
                Effect::Resource(Resource::Water, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::Land => effects
            .filter_map(|e| match e {
                Effect::ProtectLand(val) => Some(val * state.world.starting_resources.land),
                _ => None,
            })
            .sum::<f32>(),
        Var::Energy => effects
            .filter_map(|e| match e {
                Effect::DemandAmount(Output::Electricity | Output::Fuel, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::Electricity => effects
            .filter_map(|e| match e {
                Effect::DemandAmount(Output::Electricity, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::Fuel => effects
            .filter_map(|e| match e {
                Effect::DemandAmount(Output::Fuel, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::PlantCalories => effects
            .filter_map(|e| match e {
                Effect::DemandAmount(Output::PlantCalories, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::AnimalCalories => effects
            .filter_map(|e| match e {
                Effect::DemandAmount(Output::AnimalCalories, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
        Var::Contentedness => effects
            .filter_map(|e| match e {
                Effect::WorldVariable(WorldVariable::Outlook, val) => Some(*val),
                Effect::IncomeOutlookChange(val) => {
                    let change = mean_income_outlook_change(*val, state);
                    Some(change)
                }
                Effect::OutlookByIncome(amounts) => {
                    Some(hes_engine::mean_outlook_by_income(amounts, state))
                }
                Effect::DemandOutlookChange(output, val) => {
                    let change = mean_demand_outlook_change(*val, output, state);
                    Some(change)
                }
                _ => None,
            })
            .sum::<f32>(),
        Var::Biodiversity => effects
            .filter_map(|e| match e {
                Effect::WorldVariable(WorldVariable::ExtinctionRate, val) => Some(val),
                _ => None,
            })
            .sum::<f32>(),
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum Factor {
    Project {
        name: String,
        amount: f32,
        display: String,
    },
    Region {
        name: String,
        amount: f32,
        display: String,
        intensity: usize,
    },
    Process {
        name: String,
        amount: f32,
        display: String,
        intensity: usize,
        output: Output,
        produced: f32,
        display_produced: String,
        #[serde(default)]
        gases: Option<Gases>,
    },
    Industry {
        name: String,
        amount: f32,
        produced: f32,
        intensity: usize,
        display: String,
        #[serde(default)]
        gases: Option<Gases>,
    },
    Event {
        name: String,
        amount: f32,
        display: Option<String>,
    },
}
impl Factor {
    pub fn name(&self) -> &str {
        match self {
            Self::Project { name, .. } => name,
            Self::Region { name, .. } => name,
            Self::Process { name, .. } => name,
            Self::Industry { name, .. } => name,
            Self::Event { name, .. } => name,
        }
    }

    pub fn amount(&self) -> f32 {
        match self {
            Self::Project { amount, .. } => *amount,
            Self::Region { amount, .. } => *amount,
            Self::Process { amount, .. } => *amount,
            Self::Industry { amount, .. } => *amount,
            Self::Event { amount, .. } => *amount,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Project { display, .. } => display.clone(),
            Self::Region { display, .. } => display.clone(),
            Self::Process { display, .. } => display.clone(),
            Self::Industry { display, .. } => display.clone(),
            Self::Event {
                display, amount, ..
            } => display.clone().unwrap_or(amount.to_string()),
        }
    }
}

fn event_factors(var: Var, state: &State) -> Vec<Factor> {
    state
        .events
        .iter()
        .map(|event| Factor::Event {
            name: event.name.clone(),
            amount: effects_factor(var, &event.effects, state),
            display: None,
        })
        .filter(|fac| fac.amount() != 0.)
        .collect()
}

fn project_factors(var: Var, state: &State) -> Vec<Factor> {
    state
        .world
        .projects
        .online()
        .map(|p| {
            let effects = p.active_effects_with_outcomes();
            let amount = effects_factor(var, &effects, state);

            let display = if var.is_demand_var() {
                let amount = match var {
                    Var::Energy | Var::Electricity | Var::Fuel => {
                        display::round_energy(display::to_energy_units(amount))
                    }
                    Var::PlantCalories | Var::AnimalCalories => {
                        display::to_calorie_units(amount).round_to(1)
                    }
                    _ => unreachable!(),
                };
                amount.to_string()

            // Land amounts are expressed in m2
            // but should be displayed as % of available land.
            } else if var == Var::Land {
                format!(
                    "{}%",
                    display::percent_fine(amount / state.world.starting_resources.land)
                )
            } else if var == Var::Emissions {
                // Same units as processes and industries.
                display::gigatonnes(amount)
            } else {
                amount.round_to(1).to_string()
            };
            Factor::Project {
                name: p.name.clone(),
                amount,
                display,
            }
        })
        .filter(|fac| fac.amount() != 0.)
        .collect()
}

fn regional_factors(output: Output, state: &State) -> Vec<Factor> {
    let per_capita_output_demand = &state.world.per_capita_demand;
    state
        .world
        .regions
        .iter()
        .map(|region| {
            let intensity = region.income.level() + 1;
            let amount = region.demand(per_capita_output_demand)[output];
            let amount = display::output(amount, output);
            Factor::Region {
                name: region.name.clone(),
                intensity,
                display: amount.to_string(),
                amount,
            }
        })
        .filter(|fac| fac.amount() != 0.)
        .collect()
}

#[derive(Debug)]
struct Impacts {
    per_unit: f32,
    demand: f32,
    total: f32,
    display: String,
}

/// Emissions split by gas, each in CO2-equivalent (Gt).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Gases {
    pub co2: f32,
    pub ch4: f32,
    pub n2o: f32,
    pub fgas: f32,
}
impl Gases {
    /// From byproducts in grams.
    pub fn from_byproducts(b: &ByproductMap, amount: f32) -> Self {
        Gases {
            co2: b.co2 * amount * 1e-15,
            ch4: b.ch4 * 36. * amount * 1e-15,
            n2o: b.n2o * 298. * amount * 1e-15,
            fgas: b.fgas * amount * 1e-15,
        }
    }

    pub fn total(&self) -> f32 {
        self.co2 + self.ch4 + self.n2o + self.fgas
    }
}

/// How land is used, as fractions of all land.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LandSplit {
    pub farmed: f32,
    pub regrowing: f32,
    pub protected: f32,
    pub wild: f32,
}

/// Energy used by industry to build energy infrastructure
/// (the steel, cement and manufacturing for new capacity).
fn construction_factor(state: &State, resource: Resource) -> Option<Factor> {
    let amount: f32 = state
        .world
        .industries
        .iter()
        .map(|ind| ind.extra_demand.max(0.) * ind.adj_resources()[resource])
        .sum();
    (amount > 0.).then(|| Factor::Event {
        name: t!("Energy Infrastructure Construction").to_string(),
        amount,
        display: Some(display::round_energy(display::to_energy_units(amount)).to_string()),
    })
}

fn emission_gases<S: HasImpacts>(source: &S, impact: Impact, state: &State) -> Option<Gases> {
    (impact == Impact::Emissions)
        .then(|| Gases::from_byproducts(&source.byproducts(), source.demand(state)))
}

trait HasImpacts {
    fn resources(&self) -> ResourceMap;
    fn byproducts(&self) -> ByproductMap;
    fn extinction_rate(&self, total_land: f32) -> f32;
    fn demand(&self, state: &State) -> f32;
}
impl HasImpacts for Process {
    fn resources(&self) -> ResourceMap {
        self.adj_resources()
    }

    fn byproducts(&self) -> ByproductMap {
        self.adj_byproducts()
    }

    fn extinction_rate(&self, total_land: f32) -> f32 {
        self.extinction_rate(total_land)
    }

    fn demand(&self, state: &State) -> f32 {
        *state.produced.by_process.get(&self.id).unwrap_or(&0.)
    }
}

impl HasImpacts for Industry {
    fn resources(&self) -> ResourceMap {
        self.adj_resources()
    }

    fn byproducts(&self) -> ByproductMap {
        self.adj_byproducts()
    }

    fn extinction_rate(&self, total_land: f32) -> f32 {
        self.extinction_rate(total_land)
    }

    fn demand(&self, state: &State) -> f32 {
        let lic_pop = state.world.lic_population();
        self.demand(lic_pop)
    }
}

fn impact_factor<S: HasImpacts>(source: &S, impact: Impact, state: &State) -> Impacts {
    let base = match impact {
        Impact::Land | Impact::Water | Impact::Electricity | Impact::Fuel => {
            let res = impact.as_resource().expect("Checked that it's a resource.");
            source.resources()[res]
        }
        Impact::Energy => source.resources().energy(),
        Impact::Emissions => source.byproducts().co2eq(),
        Impact::Biodiversity => {
            let total_land = state.world.starting_resources.land;
            source.extinction_rate(total_land)
        }
    };

    let demand = source.demand(state);
    let total_impact = base * demand;

    let display = match impact {
        Impact::Energy => display::round_energy(display::to_energy_units(total_impact)).to_string(),
        Impact::Electricity => display::output(total_impact, Output::Electricity).to_string(),
        Impact::Fuel => display::output(total_impact, Output::Fuel).to_string(),
        Impact::Land => {
            display::format_impact(impact, total_impact, state.world.starting_resources)
        }
        _ => display::format_impact(impact, total_impact, state.resources.available),
    };

    Impacts {
        per_unit: base,
        demand,
        total: total_impact,
        display,
    }
}

fn production_factors(impact: Impact, state: &State) -> Vec<Factor> {
    state
        .world
        .processes
        .iter()
        .map(|proc| {
            let Impacts {
                per_unit,
                demand,
                total,
                display: display_amount,
            } = impact_factor(proc, impact, state);

            let kind: OutputKind = proc.output.into();
            let inten = intensity::impact_intensity(per_unit, impact, kind);
            let total_demand = state.output_demand.of(proc.output);
            let display_produced = format!("{}%", display::percent(demand / total_demand, true));
            Factor::Process {
                name: proc.name.to_string(),
                produced: demand,
                output: proc.output,
                amount: total,
                display: display_amount,
                intensity: inten,
                display_produced,
                gases: emission_gases(proc, impact, state),
            }
        })
        .chain(state.world.industries.iter().map(|ind| {
            let Impacts {
                per_unit,
                demand,
                total,
                display,
            } = impact_factor(ind, impact, state);
            let inten = intensity::impact_intensity(per_unit, impact, OutputKind::Energy);
            Factor::Industry {
                name: ind.name.to_string(),
                produced: demand,
                display,
                intensity: inten,
                amount: total,
                gases: emission_gases(ind, impact, state),
            }
        }))
        .filter(|fac| fac.amount() != 0.)
        .collect()
}

pub fn rank(state: &State) -> EnumMap<Var, Vec<Factor>> {
    let mut factors = EnumMap::default();
    for var in Var::iter() {
        let mut rankings = project_factors(var, state);
        rankings.extend(event_factors(var, state));
        if let Some(impact) = var.as_impact() {
            rankings.extend(production_factors(impact, state));
        }
        if let Some(output) = var.as_output() {
            rankings.extend(regional_factors(output, state));
        }

        // Additional factors
        match var {
            Var::Contentedness => {
                if state.world.temp_outlook.round_to(1) != 0. {
                    rankings.push(Factor::Event {
                        name: t!("Temperature Change").to_string(),
                        amount: state.world.temp_outlook.round_to(1),
                        display: None,
                    })
                }
                if state.shortages_outlook != 0. {
                    rankings.push(Factor::Event {
                        name: t!("Production Shortages").to_string(),
                        amount: -state.shortages_outlook.round_to(1),
                        display: None,
                    })
                }
                rankings.push(Factor::Event {
                    name: t!("Post-Revolution Optimism").to_string(),
                    amount: 30.,
                    display: None,
                });
                // Policy fatigue is applied to regions' outlook, so
                // pull it out of the regional factors to show it separately.
                let n_regions = state.world.regions.len().max(1) as f32;
                let fatigue = state.fatigue_applied.values().sum::<f32>() / n_regions;
                if fatigue.round_to(1) != 0. {
                    rankings.push(Factor::Event {
                        name: t!("Policy Fatigue").to_string(),
                        amount: -fatigue.round_to(1),
                        display: None,
                    })
                }
                // Delta relative to their starting value of 10.
                let regions_outlook_delta =
                    (state.world.regions.outlook() + fatigue - 10.).round_to(1);
                if regions_outlook_delta != 0. {
                    rankings.push(Factor::Event {
                        name: t!("Regional Factors").to_string(),
                        amount: regions_outlook_delta,
                        display: None,
                    })
                }
            }
            Var::Fuel => {
                if let Some(factor) = construction_factor(state, Resource::Fuel) {
                    rankings.push(factor);
                }
            }
            Var::Electricity => {
                if let Some(factor) = construction_factor(state, Resource::Electricity) {
                    rankings.push(factor);
                }
                let storage = state.storage.last;
                for (name, amount) in [
                    (t!("Storage Losses"), storage.losses),
                    (t!("Undelivered: Not Enough Storage"), storage.curtailed),
                ] {
                    if amount > 0. {
                        rankings.push(Factor::Event {
                            name: name.to_string(),
                            amount,
                            display: Some(
                                display::round_energy(display::to_energy_units(amount)).to_string(),
                            ),
                        });
                    }
                }
            }
            Var::Emissions => {
                // Land-use change, broken out by source.
                let land = state.land_carbon.last;
                for (name, amount) in [
                    (t!("Land Clearing"), land.clearing),
                    (t!("Soil Carbon from Past Clearing"), land.soil),
                    (t!("Background Deforestation"), land.deforestation),
                    (t!("Regrowing Land"), land.regrowth),
                ] {
                    let amount = amount * 1e-15;
                    if amount.abs() >= 0.05 {
                        rankings.push(Factor::Event {
                            name: name.to_string(),
                            amount,
                            display: Some(format!("{amount:+.1}Gt")),
                        });
                    }
                }
            }
            Var::Land => {
                // Note that for factors we compare against
                // *starting* land and not *available* land,
                // as available land is essentially starting land
                // minus what's protected.
                rankings.push(Factor::Event {
                    name: t!("Nature Preserves").to_string(),
                    display: Some(format!("{:.0}%", state.protected_land * 100.)),
                    amount: state.protected_land * state.world.starting_resources.land,
                });
            }
            Var::Biodiversity => {
                rankings.push(Factor::Event {
                    name: "Sea Level Rise".into(),
                    amount: state.world.slr_extinction_rate().round(),
                    display: None,
                });
                rankings.push(Factor::Event {
                    name: "Temperature Change".into(),
                    amount: state.world.tgav_extinction_rate().round(),
                    display: None,
                });
            }
            _ => {}
        }

        // NOTE TODO I do not remember what this is about
        // Split into modifiers (which don't make up demand percentages)
        // and contributors (who do make up demand percentages);
        // let [modifiers, contribs] = partition(rankings, (r) => {
        //     let amount = r.displayAmount || r.amount;
        //     return typeof amount === 'string' && (amount.startsWith('+') || amount.startsWith('-'));
        // });
        // modifiers.sort();
        // contribs.sort((a, b) => Math.abs(a.amount) > Math.abs(b.amount) ? -1 : 1)
        // factors[k] = modifiers.concat(contribs);

        // HACK: Filter out non-finite (infinite or NaN) values.
        // FIXME: All of the factor calculations *should* be finite.
        rankings.retain(|r| {
            let amount = r.amount();
            if !amount.is_finite() {
                tracing::warn!("Non-finite ranking factor: {amount} for {}", r.name());
                false
            } else {
                true
            }
        });

        // Since we filtered out non-finite values
        // we should be ok to sort this way.
        rankings.sort_by(|a, b| {
            b.amount()
                .abs()
                .partial_cmp(&a.amount().abs())
                .unwrap_or_else(|| {
                    let a_invalid = a.amount().is_finite();
                    let b_invalid = b.amount().is_finite();
                    if a_invalid && b_invalid {
                        std::cmp::Ordering::Equal
                    } else if a_invalid {
                        std::cmp::Ordering::Greater
                    } else if b_invalid {
                        std::cmp::Ordering::Less
                    } else {
                        unreachable!("At least one of the values will be non-finite.");
                    }
                })
        });
        factors[var] = rankings;
    }
    factors
}

pub fn factors_card(current_name: Option<String>, var: Var, state: &State) -> FactorsCard {
    FactorsCard {
        icon: var.icon(),
        kind: var,
        current: current_name,
        land: (var == Var::Land).then(|| {
            let total = state.world.starting_resources.land;
            let farmed = (state.resource_demand.of(Resource::Land) / total).clamp(0., 1.);
            let protected = state.protected_land.clamp(0., 1. - farmed);
            let regrowing =
                (state.land_carbon.regrowing_area() / total).clamp(0., 1. - farmed - protected);
            LandSplit {
                farmed,
                regrowing,
                protected,
                wild: (1. - farmed - regrowing - protected).max(0.),
            }
        }),
        gases: (var == Var::Emissions).then(|| Gases {
            co2: (state.emissions.co2 + state.emissions.land) * 1e-15,
            ch4: state.emissions.ch4 * 36. * 1e-15,
            n2o: state.emissions.n2o * 298. * 1e-15,
            fgas: state.emissions.fgas * 1e-15,
        }),
        total: match var {
            Var::Emissions => state.emissions.as_gtco2eq(),
            Var::Biodiversity => state.world.extinction_rate.round().max(0.),
            Var::Land => display::resource(
                state.resource_demand.of(Resource::Land)
                    + (state.protected_land * state.world.starting_resources.land),
                Resource::Land,
                state.world.starting_resources,
            ),
            Var::Energy => {
                let demand = state.output_demand.total().energy();
                display::to_energy_units(demand)
            }
            Var::Water => display::resource(
                state.resource_demand.of(Resource::Water),
                Resource::Water,
                state.resources.available,
            ),
            Var::Contentedness => state.outlook().round_to(1),
            Var::Electricity => {
                display::to_energy_units(state.output_demand.of(Output::Electricity))
            }
            Var::Fuel => display::to_energy_units(state.output_demand.of(Output::Fuel)),
            Var::PlantCalories => {
                display::to_calorie_units(state.output_demand.of(Output::PlantCalories))
            }
            Var::AnimalCalories => {
                display::to_calorie_units(state.output_demand.of(Output::AnimalCalories))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_factors_emissions() {
        let state = State::default();
        let effects = vec![
            Effect::WorldVariable(WorldVariable::Emissions, 12.),
            Effect::WorldVariable(WorldVariable::Emissions, 16.),
            Effect::WorldVariable(WorldVariable::Emissions, -2.),
        ];
        let contrib = effects_factor(Var::Emissions, &effects, &state);
        assert_eq!(contrib, 26.);
    }

    #[test]
    fn test_effect_factors_water() {
        let state = State::default();
        let effects = vec![
            Effect::Resource(Resource::Water, 8.),
            Effect::Resource(Resource::Water, 12.),
            Effect::Resource(Resource::Water, -2.),
        ];
        let contrib = effects_factor(Var::Water, &effects, &state);
        assert_eq!(contrib, 18.);
    }

    #[test]
    fn test_effect_factors_land() {
        let state = State::default();
        let effects = vec![
            Effect::ProtectLand(0.2),
            Effect::ProtectLand(-0.3),
            Effect::ProtectLand(0.5),
        ];
        let contrib = effects_factor(Var::Land, &effects, &state);
        assert_eq!(contrib, 41599996000000.0);
    }

    #[test]
    fn test_effect_factors_energy() {
        let state = State::default();
        let effects = vec![
            Effect::DemandAmount(Output::Electricity, 15.),
            Effect::DemandAmount(Output::Fuel, 12.),
            Effect::DemandAmount(Output::Fuel, 0.5),
            Effect::DemandAmount(Output::Electricity, -2.),
        ];
        let contrib = effects_factor(Var::Energy, &effects, &state);
        assert_eq!(contrib, 25.5);
    }

    #[test]
    fn test_effect_factors_electricity() {
        let state = State::default();
        let effects = vec![
            Effect::DemandAmount(Output::Electricity, 15.),
            Effect::DemandAmount(Output::Fuel, 12.),
            Effect::DemandAmount(Output::Fuel, 0.5),
            Effect::DemandAmount(Output::Electricity, -2.),
        ];
        let contrib = effects_factor(Var::Electricity, &effects, &state);
        assert_eq!(contrib, 13.);
    }

    #[test]
    fn test_effect_factors_fuel() {
        let state = State::default();
        let effects = vec![
            Effect::DemandAmount(Output::Electricity, 15.),
            Effect::DemandAmount(Output::Fuel, 12.),
            Effect::DemandAmount(Output::Fuel, 0.5),
            Effect::DemandAmount(Output::Electricity, -2.),
        ];
        let contrib = effects_factor(Var::Fuel, &effects, &state);
        assert_eq!(contrib, 12.5);
    }

    #[test]
    fn test_effect_factors_plant_calories() {
        let state = State::default();
        let effects = vec![
            Effect::DemandAmount(Output::PlantCalories, 15.),
            Effect::DemandAmount(Output::AnimalCalories, 12.),
            Effect::DemandAmount(Output::AnimalCalories, 0.5),
            Effect::DemandAmount(Output::PlantCalories, -2.),
        ];
        let contrib = effects_factor(Var::PlantCalories, &effects, &state);
        assert_eq!(contrib, 13.);
    }

    #[test]
    fn test_effect_factors_animal_calories() {
        let state = State::default();
        let effects = vec![
            Effect::DemandAmount(Output::PlantCalories, 15.),
            Effect::DemandAmount(Output::AnimalCalories, 12.),
            Effect::DemandAmount(Output::AnimalCalories, 0.5),
            Effect::DemandAmount(Output::PlantCalories, -2.),
        ];
        let contrib = effects_factor(Var::AnimalCalories, &effects, &state);
        assert_eq!(contrib, 12.5);
    }

    #[test]
    fn test_effect_factors_contentedness() {
        let state = State::default();
        let effects = vec![
            Effect::WorldVariable(WorldVariable::Outlook, 22.),
            Effect::IncomeOutlookChange(3.), // 5.
            Effect::WorldVariable(WorldVariable::Outlook, -6.),
            Effect::DemandOutlookChange(Output::Fuel, 1.), // 3.
        ];
        let contrib = effects_factor(Var::Contentedness, &effects, &state);
        assert_eq!(contrib.round(), 24.);
    }

    #[test]
    fn test_effect_factors_biodiversity() {
        let state = State::default();
        let effects = vec![
            Effect::WorldVariable(WorldVariable::ExtinctionRate, 22.),
            Effect::WorldVariable(WorldVariable::ExtinctionRate, -6.),
        ];
        let contrib = effects_factor(Var::Biodiversity, &effects, &state);
        assert_eq!(contrib.round(), 16.);
    }

    #[test]
    fn test_process_impact_factors() {
        let state = State::default();
        let source = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Industrial Livestock Ag")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.electricity > 0.);
        assert!(source.mix_share > 0);
        let factors = impact_factor(source, Impact::Electricity, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "49");

        let source = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Industrial Livestock Ag")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.fuel > 0.);
        assert!(source.mix_share > 0);
        let factors = impact_factor(source, Impact::Fuel, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "136");

        let source = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Industrial Livestock Ag")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.fuel > 0.);
        assert!(source.resources.electricity > 0.);
        assert!(source.mix_share > 0);
        let factors = impact_factor(source, Impact::Energy, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "185");

        let source = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Industrial Livestock Ag")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.land > 0.);
        assert!(source.mix_share > 0);
        let factors = impact_factor(source, Impact::Land, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "35%");

        let source = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Industrial Livestock Ag")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.water > 0.);
        assert!(source.mix_share > 0);
        let factors = impact_factor(source, Impact::Water, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "11%");

        let source = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Industrial Livestock Ag")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.byproducts.gtco2eq() > 0.);
        assert!(source.mix_share > 0);
        let factors = impact_factor(source, Impact::Emissions, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "5.4Gt");
    }

    #[test]
    fn test_industry_impact_factors() {
        let state = State::default();
        let source = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Iron and Steel")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.electricity > 0.);
        let factors = impact_factor(source, Impact::Electricity, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "278");

        let source = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Iron and Steel")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.fuel > 0.);
        let factors = impact_factor(source, Impact::Fuel, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "1268");

        let source = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Iron and Steel")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.fuel > 0.);
        assert!(source.resources.electricity > 0.);
        let factors = impact_factor(source, Impact::Energy, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "1546");

        let source = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Other Industry")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.resources.water > 0.);
        let factors = impact_factor(source, Impact::Water, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "2%");

        let source = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Chemical")
            .unwrap();
        println!("Name {:?}", source.name);
        assert!(source.byproducts.gtco2eq() > 0.);
        let factors = impact_factor(source, Impact::Emissions, &state);
        println!("  {:?}", factors);
        assert_eq!(factors.display, "1.2Gt");
    }

    #[test]
    fn test_factor_card() {
        let state = State::default();
        let card = factors_card(None, Var::Fuel, &state);
        println!("{:?}: {}", Var::Fuel, card.total_formatted());
        assert_eq!(card.total_formatted(), "9068GW");

        let card = factors_card(None, Var::Electricity, &state);
        println!("{:?}: {}", Var::Electricity, card.total_formatted());
        assert_eq!(card.total_formatted(), "2972GW");

        let card = factors_card(None, Var::Energy, &state);
        println!("{:?}: {}", Var::Energy, card.total_formatted());
        assert_eq!(card.total_formatted(), "12040GW");

        let card = factors_card(None, Var::Emissions, &state);
        println!("{:?}: {}", Var::Emissions, card.total_formatted());
        assert_eq!(card.total_formatted(), "55.5Gt");

        let card = factors_card(None, Var::Land, &state);
        println!("{:?}: {}", Var::Land, card.total_formatted());
        assert_eq!(card.total_formatted(), "59%");

        let card = factors_card(None, Var::Water, &state);
        println!("{:?}: {}", Var::Water, card.total_formatted());
        assert_eq!(card.total_formatted(), "46%");

        let card = factors_card(None, Var::PlantCalories, &state);
        println!("{:?}: {}", Var::PlantCalories, card.total_formatted());
        assert_eq!(card.total_formatted(), "342");

        let card = factors_card(None, Var::AnimalCalories, &state);
        println!("{:?}: {}", Var::AnimalCalories, card.total_formatted());
        assert_eq!(card.total_formatted(), "73");

        let card = factors_card(None, Var::Biodiversity, &state);
        println!("{:?}: {}", Var::Biodiversity, card.total_formatted());
        assert_eq!(card.total_formatted(), "98");

        let card = factors_card(None, Var::Contentedness, &state);
        println!("{:?}: {}", Var::Contentedness, card.total_formatted());
        assert_eq!(card.total_formatted(), "30");
    }
}
