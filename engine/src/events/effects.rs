use super::{PlayerVariable, WorldVariable};
use crate::{
    Id,
    kinds::{Byproduct, Feedstock, Output, Resource},
    production::ProcessFeature,
    regions::{Latitude, Region},
    state::State,
};
use serde::{Deserialize, Serialize};
use std::ops::Mul;
use strum::{Display, EnumDiscriminants, EnumIter, EnumString, IntoStaticStr};

const MIGRATION_WAVE_PERCENT_POP: f32 = 0.1;
const CLOSED_BORDERS_MULTILPIER: f32 = 0.5;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum Request {
    Project,
    Process,
}

#[derive(
    Serialize, Deserialize, PartialEq, Debug, Clone, Copy, EnumIter, IntoStaticStr, EnumString,
)]
pub enum Flag {
    RepeatTutorial,
    SkipTutorial,
    Electrified,
    Vegetarian,
    Vegan,
    ClosedBorders,
    HyperResearch,
    StopDevelopment,
    FastDevelopment,
    Degrowth,
    MetalsShortage,
    DeepSeaMining,
    ParliamentSuspended,
    MoreLabor,
    MoreAutomation,
    MoreLeisure,
    EcosystemModeling,
    LaborResistance,
    LaborSabotage,
    AlienEncounter,
    BailedOut,
    LifeGoesOn,
    WarFooting,
    FossilMoratorium,
    SodiumIonBatteries,
    PumpedHydroStorage,
    LongDurationStorage,
    ScheduledBlackouts,
}
impl std::fmt::Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let desc = match self {
            Self::HyperResearch => "Research points are cheaper.",
            Self::ClosedBorders => "Limits cross-region migration.",
            Self::AlienEncounter => "Encountered extraterrestrials",
            Self::ParliamentSuspended => {
                "A parliamentary majority is no longer required for any project."
            }
            Self::Electrified => "80% of fuel demand is electrified, needing about half as much electricity (heat pumps and motors waste far less than burning fuel).",
            Self::Vegan => "90% of animal calorie demand is met with plant calories.",
            Self::BailedOut => "You've been bailed out once",
            Self::WarFooting => "Energy, electrification and buildings projects take 20% less time to complete, and production can shift 30% per planning cycle instead of 25%.",
            Self::FossilMoratorium => "Fossil fuel processes can no longer be expanded.",
            Self::SodiumIonBatteries => "Sodium-ion batteries take over part of grid storage, needing no lithium.",
            Self::PumpedHydroStorage => "Pumped hydro takes over part of grid storage, needing no lithium.",
            Self::LongDurationStorage => "Long-duration storage takes over part of grid storage, needing no lithium but losing more energy.",
            Self::ScheduledBlackouts => "Power is cut on a schedule at night, halving how much energy needs storing.",
            Self::FastDevelopment => "Underdeveloped regions develop more quickly.",
            Self::Degrowth => "Wealthy regions income levels and consumption will decline.",
            Self::MetalsShortage => "Infrastructure projects take 20% longer to finish.",
            Self::MoreLabor => "Projects take less time to complete.",
            Self::LaborResistance => "Projects take longer to complete.",
            Self::MoreLeisure => "Projects take more time to complete.",
            Self::DeepSeaMining => "Stops or prevents metals shortages.",
            Self::MoreAutomation => "Projects take less time to complete.",
            Self::Vegetarian => "75% of animal calorie demand is met with plant calories.",
            Self::StopDevelopment => "Stops regional development throughout the world.",
            Self::LaborSabotage => "Projects take longer to complete.",
            Self::EcosystemModeling => "Restoration projects take less time to complete.",
            Self::RepeatTutorial => "Repeat the tutorial.",
            Self::SkipTutorial => "Skip the tutorial.",
            Self::LifeGoesOn => "The game never ends.",
        };
        write!(f, "{}", desc)
    }
}

#[derive(
    Serialize,
    Deserialize,
    PartialEq,
    Debug,
    Clone,
    Copy,
    EnumIter,
    IntoStaticStr,
    EnumString,
    Display,
)]
pub enum RegionFlag {
    Protests,
    Riots,
    Revolts,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter, EnumString, IntoStaticStr, Display))]
#[strum_discriminants(name(EffectKind))]
pub enum Effect {
    WorldVariable(WorldVariable, f32),
    PlayerVariable(PlayerVariable, f32),
    RegionHabitability(Latitude, f32),

    Resource(Resource, f32),
    Demand(Output, f32),
    Output(Output, f32),
    DemandAmount(Output, f32),
    OutputForFeature(ProcessFeature, f32),
    OutputForProcess(Id, f32),
    CO2ForFeature(ProcessFeature, f32),
    BiodiversityPressureForFeature(ProcessFeature, f32),
    ProcessLimit(Id, f32),

    /// Cap a process's output at what it produces when this is applied.
    FreezeProcess(Id),

    /// A temporary cut to field farming yields for an output (fraction,
    /// e.g. -0.05), which recovers evenly over the given number of years.
    YieldShock(Output, f32, usize),
    Feedstock(Feedstock, f32),

    AddEvent(Id),
    TriggerEvent(Id, usize),
    LocksProject(Id),
    UnlocksProject(Id),
    UnlocksProcess(Id),
    UnlocksNPC(Id),

    ProjectRequest(Id, bool, usize),
    ProcessRequest(Id, bool, usize),

    Migration,
    RegionLeave,
    TerminationShock,
    AddRegionFlag(RegionFlag),

    AddFlag(Flag),
    NPCRelationship(Id, f32),

    ModifyProcessByproducts(Id, Byproduct, f32),
    ModifyIndustryByproducts(Id, Byproduct, f32),
    ModifyIndustryResources(Id, Resource, f32),
    ModifyIndustryResourcesAmount(Id, Resource, f32),
    ModifyEventProbability(Id, f32),
    ModifyIndustryDemand(Id, f32),
    DemandOutlookChange(Output, f32),
    IncomeOutlookChange(f32),
    /// A change in contentedness for regions at each income level
    /// (low, lower-middle, upper-middle, high).
    OutlookByIncome([f32; 4]),
    ProjectCostModifier(Id, f32),

    ProtectLand(f32),

    BailOut(usize),
    GameOver,
}
impl AsRef<Effect> for Effect {
    fn as_ref(&self) -> &Effect {
        self
    }
}

impl Default for Effect {
    fn default() -> Self {
        Effect::PlayerVariable(PlayerVariable::PoliticalCapital, 10.)
    }
}

impl Effect {
    pub fn from_kind(
        kind: EffectKind,
        default_process: Id,
        default_project: Id,
        default_industry: Id,
        default_event: Id,
        default_npc: Id,
    ) -> Self {
        match kind {
            EffectKind::WorldVariable => Effect::WorldVariable(WorldVariable::Outlook, 0.),
            EffectKind::PlayerVariable => {
                Effect::PlayerVariable(PlayerVariable::PoliticalCapital, 0.)
            }
            EffectKind::RegionHabitability => Effect::RegionHabitability(Latitude::Tropic, 0.),
            EffectKind::Resource => Effect::Resource(Resource::Land, 0.),
            EffectKind::Demand => Effect::Demand(Output::Fuel, 0.),
            EffectKind::Output => Effect::Output(Output::Fuel, 0.),
            EffectKind::DemandAmount => Effect::DemandAmount(Output::Fuel, 0.),
            EffectKind::OutputForFeature => Effect::OutputForFeature(ProcessFeature::IsCCS, 0.),
            EffectKind::OutputForProcess => Effect::OutputForProcess(default_process, 0.),
            EffectKind::CO2ForFeature => Effect::CO2ForFeature(ProcessFeature::IsCCS, 0.),
            EffectKind::BiodiversityPressureForFeature => {
                Effect::BiodiversityPressureForFeature(ProcessFeature::IsCCS, 0.)
            }
            EffectKind::ProcessLimit => Effect::ProcessLimit(default_process, 0.),
            EffectKind::FreezeProcess => Effect::FreezeProcess(default_process),
            EffectKind::YieldShock => Effect::YieldShock(Output::PlantCalories, -0.05, 3),
            EffectKind::Feedstock => Effect::Feedstock(Feedstock::Coal, 0.),
            EffectKind::AddEvent => Effect::AddEvent(default_event),
            EffectKind::TriggerEvent => Effect::TriggerEvent(default_event, 5),
            EffectKind::LocksProject => Effect::LocksProject(default_project),
            EffectKind::UnlocksProject => Effect::UnlocksProject(default_project),
            EffectKind::UnlocksProcess => Effect::UnlocksProcess(default_process),
            EffectKind::UnlocksNPC => Effect::UnlocksNPC(default_npc),
            EffectKind::ProjectRequest => Effect::ProjectRequest(default_project, true, 20),
            EffectKind::ProcessRequest => Effect::ProcessRequest(default_process, true, 20),
            EffectKind::Migration => Effect::Migration,
            EffectKind::RegionLeave => Effect::RegionLeave,
            EffectKind::TerminationShock => Effect::TerminationShock,
            EffectKind::AddRegionFlag => Effect::AddRegionFlag(RegionFlag::Protests),
            EffectKind::AddFlag => Effect::AddFlag(Flag::Vegan),
            EffectKind::NPCRelationship => Effect::NPCRelationship(default_npc, 0.),
            EffectKind::ModifyProcessByproducts => {
                Effect::ModifyProcessByproducts(default_process, Byproduct::Co2, 0.)
            }
            EffectKind::ModifyIndustryByproducts => {
                Effect::ModifyIndustryByproducts(default_industry, Byproduct::Co2, 0.)
            }
            EffectKind::ModifyIndustryResources => {
                Effect::ModifyIndustryResources(default_industry, Resource::Land, 0.)
            }
            EffectKind::ModifyIndustryResourcesAmount => {
                Effect::ModifyIndustryResourcesAmount(default_industry, Resource::Land, 0.)
            }
            EffectKind::ModifyEventProbability => Effect::ModifyEventProbability(default_event, 0.),
            EffectKind::ModifyIndustryDemand => Effect::ModifyIndustryDemand(default_industry, 0.),
            EffectKind::DemandOutlookChange => Effect::DemandOutlookChange(Output::Fuel, 0.),
            EffectKind::IncomeOutlookChange => Effect::IncomeOutlookChange(0.),
            EffectKind::OutlookByIncome => Effect::OutlookByIncome([0.; 4]),
            EffectKind::ProjectCostModifier => Effect::ProjectCostModifier(default_project, 0.),
            EffectKind::ProtectLand => Effect::ProtectLand(0.1),
            EffectKind::BailOut => Effect::BailOut(20),
            EffectKind::GameOver => Effect::GameOver,
        }
    }

    pub fn process_id(&self) -> Option<Id> {
        match self {
            Effect::OutputForProcess(id, _)
            | Effect::ProcessLimit(id, _)
            | Effect::FreezeProcess(id)
            | Effect::UnlocksProcess(id)
            | Effect::ProcessRequest(id, ..)
            | Effect::ModifyProcessByproducts(id, ..) => Some(*id),
            _ => None,
        }
    }

    pub fn project_id(&self) -> Option<Id> {
        match self {
            Effect::LocksProject(id)
            | Effect::UnlocksProject(id)
            | Effect::ProjectRequest(id, ..)
            | Effect::ProjectCostModifier(id, ..) => Some(*id),
            _ => None,
        }
    }

    pub fn industry_id(&self) -> Option<Id> {
        match self {
            Effect::ModifyIndustryByproducts(id, ..)
            | Effect::ModifyIndustryResources(id, ..)
            | Effect::ModifyIndustryResourcesAmount(id, ..)
            | Effect::ModifyIndustryDemand(id, ..) => Some(*id),
            _ => None,
        }
    }

    pub fn event_id(&self) -> Option<Id> {
        match self {
            Effect::AddEvent(id)
            | Effect::TriggerEvent(id, _)
            | Effect::ModifyEventProbability(id, _) => Some(*id),
            _ => None,
        }
    }
}

impl Effect {
    /// For comparing if two effects are of the same "type"
    /// and thus may be alternatives to one another.
    pub fn fingerprint(&self) -> String {
        let discrim: EffectKind = self.into();
        let discrim: &'static str = discrim.into();
        let subkind: &'static str = match self {
            Self::WorldVariable(var, _) => var.into(),
            Self::PlayerVariable(var, _) => var.into(),
            Self::RegionHabitability(lat, _) => lat.into(),
            Self::Resource(res, _) => res.into(),
            Self::Demand(out, _) => out.into(),
            Self::Output(out, _) => out.into(),
            Self::DemandAmount(out, _) => out.into(),
            Self::OutputForFeature(feat, _) => feat.into(),
            Self::CO2ForFeature(feat, _) => feat.into(),
            Self::BiodiversityPressureForFeature(feat, _) => feat.into(),
            Self::Feedstock(fs, _) => fs.into(),
            Self::ModifyProcessByproducts(_, byp, _) => byp.into(),
            Self::ModifyIndustryByproducts(_, byp, _) => byp.into(),
            Self::ModifyIndustryResources(_, res, _) => res.into(),
            Self::ModifyIndustryResourcesAmount(_, res, _) => res.into(),
            Self::DemandOutlookChange(out, _) => out.into(),
            _ => "",
        };
        format!("{discrim}:{subkind}")
    }

    pub fn apply(&self, state: &mut State, region_id: Option<Id>) {
        match self {
            Effect::GameOver => {
                if !state.flags.contains(&Flag::LifeGoesOn) {
                    state.game_over = true;
                }
            }
            Effect::BailOut(amount) => {
                if state.political_capital < 0 {
                    state.political_capital = 0;
                }
                state.political_capital += *amount as isize;
            }
            Effect::WorldVariable(var, change) => {
                match var {
                    WorldVariable::Year => {
                        if *change > 0. {
                            state.world.year += *change as usize
                        }
                    }
                    WorldVariable::Population => state.world.regions.change_population(*change),
                    WorldVariable::PopulationGrowth => {
                        state.world.population_growth_modifier += *change
                    }
                    WorldVariable::Emissions => {
                        state.byproducts.modifier.co2 += *change * 1e15; // effect in Gt
                        state.emissions.co2 += *change * 1e15; // Apply immediately
                    }
                    WorldVariable::ExtinctionRate => {
                        state.world.extinction_rate_modifier += *change
                    }
                    WorldVariable::Outlook => {
                        state.world.base_outlook += *change;
                    }
                    WorldVariable::Temperature => state.world.temperature_modifier += *change,
                    WorldVariable::SeaLevelRise => state.world.sea_level_rise += *change,
                    WorldVariable::Deforestation => {
                        state.land_carbon.deforestation_modifier += *change
                    }
                    WorldVariable::Regrowth => state.land_carbon.regrowth_modifier += *change,
                    WorldVariable::BatteryRecycling => state.storage.recycling += *change,
                    WorldVariable::LithiumMining => state.storage.mining_modifier += *change,
                    WorldVariable::StorageLosses => state.storage.losses_modifier += *change,
                    WorldVariable::LandClearing
                    | WorldVariable::LandRegrowth
                    | WorldVariable::StorageShortfall => {}
                    WorldVariable::SeaLevelRiseRate => {
                        state.world.sea_level_rise_modifier += *change
                    }
                    WorldVariable::Precipitation => state.world.precipitation += *change,
                }
            }
            Effect::PlayerVariable(var, change) => match var {
                PlayerVariable::PoliticalCapital => state.political_capital += *change as isize,
                PlayerVariable::ResearchPoints => state.research_points += *change as isize,
                _ => (),
            },
            Effect::RegionHabitability(latitude, change) => {
                for region in state
                    .world
                    .regions
                    .iter_mut()
                    .filter(|r| &r.latitude == latitude)
                {
                    region.base_habitability += change;
                }
            }
            Effect::Resource(resource, amount) => {
                state.resources.available[*resource] += amount;
            }
            Effect::Demand(output, pct_change) => {
                for demand in &mut state.world.per_capita_demand {
                    demand.factor[*output] += pct_change;
                }
            }
            Effect::DemandAmount(output, amount) => {
                state.output_demand.modifier[*output] += amount;
            }
            Effect::Output(output, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.output == *output)
                {
                    process.output_modifier += pct_change;
                }
            }
            Effect::OutputForFeature(feat, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.features.contains(feat))
                {
                    process.output_modifier += pct_change;
                }
            }
            Effect::OutputForProcess(id, pct_change) => {
                let process = &mut state.world.processes[id];
                process.output_modifier += pct_change;
            }
            Effect::CO2ForFeature(feat, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.features.contains(feat))
                {
                    process.byproduct_modifiers.co2 += pct_change;
                }
            }
            Effect::BiodiversityPressureForFeature(feat, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.features.contains(feat))
                {
                    process.byproduct_modifiers.biodiversity += pct_change;
                }
            }
            Effect::YieldShock(output, change, years) => {
                state.apply_yield_shock(*output, *change, *years);
            }
            Effect::FreezeProcess(id) => {
                let produced = state.produced.by_process.get(id).copied().unwrap_or(0.);
                let process = &mut state.world.processes[id];
                let base = produced / (1. + process.produced_modifier).max(0.01);
                process.frozen_limit = Some(base);
            }
            Effect::ProcessLimit(id, change) => {
                let process = &mut state.world.processes[id];
                if let Some(limit) = process.limit {
                    process.limit = Some(limit + change);
                }
            }
            Effect::Feedstock(feedstock, pct_change) => {
                state.feedstocks.available[*feedstock] *= 1. + pct_change;
            }
            Effect::AddEvent(id) => {
                state.event_pool.events[id].locked = false;
            }
            Effect::TriggerEvent(id, years) => {
                state.event_pool.queue_event(*id, region_id, *years);
            }
            Effect::LocksProject(id) => {
                state.world.projects[id].locked = true;
            }
            Effect::UnlocksProject(id) => {
                state.world.projects[id].locked = false;
            }
            Effect::UnlocksProcess(id) => {
                state.world.processes[id].locked = false;
            }
            Effect::UnlocksNPC(id) => {
                state.npcs[id].locked = false;
            }
            Effect::ProjectRequest(id, active, bounty) => {
                state
                    .requests
                    .push((Request::Project, *id, *active, *bounty));
            }
            Effect::ProcessRequest(id, active, bounty) => {
                state
                    .requests
                    .push((Request::Process, *id, *active, *bounty));
            }
            Effect::Migration => {
                if let Some(id) = &region_id {
                    let modifier = if state.flags.contains(&Flag::ClosedBorders) {
                        CLOSED_BORDERS_MULTILPIER
                    } else {
                        1.
                    };
                    let leave_pop =
                        state.world.regions[id].population * MIGRATION_WAVE_PERCENT_POP * modifier;
                    state.world.regions[id].population -= leave_pop;

                    // Find the most habitable regions
                    let mean_habitability: f32 = state.world.regions.habitability();
                    let target_regions: Vec<&mut Region> = state
                        .world
                        .regions
                        .iter_mut()
                        .filter(|r| &r.id != id && r.habitability() > mean_habitability)
                        .collect();
                    let per_region = leave_pop / target_regions.len() as f32;
                    for region in target_regions {
                        region.population += per_region;
                    }
                }
            }
            Effect::RegionLeave => {
                if let Some(id) = &region_id {
                    state.world.regions[id].seceded = true;
                }
            }
            Effect::AddRegionFlag(flag) => {
                if let Some(id) = &region_id {
                    state.world.regions[id].flags.push(*flag);
                }
            }
            Effect::AddFlag(flag) => {
                state.flags.push(*flag);
            }
            Effect::NPCRelationship(id, change) => {
                state.npcs[id].relationship += change;
            }

            Effect::ModifyProcessByproducts(id, byproduct, change) => {
                state.world.processes[id].byproduct_modifiers[*byproduct] += change;
            }
            Effect::ModifyIndustryByproducts(id, byproduct, change) => {
                state.world.industries[id].byproduct_modifiers[*byproduct] += change;
            }
            Effect::ModifyIndustryResources(id, resource, change) => {
                state.world.industries[id].resource_modifiers[*resource] += change;
            }
            Effect::ModifyIndustryResourcesAmount(id, resource, change) => {
                state.world.industries[id].resources[*resource] += change;
            }
            Effect::ModifyEventProbability(id, change) => {
                state.event_pool.events[id].prob_modifier += change;
            }
            Effect::ModifyIndustryDemand(id, change) => {
                state.world.industries[id].demand_modifier += change;
            }
            Effect::DemandOutlookChange(output, mult) => {
                for region in state.world.regions.iter_mut() {
                    region.outlook += (mult
                        * region.demand_level(output, &state.world.per_capita_demand) as f32)
                        .round();
                }
            }
            Effect::IncomeOutlookChange(mult) => {
                for region in state.world.regions.iter_mut() {
                    region.outlook += (mult * region.income.level() as f32).round();
                }
            }
            Effect::OutlookByIncome(amounts) => {
                for region in state.world.regions.iter_mut() {
                    region.outlook += amounts[region.income.level()];
                }
            }
            Effect::ProjectCostModifier(id, change) => {
                state.world.projects[id].cost_modifier += change;
            }
            Effect::TerminationShock => {
                let p = state
                    .world
                    .projects
                    .iter()
                    .find(|p| {
                        // HACK: Not great to be matching on the
                        // project's name; ideally would introduce
                        // a flag effect that mirrors `TerminationShock`
                        // and match on any project that contains that flag,
                        // but that may be a complicated change to make at this point.
                        p.name.contains("Solar Radiation Management")
                    })
                    .unwrap();
                let effects = p.active_effects();
                let mut temp = 0.;
                for eff in effects {
                    if let Effect::WorldVariable(WorldVariable::Temperature, val) = eff {
                        temp += val
                    };
                }
                state.world.temperature_modifier -= temp;
            }
            Effect::ProtectLand(percent) => {
                state.protected_land += percent;
            }
        }
    }

    pub fn unapply(&self, state: &mut State, _region_id: Option<Id>) {
        match self {
            Effect::WorldVariable(var, change) => {
                match var {
                    WorldVariable::Year => {
                        if *change > 0. {
                            state.world.year -= *change as usize
                        }
                    }
                    WorldVariable::Population => state.world.regions.change_population(-*change),
                    WorldVariable::PopulationGrowth => {
                        state.world.population_growth_modifier -= *change
                    }
                    WorldVariable::Emissions => {
                        state.byproducts.modifier.co2 -= *change * 1e15;
                        state.emissions.co2 -= *change * 1e15; // Apply immediately
                    }
                    WorldVariable::ExtinctionRate => {
                        state.world.extinction_rate_modifier -= *change
                    }
                    WorldVariable::Outlook => state.world.base_outlook -= *change,
                    WorldVariable::Temperature => state.world.temperature_modifier -= *change,
                    WorldVariable::SeaLevelRise => state.world.sea_level_rise -= *change,
                    WorldVariable::Deforestation => {
                        state.land_carbon.deforestation_modifier -= *change
                    }
                    WorldVariable::Regrowth => state.land_carbon.regrowth_modifier -= *change,
                    WorldVariable::BatteryRecycling => state.storage.recycling -= *change,
                    WorldVariable::LithiumMining => state.storage.mining_modifier -= *change,
                    WorldVariable::StorageLosses => state.storage.losses_modifier -= *change,
                    WorldVariable::LandClearing
                    | WorldVariable::LandRegrowth
                    | WorldVariable::StorageShortfall => {}
                    WorldVariable::SeaLevelRiseRate => {
                        state.world.sea_level_rise_modifier -= *change
                    }
                    WorldVariable::Precipitation => state.world.precipitation -= *change,
                }
            }
            Effect::PlayerVariable(var, change) => match var {
                PlayerVariable::PoliticalCapital => state.political_capital -= *change as isize,
                PlayerVariable::ResearchPoints => state.research_points -= *change as isize,
                _ => (),
            },
            Effect::RegionHabitability(latitude, change) => {
                for region in state
                    .world
                    .regions
                    .iter_mut()
                    .filter(|r| &r.latitude == latitude)
                {
                    region.base_habitability -= change;
                }
            }
            Effect::Resource(resource, amount) => {
                state.resources.available[*resource] -= amount;
            }
            Effect::Demand(output, pct_change) => {
                for demand in &mut state.world.per_capita_demand {
                    demand.factor[*output] -= pct_change;
                }
            }
            Effect::DemandAmount(output, amount) => {
                state.output_demand.modifier[*output] -= amount;
            }
            Effect::Output(output, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.output == *output)
                {
                    process.output_modifier -= pct_change;
                }
            }
            Effect::OutputForFeature(feat, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.features.contains(feat))
                {
                    process.output_modifier -= pct_change;
                }
            }
            Effect::OutputForProcess(id, pct_change) => {
                let process = &mut state.world.processes[id];
                process.output_modifier -= pct_change;
            }
            Effect::CO2ForFeature(feat, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.features.contains(feat))
                {
                    process.byproduct_modifiers.co2 -= pct_change;
                }
            }
            Effect::BiodiversityPressureForFeature(feat, pct_change) => {
                for process in state
                    .world
                    .processes
                    .iter_mut()
                    .filter(|p| p.features.contains(feat))
                {
                    process.byproduct_modifiers.biodiversity -= pct_change;
                }
            }
            Effect::YieldShock(..) => {
                // Shocks recover on their own; they are only used by events,
                // which are never unapplied.
            }
            Effect::FreezeProcess(id) => {
                state.world.processes[id].frozen_limit = None;
            }
            Effect::ProcessLimit(id, change) => {
                let process = &mut state.world.processes[id];
                if let Some(limit) = process.limit {
                    process.limit = Some(limit - change);
                }
            }
            Effect::Feedstock(feedstock, pct_change) => {
                state.feedstocks.available[*feedstock] /= 1. + pct_change;
            }
            Effect::NPCRelationship(id, change) => {
                state.npcs[id].relationship -= change;
            }
            Effect::ModifyProcessByproducts(id, byproduct, change) => {
                state.world.processes[id].byproduct_modifiers[*byproduct] -= change;
            }
            Effect::ModifyIndustryByproducts(id, byproduct, change) => {
                state.world.industries[id].byproduct_modifiers[*byproduct] -= change;
            }
            Effect::ModifyIndustryResources(id, resource, change) => {
                state.world.industries[id].resource_modifiers[*resource] -= change;
            }
            Effect::ModifyIndustryResourcesAmount(id, resource, change) => {
                state.world.industries[id].resources[*resource] -= change;
            }
            Effect::ModifyEventProbability(id, change) => {
                state.event_pool.events[id].prob_modifier -= change;
            }
            Effect::ModifyIndustryDemand(id, change) => {
                state.world.industries[id].demand_modifier -= change;
            }
            Effect::DemandOutlookChange(output, mult) => {
                for region in state.world.regions.iter_mut() {
                    region.outlook -= (mult
                        * region.demand_level(output, &state.world.per_capita_demand) as f32)
                        .floor();
                }
            }
            // Undo exactly what `apply` added (upstream used floor here,
            // which didn't match apply's rounding for positive changes).
            Effect::IncomeOutlookChange(mult) => {
                for region in state.world.regions.iter_mut() {
                    region.outlook -= (mult * region.income.level() as f32).round();
                }
            }
            Effect::OutlookByIncome(amounts) => {
                for region in state.world.regions.iter_mut() {
                    region.outlook -= amounts[region.income.level()];
                }
            }
            Effect::ProjectCostModifier(id, change) => {
                state.world.projects[id].cost_modifier -= change;
            }
            Effect::TerminationShock => {
                let p = state
                    .world
                    .projects
                    .iter()
                    .find(|p| {
                        // HACK: Not great to be matching on the
                        // project's name; ideally would introduce
                        // a flag effect that mirrors `TerminationShock`
                        // and match on any project that contains that flag,
                        // but that may be a complicated change to make at this point.
                        p.name.contains("Solar Radiation Management")
                    })
                    .unwrap();
                let effects = p.active_effects();
                let mut temp = 0.;
                for eff in effects {
                    if let Effect::WorldVariable(WorldVariable::Temperature, val) = eff {
                        temp += val;
                    };
                }
                state.world.temperature_modifier += temp;
            }
            Effect::ProtectLand(percent) => {
                state.protected_land -= percent;
            }
            Effect::AddFlag(flag) => {
                if let Some(idx) = state.flags.iter().position(|x| x == flag) {
                    state.flags.remove(idx);
                }
            }
            Effect::LocksProject(id) => {
                state.world.projects[id].locked = false;
            }
            Effect::UnlocksProject(id) => {
                state.world.projects[id].locked = true;
            }
            Effect::UnlocksProcess(id) => {
                state.world.processes[id].locked = true;
            }
            Effect::UnlocksNPC(id) => {
                state.npcs[id].locked = true;
            }

            // Other effects aren't reversible
            _ => (),
        }
    }
}

// For scaling effects by float
impl Mul<f32> for Effect {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        match self {
            Effect::WorldVariable(var, val) => Effect::WorldVariable(var, val * rhs),
            Effect::PlayerVariable(var, val) => Effect::PlayerVariable(var, val * rhs),
            Effect::Resource(resource, val) => Effect::Resource(resource, val * rhs),
            Effect::Demand(output, val) => Effect::Demand(output, val * rhs),
            Effect::Output(output, val) => Effect::Output(output, val * rhs),
            Effect::DemandAmount(output, val) => Effect::DemandAmount(output, val * rhs),
            Effect::OutputForFeature(feat, val) => Effect::OutputForFeature(feat, val * rhs),
            Effect::OutputForProcess(id, val) => Effect::OutputForProcess(id, val * rhs),
            Effect::Feedstock(feedstock, val) => Effect::Feedstock(feedstock, val * rhs),
            Effect::ModifyIndustryByproducts(id, byproduct, val) => {
                Effect::ModifyIndustryByproducts(id, byproduct, val * rhs)
            }
            Effect::ModifyIndustryResources(id, resource, val) => {
                Effect::ModifyIndustryResources(id, resource, val * rhs)
            }
            // Upstream bug: this became a ModifyIndustryResources (a
            // multiplier), so gradual projects multiplied resource use
            // instead of adding to it while they were being built.
            Effect::ModifyIndustryResourcesAmount(id, resource, val) => {
                Effect::ModifyIndustryResourcesAmount(id, resource, val * rhs)
            }
            Effect::ModifyIndustryDemand(id, val) => Effect::ModifyIndustryDemand(id, val * rhs),
            Effect::ModifyEventProbability(id, val) => {
                Effect::ModifyEventProbability(id, val * rhs)
            }
            Effect::DemandOutlookChange(output, val) => {
                Effect::DemandOutlookChange(output, val * rhs)
            }
            Effect::IncomeOutlookChange(val) => Effect::IncomeOutlookChange(val * rhs),
            Effect::OutlookByIncome(vals) => Effect::OutlookByIncome(vals.map(|v| v * rhs)),
            Effect::ProjectCostModifier(id, val) => Effect::ProjectCostModifier(id, val * rhs),
            Effect::ProtectLand(val) => Effect::ProtectLand(val * rhs),
            _ => self,
        }
    }
}

pub fn mean_outlook_by_income(amounts: &[f32; 4], state: &State) -> f32 {
    state
        .world
        .regions
        .iter()
        .map(|region| amounts[region.income.level()])
        .sum::<f32>()
        / state.world.regions.len() as f32
}

pub fn mean_income_outlook_change(mult: f32, state: &State) -> f32 {
    state
        .world
        .regions
        .iter()
        .map(|region| (mult * region.income.level() as f32).floor())
        .sum::<f32>()
        / state.world.regions.len() as f32
}

pub fn mean_demand_outlook_change(mult: f32, output: &Output, state: &State) -> f32 {
    state
        .world
        .regions
        .iter()
        .map(|region| {
            (mult * region.demand_level(output, &state.world.per_capita_demand) as f32).floor()
        })
        .sum::<f32>()
        / state.world.regions.len() as f32
}

#[cfg(test)]
mod tests {
    use crate::Status;

    use super::*;

    #[test]
    fn test_extinction_rate_effects_apply() {
        let mut state = State::default();
        state.world.update_extinction_rate(&state.produced.by_process);
        let before = state.world.extinction_rate;

        let effect = Effect::WorldVariable(WorldVariable::ExtinctionRate, 7.);
        effect.apply(&mut state, None);
        state.world.update_extinction_rate(&state.produced.by_process);
        assert!((state.world.extinction_rate - (before + 7.)).abs() < 1e-3);

        effect.unapply(&mut state, None);
        state.world.update_extinction_rate(&state.produced.by_process);
        assert!((state.world.extinction_rate - before).abs() < 1e-3);
    }

    #[test]
    fn test_smallholder_upgrade_path() {
        let mut state = State::default();
        let find_process = |state: &State, name: &str| {
            state.world.processes.iter().find(|p| p.name == name).unwrap().clone()
        };
        let project_effects = |state: &State, name: &str| {
            state.world.projects.iter().find(|p| p.name == name).unwrap().effects.clone()
        };

        let start = find_process(&state, "Smallholder Farms");
        assert!(start.adj_byproducts().biodiversity > 0.);

        for name in [
            "Reconcile Town and Country",
            "Regenerative Agriculture",
            "Multistrata Agroforestry",
            "Compost",
            "Biochar",
        ] {
            let effects = project_effects(&state, name);
            state.apply_effects(&effects, None);
        }
        let end = find_process(&state, "Smallholder Farms");
        // Biodiversity pressure 0.1 * (1 - 5) = -0.4, with half the N2O.
        assert!((end.byproducts.biodiversity * (1. + end.byproduct_modifiers.biodiversity) + 0.4).abs() < 1e-4);
        assert!((end.byproduct_modifiers.n2o + 0.5).abs() < 1e-4);
        assert!(end.adj_resources().land < start.adj_resources().land);

        // The pesticide ban costs industrial ag yield but halves its biodiversity pressure.
        let industrial = find_process(&state, "Industrial Crop Ag");
        let effects = project_effects(&state, "Enforce Pesticide Ban");
        state.apply_effects(&effects, None);
        let banned = find_process(&state, "Industrial Crop Ag");
        assert!(banned.adj_resources().land > industrial.adj_resources().land);
        assert!(
            (banned.byproducts.biodiversity * (1. + banned.byproduct_modifiers.biodiversity)
                - industrial.byproducts.biodiversity * 0.5)
                .abs()
                < 1e-4
        );
    }

    #[test]
    fn test_vertical_farming_gets_one_share() {
        let state = State::default();
        let vf = state.world.processes.iter().find(|p| p.name == "Vertical Farming").unwrap();
        let mut demand = state.output_demand.total();
        for plant_demand in [6.9e15, 1.0e16, 1.35e16] {
            demand.plant_calories = plant_demand;
            assert_eq!(vf.max_share(&demand, &state.feedstocks.available), 1, "at {plant_demand:e}");
        }
    }

    #[test]
    fn test_fossil_moratorium_blocks_growth_not_shrinking() {
        let mut state = State::default();
        let coal = state.world.processes.iter().find(|p| p.name == "Coal Power Generation").unwrap().id;
        let solar = state.world.processes.iter().find(|p| p.name == "Solar PV").unwrap().id;
        let share = state.world.processes[&coal].mix_share;
        assert!(state.process_max_share(&coal) > share);

        Effect::AddFlag(Flag::FossilMoratorium).apply(&mut state, None);
        assert_eq!(state.process_max_share(&coal), share);
        assert!(state.process_max_share(&solar) > state.world.processes[&solar].mix_share);

        state.change_process_mix_share(&coal, -1);
        assert_eq!(state.process_max_share(&coal), share - 1);

        Effect::AddFlag(Flag::FossilMoratorium).unapply(&mut state, None);
        assert!(state.process_max_share(&coal) > share - 1);
    }

    #[test]
    fn test_war_footing_speeds_up_energy_projects() {
        use crate::{Group, ProjectType};
        let mut state = State::default();
        let pick = |state: &State, group: Group| {
            state
                .world
                .projects
                .iter()
                .find(|p| p.group == group && p.kind != ProjectType::Policy && p.cost > 0)
                .unwrap()
                .id
        };
        state.step_year(state.world.temperature);
        let energy = pick(&state, Group::Energy);
        let food = pick(&state, Group::Food);
        let (energy_before, food_before) =
            (state.world.projects[&energy].cost, state.world.projects[&food].cost);

        Effect::AddFlag(Flag::WarFooting).apply(&mut state, None);
        state.step_year(state.world.temperature);
        assert!((state.world.projects[&energy].cost as f32) < energy_before as f32 * 0.9);
        assert!(state.world.projects[&food].cost >= food_before);
    }

    #[test]
    fn test_species_recovery_events_unlock() {
        let mut state = State::default();
        for (extinct, recovery) in [
            ("Mountain Gorilla Declared Extinct", "Mountain Gorillas Return"),
            ("Vaquita Porpoise Declared Extinct", "The Vaquita Swims Again"),
            ("Polar Bear Declared Extinct", "Polar Bears Return to the Ice"),
            ("Great Barrier Reef Dead", "Coral Returns to the Great Barrier Reef"),
        ] {
            let recovery = state.event_pool.events.iter().find(|e| e.name == recovery).unwrap().id;
            assert!(state.event_pool.events[&recovery].locked);
            let effects = state.event_pool.events.iter().find(|e| e.name == extinct).unwrap().effects.clone();
            state.apply_effects(&effects, None);
            assert!(!state.event_pool.events[&recovery].locked, "{extinct}");
        }
    }

    #[test]
    fn test_land_effects_apply_and_unapply() {
        let mut state = State::default();
        let defor = Effect::WorldVariable(WorldVariable::Deforestation, -0.3);
        let regrow = Effect::WorldVariable(WorldVariable::Regrowth, 0.5);
        defor.apply(&mut state, None);
        regrow.apply(&mut state, None);
        assert!((state.land_carbon.deforestation_modifier + 0.3).abs() < 1e-6);
        assert!((state.land_carbon.regrowth_modifier - 0.5).abs() < 1e-6);
        defor.unapply(&mut state, None);
        regrow.unapply(&mut state, None);
        assert_eq!(state.land_carbon.deforestation_modifier, 0.);
        assert_eq!(state.land_carbon.regrowth_modifier, 0.);
    }

    #[test]
    fn test_factory_farming_reform_freezes_capacity() {
        use crate::Output;
        let mut state = State::default();
        let livestock = state.world.processes.iter().find(|p| p.name == "Industrial Livestock Ag").unwrap().id;
        let effects = state.world.projects.iter().find(|p| p.name == "Factory Farming Reform").unwrap().effects.clone();
        state.step_year(state.world.temperature);
        let frozen_at = state.produced.by_process[&livestock];
        state.apply_effects(&effects, None);

        // Meat demand keeps growing, but industrial livestock can't, beyond
        // what better yields get out of the same farms.
        let modifier_then = state.world.processes[&livestock].output_modifier;
        for _ in 0..10 {
            state.step_year(state.world.temperature);
        }
        let yield_growth =
            (1. + state.world.processes[&livestock].output_modifier) / (1. + modifier_then);
        assert!(state.produced.by_process[&livestock] <= frozen_at * yield_growth * 1.0001);
        assert!(state.produced.amount.animal_calories < state.output_demand.of(Output::AnimalCalories));

        // Repealing lifts the freeze.
        for effect in &effects {
            effect.unapply(&mut state, None);
        }
        state.step_year(state.world.temperature);
        assert!(state.produced.by_process[&livestock] > frozen_at);
    }

    #[test]
    fn test_history_is_recorded_each_year() {
        let mut state = State::default();
        let start = state.world.year;
        for _ in 0..7 {
            state.step_year(state.world.temperature);
        }
        let years: Vec<usize> = state.history.iter().map(|r| r.year).collect();
        assert_eq!(years, (start..=start + 7).collect::<Vec<_>>());
        assert!(state.history.iter().all(|r| r.emissions > 0.));
    }

    #[test]
    fn test_yield_shocks_recover_and_dont_clear_land() {
        let mut state = State::default();
        let wheat = state.world.processes.iter().find(|p| p.name == "Industrial Crop Ag").unwrap().id;
        state.step_year(state.world.temperature);
        let before = state.world.processes[&wheat].output_modifier;

        Effect::YieldShock(Output::PlantCalories, -0.12, 4).apply(&mut state, None);
        let mut cleared = 0.;
        for _ in 0..4 {
            state.step_year(state.world.temperature);
            cleared += state.land_carbon.last.clearing;
        }
        // Recovered (plus a few years of background yield growth).
        assert!(state.world.processes[&wheat].output_modifier >= before);
        assert!(state.yield_shocks.is_empty());
        // Background clearing only: well under what clearing for the shock would release.
        assert!(cleared * 1e-15 < 10., "{}", cleared * 1e-15);
    }

    #[test]
    fn test_unrest_can_happen_in_every_region() {
        use crate::events::Phase;
        let mut state = State::default();
        state.world.year = 2030;
        let protest = state.event_pool.events.iter().find(|e| e.name == "Unrest - Protest").unwrap().id;
        let ids: Vec<_> = state.world.regions.iter().take(3).map(|r| r.id).collect();
        for id in &ids {
            state.world.regions[id].outlook = 0.;
        }
        let mut pool = state.event_pool.clone();
        let fired: Vec<_> = pool
            .roll_for_phase(Phase::WorldMain, &state)
            .into_iter()
            .filter(|(ev, _)| ev.id == protest)
            .collect();
        assert!(fired.len() >= 3, "protests fired in {} regions", fired.len());
        // ...but not twice in the same region.
        let again = pool
            .roll_for_phase(Phase::WorldMain, &state)
            .into_iter()
            .filter(|(ev, r)| ev.id == protest && r.is_some_and(|r| ids.contains(&r)))
            .count();
        assert_eq!(again, 0);
    }

    #[test]
    fn test_storage_caps_wind_and_solar() {
        let mut state = State::default();
        state.step_year(state.world.temperature);
        let solar = state.world.processes.iter().find(|p| p.name == "Solar PV").unwrap().id;
        // At the start there's room to grow well past today's share.
        let supported = state.supported_intermittent_share();
        assert!(supported > state.intermittent_share() + 0.2, "{supported}");
        let open_max = state.process_max_share(&solar);
        assert!(open_max >= 6, "{open_max}");

        // With no batteries and no mining, wind and solar can't grow past
        // what needs no storage.
        state.storage.lithium_in_use = 0.;
        state.storage.mining_capacity = 0.;
        let capped = state.process_max_share(&solar);
        assert!(capped < open_max, "{capped} vs {open_max}");
        assert!(capped as f32 / 20. <= 0.2 + 0.05);
    }

    #[test]
    fn test_termination_shock() {
        let mut state = State::default();
        let temp_prev = state.world.temperature;
        let effects = {
            let project = state
                .world
                .projects
                .iter_mut()
                .find(|p| p.name.contains("Solar Radiation Management"))
                .unwrap();

            project.points = 100;
            project.status = Status::Building;
            for i in 0..40 {
                project.advance(state.world.year + i);
            }
            assert_eq!(project.status, Status::Active);

            project.active_effects().clone()
        };

        state.apply_effects(&effects, None);
        state.world.update_climate(temp_prev);

        let temp_next = state.world.temperature;
        assert!(temp_next < temp_prev);

        let effect = Effect::TerminationShock;
        state.apply_effects(&[effect.clone()], None);
        state.world.update_climate(temp_prev);
        assert_eq!(state.world.temperature, temp_prev);

        // Unapplying should return back to prev temp.
        effect.unapply(&mut state, None);
        state.world.update_climate(temp_prev);
        assert_eq!(state.world.temperature, temp_next);
    }

    #[test]
    fn test_output_demand_amount() {
        let mut state = State::default();
        state.output_demand.base.plant_calories = 5.;
        let effect = Effect::DemandAmount(Output::PlantCalories, 1.);
        state.apply_effects(&[effect], None);
        assert_eq!(state.output_demand.of(Output::PlantCalories), 6.);
    }

    #[test]
    fn test_scaled_resource_amount_stays_an_amount() {
        let id = Id::new_v4();
        let effect = Effect::ModifyIndustryResourcesAmount(id, Resource::Electricity, 10.) * 0.5;
        assert_eq!(effect, Effect::ModifyIndustryResourcesAmount(id, Resource::Electricity, 5.));
    }

    #[test]
    fn test_freeze_caps_capacity_not_output() {
        // Freezing then cutting yields lowers output instead of using
        // more land, whichever order they happen in.
        for freeze_first in [true, false] {
            let mut state = State::default();
            state.step_year(state.world.temperature);
            let id = state
                .world
                .processes
                .iter()
                .find(|p| p.name == "Industrial Livestock Ag")
                .unwrap()
                .id;
            let before = state.produced.by_process[&id];
            let freeze = Effect::FreezeProcess(id);
            let cut = Effect::OutputForProcess(id, -0.2);
            if freeze_first {
                freeze.apply(&mut state, None);
                cut.apply(&mut state, None);
            } else {
                cut.apply(&mut state, None);
                freeze.apply(&mut state, None);
            }
            let limit = state.world.processes[&id].effective_limit().unwrap();
            assert!((limit / before - 0.8).abs() < 0.01, "{freeze_first}: {}", limit / before);
        }
    }

    #[test]
    fn test_outlook_by_income_round_trips() {
        let mut state = State::default();
        let before: Vec<f32> = state.world.regions.iter().map(|r| r.outlook).collect();
        let effect = Effect::OutlookByIncome([-2., -1.5, -0.5, 0.5]);
        effect.apply(&mut state, None);
        for (region, b) in state.world.regions.iter().zip(&before) {
            let expected = [-2., -1.5, -0.5, 0.5][region.income.level()];
            assert_eq!(region.outlook - b, expected);
        }
        effect.unapply(&mut state, None);
        for (region, b) in state.world.regions.iter().zip(&before) {
            assert_eq!(region.outlook, *b);
        }
    }

    #[test]
    fn test_war_footing_fatigue_felt_everywhere() {
        let mut state = State::default();
        let id = state.world.projects.iter().find(|p| p.name == "Climate War Footing").unwrap().id;
        state.world.projects[&id].status = Status::Active;
        for _ in 0..10 {
            state.step_year(state.world.temperature);
        }
        let poor = state.world.regions.iter().find(|r| r.income.level() == 0).unwrap().id;
        assert!(state.fatigue_applied[&poor] > 0.5);
    }
}
