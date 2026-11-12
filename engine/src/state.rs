use std::collections::BTreeMap;

use crate::{
    Collection, Id,
    events::{Condition, Effect, Event, EventPool, Flag, Phase, Request},
    kinds::*,
    land::LandCarbon,
    materials::Infrastructure,
    storage::{Storage, StorageTech},
    npcs::NPC,
    outputs,
    production::{ProcessChanges, ProcessFeature, calculate_required, produce},
    projects::{Group, Outcome, Project, ProjectChanges, Status, Type as ProjectType},
    resources,
    world::World,
};
use serde::{Deserialize, Serialize};

const LIFESPAN: usize = 60;
const PRODUCTION_SHORTAGE_PENALTY: f32 = 60.;

/// Extra contentedness penalty per share of electricity demand unmet.
const BLACKOUT_PENALTY: f32 = 50.;

/// Background farm yield growth, approaching a maximum with a time
/// constant in years. Crops: up to +60% (~1%/yr at first). Livestock
/// output per hectare has historically grown faster through feed
/// efficiency and intensification: up to +100% (~1.8%/yr at first).
const CROP_YIELD_GROWTH: (f32, f32) = (0.6, 60.);
const LIVESTOCK_YIELD_GROWTH: (f32, f32) = (1.0, 55.);

/// Have to all be below these values to win
const WIN_EMISSIONS: f32 = 0.0;
const WIN_EXTINCTION: f32 = 20.0;
const WIN_TEMPERATURE: f32 = 1.0;

/// A temporary yield shock that is recovering.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct YieldShock {
    pub output: Output,
    /// Output modifier still to be recovered (positive).
    pub remaining: f32,
    /// Recovered per year.
    pub per_year: f32,
}

/// Key values for one year, for history charts.
/// Emissions are in Gt CO2-equivalent; land use is a fraction of all land.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
pub struct YearRecord {
    pub year: usize,
    pub temperature: f32,
    pub emissions: f32,
    pub co2: f32,
    pub land: f32,
    pub ch4: f32,
    pub n2o: f32,
    pub extinction_rate: f32,
    pub land_use: f32,
    pub outlook: f32,
}

/// Represents the game state.
/// Fuel energy a combustion vehicle uses per unit of electricity an
/// electric one would.
const FUEL_PER_EV_ELECTRICITY: f32 = 4.;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct State {
    pub world: World,
    pub runs: usize,

    pub game_over: bool,
    pub death_year: usize,

    pub political_capital: isize,
    pub research_points: isize,
    pub npcs: Collection<NPC>,

    // Requests: (
    //  request type,
    //  entity id,
    //  state (active: true/false),
    //  political capital bounty
    // )
    pub requests: Vec<(Request, Id, bool, usize)>,
    pub flags: Vec<Flag>,

    // Keep track of what policies
    // need to have rolled outcomes
    pub policy_queue: Vec<Id>,

    pub produced: Production,
    pub resources: Resources,
    pub feedstocks: Feedstocks,
    pub output_demand: OutputDemand,
    pub resource_demand: ResourceDemand,

    /// Note that the biodiversity values here
    /// should be ignored, as we do the proper
    /// scaling in `Process::extinction_rate`.
    pub byproducts: Byproducts,

    pub protected_land: f32,

    pub shortages_outlook: f32,
    pub emissions: Emissions,
    pub last_outlook: f32,

    /// Carbon from clearing and regrowing land.
    #[serde(default)]
    pub land_carbon: LandCarbon,

    /// Grid storage and lithium.
    #[serde(default)]
    pub storage: Storage,

    /// A snapshot of key values for every year so far, for charts.
    #[serde(default)]
    pub history: Vec<YearRecord>,

    /// Energy infrastructure built, and the materials it took.
    #[serde(default)]
    pub infrastructure: Infrastructure,

    /// Years each fatigue-causing policy has been in force.
    #[serde(default)]
    pub policy_years: BTreeMap<Id, f32>,

    /// Policy fatigue currently applied to each region's contentedness.
    #[serde(default)]
    pub fatigue_applied: BTreeMap<Id, f32>,

    /// Policy fatigue per income level this year, for display.
    #[serde(default)]
    pub policy_fatigue: f32,

    /// Temporary yield shocks still recovering.
    #[serde(default)]
    pub yield_shocks: Vec<YieldShock>,

    /// Years of background farm yield growth applied so far.
    #[serde(default)]
    pub yield_years: f32,

    pub events: Vec<Event>,

    pub event_pool: EventPool,
}

impl Default for State {
    fn default() -> Self {
        Self::new(World::default())
    }
}

impl State {
    pub fn new(mut world: World) -> State {
        let mut npcs = NPC::load();
        let n_npcs = npcs.iter().filter(|npc| !npc.locked).count() as f32;
        for npc in npcs.iter_mut() {
            if !npc.locked {
                npc.seats = 1. / n_npcs;
            }
        }

        // Ensure that all projects have at least one outcome.
        for project in world.projects.iter_mut() {
            if project.outcomes.is_empty() {
                project.outcomes.push(Outcome::default());
            }
        }

        let events = world.events.clone();
        let death_year = world.year + LIFESPAN;

        let resources = Reserve::from(world.starting_resources);
        let feedstocks = Reserve::from(world.feedstock_reserves);

        let mut state = State {
            npcs,
            world,
            // Upstream starts at 100; this fork's early game is harder
            // (land-use emissions, effects that now apply), so start higher.
            political_capital: 150,
            research_points: 0,
            death_year,
            resources,
            feedstocks,

            protected_land: 0.1, // Starts at 10%

            events: vec![],
            event_pool: EventPool::new(events),

            runs: 0,
            game_over: false,

            last_outlook: 0.,
            shortages_outlook: 0.,
            emissions: Emissions::default(),
            land_carbon: LandCarbon::default(),
            yield_years: 0.,
            yield_shocks: vec![],
            policy_years: BTreeMap::new(),
            fatigue_applied: BTreeMap::new(),
            policy_fatigue: 0.,
            infrastructure: Infrastructure::default(),
            storage: Storage::default(),
            history: vec![],
            produced: Production::default(),
            output_demand: OutputDemand::default(),
            resource_demand: ResourceDemand::default(),
            byproducts: Byproducts::default(),

            flags: vec![],
            requests: vec![],
            policy_queue: vec![],
        };
        state.initialize();
        state
    }

    fn initialize(&mut self) {
        self.last_outlook = self.outlook();
        self.update_demand();
        self.step_production();
        self.update_project_costs();
        self.world.update_climate(self.world.temperature);
        self.record_history();
    }

    fn record_history(&mut self) {
        let gt = |g: f32| g * 1e-15;
        let record = YearRecord {
            year: self.world.year,
            temperature: self.world.temperature,
            emissions: self.emissions.as_gtco2eq(),
            co2: gt(self.emissions.co2),
            land: gt(self.emissions.land),
            ch4: gt(self.emissions.ch4 * 36.),
            n2o: gt(self.emissions.n2o * 298.),
            extinction_rate: self.world.extinction_rate,
            land_use: self.resource_demand.of(Resource::Land) / self.world.starting_resources.land,
            outlook: self.outlook(),
        };
        match self.history.last_mut() {
            Some(last) if last.year == record.year => *last = record,
            _ => self.history.push(record),
        }
    }

    /// If we won the game.
    pub fn won(&self) -> bool {
        self.emissions.as_gtco2eq() <= WIN_EMISSIONS
            && self.world.extinction_rate <= WIN_EXTINCTION
            && self.world.temperature <= WIN_TEMPERATURE
            && !self.flags.contains(&Flag::LifeGoesOn)
    }

    pub fn things_are_good(&self) -> bool {
        self.world.temperature <= 1.
            || self.world.extinction_rate <= 20.
            || self.emissions.as_gtco2eq() <= 0.
    }

    pub fn apply_disaster(&mut self, intensity: isize, region_id: &Id) {
        self.world.regions[region_id].base_habitability -= intensity as f32;
    }

    pub fn outlook(&self) -> f32 {
        self.world.outlook() - self.shortages_outlook
    }

    pub fn change_political_capital(&mut self, amount: isize) {
        self.political_capital += amount;
    }

    pub fn collect_research_points(&mut self) -> isize {
        let points = self.research_points;
        self.research_points = 0;
        points
    }

    pub fn step_year(&mut self, tgav: f32) -> Vec<Update> {
        let mut updates = vec![];
        let changes = self.step_projects();
        for (id, changes) in changes {
            if changes.completed {
                updates.push(Update::Project { id });
            }
            self.apply_changes(changes);
        }

        self.step_yields();
        self.update_demand();
        self.step_production();

        updates.extend(self.step_world(tgav));
        self.world.year += 1;
        self.record_history();

        // We actually apply policy changes the year
        // before so that any impacts they have are
        // accounted in the production phase of the last
        // year.
        if self.is_pre_planning_year() {
            let mut outcomes = self.roll_new_policy_outcomes();
            updates.append(&mut outcomes);
        }

        updates
    }

    /// Background improvement in farm yields from better seeds, feed,
    /// practices and so on, tapering off over time. Without it, growing
    /// food demand would need far more new farmland each year than the
    /// world has actually been clearing.
    /// Field farms (not vertical farms or cellular meat) producing `output`.
    fn farms_mut(&mut self, output: Output) -> impl Iterator<Item = &mut crate::Process> {
        self.world
            .processes
            .iter_mut()
            .filter(move |p| p.output == output && p.feedstock.0 == Feedstock::Soil)
    }

    pub fn apply_yield_shock(&mut self, output: Output, change: f32, years: usize) {
        for process in self.farms_mut(output) {
            process.output_modifier += change;
        }
        if change < 0. && years > 0 {
            self.yield_shocks.push(YieldShock {
                output,
                remaining: -change,
                per_year: -change / years as f32,
            });
        }
    }

    fn step_yields(&mut self) {
        // Temporary shocks recover.
        let mut shocks = std::mem::take(&mut self.yield_shocks);
        for shock in shocks.iter_mut() {
            let recover = shock.per_year.min(shock.remaining);
            for process in self.farms_mut(shock.output) {
                process.output_modifier += recover;
            }
            shock.remaining -= recover;
        }
        shocks.retain(|s| s.remaining > 1e-6);
        self.yield_shocks = shocks;

        let growth = |(max, time): (f32, f32)| {
            let at = |years: f32| max * (1. - (-years / time).exp());
            at(self.yield_years + 1.) - at(self.yield_years)
        };
        let (crops, livestock) = (growth(CROP_YIELD_GROWTH), growth(LIVESTOCK_YIELD_GROWTH));
        self.yield_years += 1.;
        for process in self.world.processes.iter_mut() {
            if process.feedstock.0 != Feedstock::Soil {
                continue;
            }
            match process.output {
                Output::PlantCalories => process.output_modifier += crops,
                Output::AnimalCalories => process.output_modifier += livestock,
                _ => {}
            }
        }
    }

    pub fn is_planning_year(&self) -> bool {
        self.world.year.is_multiple_of(5)
    }

    pub fn is_pre_planning_year(&self) -> bool {
        (self.world.year + 1).is_multiple_of(5)
    }

    pub fn apply_effects(&mut self, effects: &[Effect], region_id: Option<Id>) {
        for effect in effects {
            effect.apply(self, region_id);
        }
    }

    pub fn apply_event(&mut self, event_id: Id, region_id: Option<Id>) {
        let mut effects = vec![];
        let event = &self.event_pool.events[&event_id];
        self.events.push(event.clone());

        for effect in &event.effects {
            effects.push((effect.clone(), region_id));
        }

        for (effect, region_id) in effects {
            effect.apply(self, region_id);
        }
    }

    pub fn eval_conditions(&self, conditions: &[Condition], region_id: Option<Id>) -> bool {
        if conditions.is_empty() {
            true
        } else {
            conditions.iter().all(|c| c.eval(self, region_id))
        }
    }

    fn apply_changes<C: Changes>(&mut self, changes: C) {
        changes.apply(self);
        self.update_demand();
    }

    /// Recompute base demands from scratch.
    /// NOTE: This is the *only* place the base
    /// demands should be updated. If base demand
    /// needs to be influenced elsewhere then use
    /// `modifiable.modifier` or `modifiable.factor`
    /// instead.
    fn update_demand(&mut self) {
        let (output_demand, mut resource_demand, industry_byproducts) = {
            let world = &self.world;
            let mut output_demand = outputs!();
            let mut resource_demand = resources!();

            // Ignore electric/fuel, captured by everything else
            let world_demand = world.region_demand();
            output_demand.animal_calories += world_demand.animal_calories;
            output_demand.plant_calories += world_demand.plant_calories;

            // Demand and impacts from non-modeled industries
            let lic_pop = world.lic_population();
            let industry_demand = world.industries.resource_demand(lic_pop);
            let industry_byproducts = world.industries.byproducts(lic_pop);

            output_demand.fuel = industry_demand.fuel;
            output_demand.electricity = industry_demand.electricity;

            // Vehicles that should be electric but couldn't get batteries
            // keep running on fuel (about four times the energy, since
            // engines waste most of it as heat).
            let ev_shortfall = self.storage.last.ev_shortfall;
            if ev_shortfall > 0. {
                let ev_electricity = world
                    .industries
                    .iter()
                    .find(|i| i.name == "Road Transport")
                    .map(|i| i.total_demand_for_resource(lic_pop, Resource::Electricity))
                    .unwrap_or(0.);
                let stranded = ev_electricity * ev_shortfall;
                output_demand.electricity -= stranded;
                output_demand.fuel += stranded * FUEL_PER_EV_ELECTRICITY;
            }

            // Water and land demand from industries.
            // Process resource demand will be added later.
            resource_demand.water = industry_demand.water;
            resource_demand.land = industry_demand.land;

            // Electrification is only relevant for non-modeled industry;
            // for processes we always rely on their actual values.
            if self.flags.contains(&Flag::Electrified) {
                let electrified = output_demand.fuel * 0.8;
                output_demand.electricity += electrified;
                output_demand.fuel -= electrified;
            }

            // For vegan-ish diets, move some animal calorie
            // demand to plants.
            let cal_change = if self.flags.contains(&Flag::Vegan) {
                output_demand.animal_calories * 0.9
            } else if self.flags.contains(&Flag::Vegetarian) {
                output_demand.animal_calories * 0.75
            } else {
                0.
            };
            output_demand.animal_calories -= cal_change;
            output_demand.plant_calories += cal_change;

            (output_demand, resource_demand, industry_byproducts)
        };
        self.output_demand.base = output_demand;

        // Generate production orders based on current process mixes and demand
        let total_demand = self.output_demand.total();
        let orders = self.world.processes.orders(&total_demand);

        // Calculate required resources so we can add in food energy requirements
        let (required_resources, required_feedstocks) = calculate_required(&orders);

        self.output_demand.base.electricity += required_resources.electricity;
        self.output_demand.base.fuel += required_resources.fuel;

        // Storage round-trip losses, and power not supplied during
        // scheduled blackouts.
        let electricity = self.output_demand.total().electricity;
        self.output_demand.base.electricity += self.storage.extra_demand(
            electricity,
            self.intermittent_share(),
            self.storage_tech(),
        );
        self.output_demand.base.electricity -= crate::storage::scheduled_cut(
            electricity,
            self.intermittent_share(),
            self.storage_tech(),
        );

        // Now re-calculate orders
        let total_demand = self.output_demand.total();
        let orders = self.world.processes.orders(&total_demand);

        // Apply land protection
        self.resources.available.land =
            self.world.starting_resources.land * (1. - self.protected_land);

        // Run production function
        let (
            produced_by_process,
            produced_by_type,
            consumed_resources,
            consumed_feedstocks,
            production_byproducts,
        ) = produce(
            &orders,
            (&required_resources, &required_feedstocks),
            &self.resources.available,
            &self.feedstocks.available,
        );

        self.produced.by_process = produced_by_process;
        for process in self.world.processes.iter_mut() {
            process.produced_modifier = process.output_modifier;
        }
        self.produced.amount = produced_by_type;

        // Without enough storage, some wind and solar power can't be
        // delivered when it's needed: a real electricity shortage.
        let unserved = self
            .storage
            .unserved(self.produced.amount.electricity, self.intermittent_share(), self.storage_tech());
        self.produced.amount.electricity = (self.produced.amount.electricity - unserved).max(0.);

        resource_demand.water += required_resources.water;
        resource_demand.land += required_resources.land;
        resource_demand.fuel = output_demand.fuel;
        resource_demand.electricity = output_demand.electricity;
        self.resource_demand.base = resource_demand;
        self.resources.consumed = consumed_resources;
        self.resources.required = required_resources;
        self.resources.available.fuel = self.produced.amount.fuel;
        self.resources.available.electricity = self.produced.amount.electricity;

        self.feedstocks.consumed = consumed_feedstocks;
        // Lithium for batteries (grid storage and vehicles).
        self.feedstocks.consumed[Feedstock::Lithium] += self.storage.last.lithium_mined;
        self.feedstocks.required = required_feedstocks;

        self.byproducts.base = production_byproducts + industry_byproducts;
        self.emissions.update(self.byproducts.total());
    }

    fn step_production(&mut self) {
        self.feedstocks.consume(self.feedstocks.consumed);

        // Water and land aren't "consumed" as land
        // can obviously be re-purposed and we assume water
        // is more or less renewable.
        // Then we subtract out fuel and electricity usage
        // that's compensated by production.
        // However, we don't actually need to consume resources because
        // `self.resources.available` actually represents resources
        // available for the *next* year of production.
        // self.resources.consume(consumed_resources);

        // Outlook impacts based on production shortages
        // If all demand met is 0 it should be an instant game over, basically.
        let demand_met = self.produced.total() / self.output_demand.total();
        self.shortages_outlook = (PRODUCTION_SHORTAGE_PENALTY
            - ((demand_met.fuel
                + demand_met.electricity
                + demand_met.animal_calories
                + demand_met.plant_calories)
                * PRODUCTION_SHORTAGE_PENALTY
                / 4.))
            .max(0.)
            // Blackouts hit everything, so electricity shortages count extra.
            + BLACKOUT_PENALTY * (1. - demand_met.electricity).clamp(0., 1.);

        self.world.update_extinction_rate(&self.produced.by_process);

        // Land-use change emissions, once per year. Extra land used only
        // because of a temporary yield shock is covered by fallow land and
        // heavier use of existing farmland, not by clearing, so it's left out.
        let shock_land: f32 = self
            .world
            .processes
            .iter()
            .filter(|p| p.feedstock.0 == Feedstock::Soil)
            .map(|p| {
                let remaining: f32 = self
                    .yield_shocks
                    .iter()
                    .filter(|s| s.output == p.output)
                    .map(|s| s.remaining)
                    .sum();
                if remaining <= 0. {
                    return 0.;
                }
                let produced = self.produced.by_process.get(&p.id).copied().unwrap_or(0.);
                let m = 1. + p.output_modifier;
                produced * p.resources.land * (1. / m.max(1e-3) - 1. / (m + remaining))
            })
            .sum();
        let land_emissions = self.land_carbon.step(
            self.resources.consumed.land - shock_land.max(0.),
            self.world.starting_resources.land,
            self.protected_land,
            self.world.temperature,
        );
        self.emissions.land = land_emissions.total();

        // Materials for energy infrastructure built this year; these feed
        // into industry demand next year.
        let lic_pop = self.world.lic_population();
        self.infrastructure.step(
            &self.world.processes,
            &self.produced.by_process,
            &mut self.world.industries,
            lic_pop,
        );

        // Batteries for grid storage and electric vehicles.
        let ev_electricity = self
            .world
            .industries
            .iter()
            .find(|i| i.name == "Road Transport")
            .map(|i| i.total_demand_for_resource(lic_pop, Resource::Electricity))
            .unwrap_or(0.);
        let lithium_left = self.feedstocks.available[Feedstock::Lithium];
        self.storage.step(
            self.produced.amount.electricity,
            self.intermittent_share(),
            ev_electricity,
            self.storage_tech(),
            lithium_left,
        );
    }

    /// Share of electricity planned from intermittent sources (wind, solar).
    pub fn intermittent_share(&self) -> f32 {
        self.world
            .processes
            .iter()
            .filter(|p| p.output == Output::Electricity && p.features.contains(&ProcessFeature::IsIntermittent))
            .map(|p| p.mix_percent())
            .sum::<f32>()
            .min(1.)
    }

    /// Apply ongoing discontent from policies in force, as a level that
    /// changes over time rather than a penalty that piles up each year.
    fn update_policy_fatigue(&mut self) {
        let mut total = 0.;
        let mut active = BTreeMap::new();
        for project in self.world.projects.iter() {
            if let Some(fatigue) = project.fatigue {
                if project.status == Status::Active {
                    let years = self.policy_years.get(&project.id).copied().unwrap_or(0.) + 1.;
                    total += fatigue.level(years);
                    active.insert(project.id, years);
                }
            }
        }
        self.policy_years = active;
        self.policy_fatigue = total;
        for region in self.world.regions.iter_mut() {
            let target = total * region.income.level() as f32;
            let applied = self.fatigue_applied.get(&region.id).copied().unwrap_or(0.);
            region.outlook -= target - applied;
            self.fatigue_applied.insert(region.id, target);
        }
    }

    /// The highest wind and solar share storage can support this planning cycle.
    pub fn supported_intermittent_share(&self) -> f32 {
        self.storage
            .supported_share(self.output_demand.of(Output::Electricity), self.storage_tech(), 5.)
    }

    pub fn storage_tech(&self) -> StorageTech {
        StorageTech {
            sodium_ion: self.flags.contains(&Flag::SodiumIonBatteries),
            pumped_hydro: self.flags.contains(&Flag::PumpedHydroStorage),
            long_duration: self.flags.contains(&Flag::LongDurationStorage),
            scheduled_blackouts: self.flags.contains(&Flag::ScheduledBlackouts),
        }
    }

    fn step_world(&mut self, tgav: f32) -> Vec<Update> {
        if self.world.year >= self.death_year && !self.flags.contains(&Flag::LifeGoesOn) {
            self.game_over = true;
        }

        self.world.update_populations();
        let temp_change = self.world.update_climate(tgav);

        let stop = self.flags.contains(&Flag::StopDevelopment);
        let fast = self.flags.contains(&Flag::FastDevelopment);
        let degrow = self.flags.contains(&Flag::Degrowth);
        let (regions_up, regions_down) = self.world.regions.develop(stop, fast, degrow);

        let wretched_ally = self.npcs.is_ally("The Fanonist");
        let consumerist_ally = self.npcs.is_ally("The Consumerist");
        self.world
            .update_outlook(temp_change, wretched_ally, consumerist_ally);
        self.update_policy_fatigue();
        regions_up
            .into_iter()
            .map(|id| Update::Region { id, up: true })
            .chain(
                regions_down
                    .into_iter()
                    .map(|id| Update::Region { id, up: false }),
            )
            .collect()
    }

    // Every planning cycle
    pub fn finish_cycle(&mut self) {
        let outlook_change = self.outlook() - self.last_outlook;
        let recent_projects: Vec<&Project> = self.world.projects.recent(self.world.year).collect();
        self.npcs.update_seats(outlook_change, &recent_projects);
        self.last_outlook = self.outlook();
    }

    pub fn check_requests(&mut self) -> Vec<(Request, Id, bool, usize)> {
        let mut i = 0;
        let mut completed = Vec::new();
        while i < self.requests.len() {
            let (kind, id, active, bounty) = self.requests[i].clone();
            let complete = match kind {
                Request::Project => {
                    let project = &self.world.projects[&id];
                    (active
                        && (project.status == Status::Active || project.status == Status::Finished))
                        || (!active
                            && (project.status == Status::Inactive
                                || project.status == Status::Halted))
                }
                Request::Process => {
                    let process = &self.world.processes[&id];
                    (active && process.is_promoted()) || (!active && process.is_banned())
                }
            };
            if complete {
                self.requests.remove(i);
                completed.push((kind, id, active, bounty));
            } else {
                i += 1;
            }
        }
        completed
    }

    pub fn change_process_mix_share(&mut self, process_id: &Id, change: isize) {
        let changes = self.world.processes[process_id].change_mix_share(change);
        self.apply_changes(changes);
    }

    pub fn process_max_share(&self, process_id: &Id) -> usize {
        self.process_max_share_planned(process_id, None, None)
    }

    /// The most shares of electricity wind and solar together can have.
    pub fn intermittent_cap_shares(&self) -> usize {
        (self.supported_intermittent_share() * 20.).floor() as usize
    }

    /// The wind and solar cap (in shares) for a planned electricity demand.
    pub fn intermittent_cap_shares_for(&self, electricity: f32) -> usize {
        (self.storage_outlook(0., electricity).supported_share * 20.).floor() as usize
    }

    /// Storage and lithium needs for a planned wind and solar share and
    /// electricity demand over the next planning cycle.
    pub fn storage_outlook(&self, intermittent_share: f32, electricity: f32) -> crate::storage::StorageOutlook {
        self.storage.outlook(
            electricity,
            intermittent_share,
            self.storage.last.ev_lithium_needed,
            self.storage_tech(),
            5.,
        )
    }

    /// Whether a process is limited by the shared wind and solar cap.
    pub fn is_intermittent_electricity(&self, process_id: &Id) -> bool {
        let process = &self.world.processes[process_id];
        process.output == Output::Electricity
            && process.features.contains(&ProcessFeature::IsIntermittent)
    }

    /// Like `process_max_share`, but `planned_others` gives the planned
    /// shares of the other wind and solar processes (including changes not
    /// yet applied), so the shared cap reflects the plan.
    pub fn process_max_share_planned(
        &self,
        process_id: &Id,
        planned_others: Option<usize>,
        planned_electricity: Option<f32>,
    ) -> usize {
        let output_demand = self.output_demand.total();
        let feedstocks = self.feedstocks.available;
        let process = &self.world.processes[process_id];
        let max_share = process.max_share(&output_demand, &feedstocks);

        // Wind and solar can only grow as far as storage can support them.
        let max_share = if self.is_intermittent_electricity(process_id) {
            let others = planned_others.unwrap_or_else(|| {
                ((self.intermittent_share() * 20.).round() as usize).saturating_sub(process.mix_share)
            });
            let supported = match planned_electricity {
                Some(electricity) => self.intermittent_cap_shares_for(electricity),
                None => self.intermittent_cap_shares(),
            };
            max_share.min(supported.saturating_sub(others).max(process.mix_share))
        } else {
            max_share
        };

        // Under a fossil fuel moratorium, fossil processes can shrink but not grow.
        if self.flags.contains(&Flag::FossilMoratorium)
            && process.features.contains(&ProcessFeature::IsFossil)
        {
            max_share.min(process.mix_share)
        } else {
            max_share
        }
    }

    pub fn roll_events(&mut self, phase: Phase) -> Vec<ResolvedEvent> {
        let mut pool = self.event_pool.clone();
        let events = pool.roll_for_phase(phase, self);
        self.event_pool = pool;

        let events: Vec<ResolvedEvent> = events
            .into_iter()
            .map(|(ev, region_id)| ResolvedEvent {
                event: ev,
                region: region_id.map(|id| (id, self.world.regions[&id].name.to_string())),
            })
            .collect();

        // Icon events, aka disasters,
        // are handled differently, so we don't
        // apply their effects immediately here.
        if phase != Phase::Icon {
            for ev in &events {
                self.apply_event(ev.id, ev.region.as_ref().map(|(id, _)| *id));
            }
        }

        events
    }
}

// Project related functionality.
impl State {
    fn step_projects(&mut self) -> Vec<(Id, ProjectChanges)> {
        let mut changes = self.world.projects.step(self.world.year);

        let mut outcomes: Vec<(Id, usize)> = Vec::new();
        for (id, changes) in &mut changes {
            if changes.completed {
                let project = &self.world.projects[id];
                if let Some((outcome, i)) = self.roll_project_outcome(project) {
                    for effect in &outcome.effects {
                        changes.add_effects.push(effect.clone());
                    }
                    outcomes.push((*id, i));
                }
            }
        }

        for (id, i) in outcomes {
            self.world.projects[&id].active_outcome = Some(i);
        }

        self.update_project_costs();

        changes
    }

    fn update_project_costs(&mut self) {
        let base_modifier = self.base_project_cost_modifier();
        let total_demand = self.output_demand.total();
        let income_level = self.world.regions.income_level();

        let posadist_ally = self.npcs.is_ally("The Posadist");
        let utopian_ally = self.npcs.is_ally("The Utopian");
        let animal_ally = self.npcs.is_ally("The Animal Liberationist");
        let environ_ally = self.npcs.is_ally("The Environmentalist");
        let ecofem_ally = self.npcs.is_ally("The Ecofeminist");
        let malthus_ally = self.npcs.is_ally("The Malthusian");

        for project in self.world.projects.iter_mut() {
            let mut group_modifier = 1.0;
            if posadist_ally && project.group == Group::Nuclear {
                group_modifier *= 0.5;
            }
            if ecofem_ally
                && matches!(
                    project.group,
                    Group::Food | Group::Agriculture | Group::Protection
                )
            {
                group_modifier *= 0.75;
            }
            if utopian_ally
                && matches!(
                    project.group,
                    Group::Limits | Group::Protection | Group::Restoration
                )
            {
                group_modifier *= 0.75;
            }
            if environ_ally && project.group == Group::Protection {
                group_modifier *= 0.5;
            }
            if animal_ally && project.group == Group::Food {
                group_modifier *= 0.5;
            }
            if malthus_ally && project.group == Group::Population {
                group_modifier *= 0.5;
            }
            if self.flags.contains(&Flag::EcosystemModeling) && project.group == Group::Restoration
            {
                group_modifier *= 1.1;
            }
            if self.flags.contains(&Flag::WarFooting)
                && matches!(
                    project.group,
                    Group::Energy | Group::Electrification | Group::Buildings
                )
            {
                group_modifier *= 0.8;
            }
            project.update_cost(
                self.world.year,
                income_level,
                &total_demand,
                // Modifier only relevant for built projects,
                // not policies.
                if project.kind == ProjectType::Policy {
                    1.0
                } else {
                    base_modifier * group_modifier
                },
            );
            project.update_required_majority(&self.npcs);
        }
    }

    fn base_project_cost_modifier(&self) -> f32 {
        let mut modifier = if self.flags.contains(&Flag::MetalsShortage)
            && !self.flags.contains(&Flag::DeepSeaMining)
        {
            0.8
        } else {
            1.
        };
        if self.flags.contains(&Flag::MoreLabor) {
            modifier *= 0.9;
        }
        if self.flags.contains(&Flag::MoreAutomation) {
            modifier *= 0.9;
        }
        if self.flags.contains(&Flag::MoreLeisure) {
            modifier *= 1.1;
        }
        if self.flags.contains(&Flag::LaborResistance) {
            modifier *= 1.05;
        }
        if self.flags.contains(&Flag::LaborSabotage) {
            modifier *= 1.05;
        }
        modifier
    }

    pub fn start_project(&mut self, project_id: &Id) {
        let is_policy = self.world.projects[project_id].start();
        if is_policy {
            self.policy_queue.push(*project_id);
        }
    }

    pub fn stop_project(&mut self, project_id: &Id) {
        let (changes, is_policy) = self.world.projects[project_id].stop();
        if is_policy {
            self.policy_queue.retain(|&id| id != *project_id);
        }
        self.apply_changes(changes);
    }

    pub fn upgrade_project(&mut self, project_id: &Id) {
        let changes = self.world.projects[project_id].upgrade();
        self.apply_changes(changes);
    }

    pub fn downgrade_project(&mut self, project_id: &Id) {
        let changes = self.world.projects[project_id].downgrade();
        self.apply_changes(changes);
    }

    pub fn set_project_points(&mut self, project_id: &Id, points: usize) {
        self.world.projects[project_id].set_points(points);
    }

    /// Roll to see the outcome of this project
    fn roll_project_outcome<'a>(&self, project: &'a Project) -> Option<(&'a Outcome, usize)> {
        let mut outcome = None;
        for (i, o) in project.outcomes.iter().enumerate() {
            if let Some(likelihood) = o.probability.eval(self, None) {
                let prob = likelihood.p();
                if fastrand::f32() <= prob {
                    outcome = Some((o, i));
                    break;
                }
            }
        }
        if outcome.is_none() {
            outcome = Some((&project.outcomes[0], 0));
        }
        outcome
    }

    fn roll_new_policy_outcomes(&mut self) -> Vec<Update> {
        let mut effects: Vec<Effect> = Vec::new();
        let ids: Vec<Id> = self.policy_queue.drain(..).collect();
        for id in &ids {
            let mut active_outcome = None;
            let proj = &self.world.projects[id];
            if let Some((outcome, i)) = self.roll_project_outcome(proj) {
                for effect in &outcome.effects {
                    effects.push(effect.clone());
                }
                active_outcome = Some(i);
            }
            let proj = &mut self.world.projects[id];
            proj.active_outcome = active_outcome;
            proj.status = Status::Active;
            for effect in &proj.effects {
                effects.push(effect.clone());
            }

            let mut changes = ProjectChanges::default();
            changes.add_supporters(&proj.supporters);
            changes.add_opposers(&proj.opposers);
            self.apply_changes(changes);
        }

        for effect in effects {
            effect.apply(self, None);
        }
        self.update_demand();

        ids.into_iter().map(|id| Update::Policy { id }).collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Production {
    pub amount: OutputMap,
    pub by_process: BTreeMap<Id, f32>,
}
impl Production {
    pub fn of(&self, output: Output) -> f32 {
        self.amount[output]
    }

    pub fn total(&self) -> OutputMap {
        self.amount
    }
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Emissions {
    pub co2: f32,
    pub ch4: f32,
    pub n2o: f32,

    /// Net CO2 from land-use change (clearing, regrowth, deforestation), g CO2.
    #[serde(default)]
    pub land: f32,
}
impl Emissions {
    pub fn update(&mut self, byproducts: ByproductMap) {
        self.co2 = byproducts.co2;
        self.ch4 = byproducts.ch4;
        self.n2o = byproducts.n2o;
    }

    pub fn as_co2eq(&self) -> f32 {
        self.co2 + self.land + (self.n2o * 298.) + (self.ch4 * 36.)
    }

    /// Land-use change emissions in the units Hector expects (Pg C/y).
    pub fn land_for_hector(&self) -> f32 {
        self.land * 12. / 44. * 1e-15
    }

    pub fn as_gtco2eq(&self) -> f32 {
        self.as_co2eq() * 1e-15
    }

    /// Convert to units expected by Hector:
    /// - CO2: Pg C/y
    /// - CH4: Tg/y
    /// - N2O: Tg/y
    ///
    /// Land-use change emissions are passed to Hector separately
    /// (see `land_for_hector`).
    ///
    /// Units: <https://github.com/JGCRI/hector/wiki/Hector-Units>
    pub fn for_hector(&self) -> (f32, f32, f32) {
        let co2 = self.co2 * 12. / 44. * 1e-15; // Pg C/y
        let ch4 = self.ch4 * 1e-12; // Tg/y
        let n2o = self.n2o * 1e-12; // Tg/y
        (co2, ch4, n2o)
    }
}

pub trait Changes {
    fn apply(self, state: &mut State);
}
impl Changes for ProcessChanges {
    fn apply(self, state: &mut State) {
        for (id, change) in self.relationships {
            state.npcs[&id].relationship += change;
        }
    }
}
impl Changes for ProjectChanges {
    fn apply(self, state: &mut State) {
        for effect in self.remove_effects {
            effect.unapply(state, None);
        }
        for effect in self.add_effects {
            effect.apply(state, None);
        }
        for (id, change) in self.relationships {
            state.npcs[&id].relationship += change;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedEvent {
    pub event: Event,
    pub region: Option<(Id, String)>,
}
impl std::ops::Deref for ResolvedEvent {
    type Target = Event;
    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Update {
    Region {
        id: Id,
        up: bool, // or down
    },
    Policy {
        id: Id,
    },
    Project {
        id: Id,
    },
}

impl Update {
    pub fn is_region(&self) -> bool {
        matches!(self, Update::Region { .. })
    }

    pub fn is_region_up(&self) -> bool {
        matches!(self, Update::Region { up: true, .. })
    }

    pub fn is_region_down(&self) -> bool {
        matches!(self, Update::Region { up: false, .. })
    }

    pub fn is_project(&self) -> bool {
        matches!(self, Update::Project { .. } | Update::Policy { .. })
    }

    pub fn is_policy(&self) -> bool {
        matches!(self, Update::Policy { .. })
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_promote_process() {
//         let mut state = State::new(Difficulty::Normal);
//         state.processes[0].mix_share = 0;
//         assert_eq!(state.processes[0].is_promoted(), false);
//
//         state.change_mix_share(0, 10);
//         assert_eq!(state.processes[0].is_promoted(), true);
//         for npc_id in &state.processes[0].supporters {
//             assert_eq!(
//                 state.npcs[*npc_id].relationship,
//                 3. + RELATIONSHIP_CHANGE_AMOUNT
//             );
//         }
//         for npc_id in &state.processes[0].opposers {
//             assert_eq!(
//                 state.npcs[*npc_id].relationship,
//                 3. - RELATIONSHIP_CHANGE_AMOUNT
//             );
//         }
//
//         state.change_mix_share(0, -8);
//         assert_eq!(state.processes[0].is_promoted(), false);
//         for npc_id in &state.processes[0].supporters {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//         for npc_id in &state.processes[0].opposers {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//     }
//
//     #[test]
//     fn test_ban_process() {
//         let mut state = State::new(Difficulty::Normal);
//         state.processes[0].mix_share = 5;
//         assert_eq!(state.processes[0].is_banned(), false);
//
//         state.change_mix_share(0, -5);
//         assert_eq!(state.processes[0].is_banned(), true);
//         for npc_id in &state.processes[0].supporters {
//             assert_eq!(
//                 state.npcs[*npc_id].relationship,
//                 3. - RELATIONSHIP_CHANGE_AMOUNT
//             );
//         }
//         for npc_id in &state.processes[0].opposers {
//             assert_eq!(
//                 state.npcs[*npc_id].relationship,
//                 3. + RELATIONSHIP_CHANGE_AMOUNT
//             );
//         }
//
//         state.change_mix_share(0, 2);
//         assert_eq!(state.processes[0].is_banned(), false);
//         for npc_id in &state.processes[0].supporters {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//         for npc_id in &state.processes[0].opposers {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//     }
//
//     #[test]
//     fn test_project_policy() {
//         let mut rng = SmallRng::from_entropy();
//         let mut state = State::new(Difficulty::Normal);
//
//         let id = {
//             let p = state
//                 .projects
//                 .iter_mut()
//                 .find(|p| {
//                     p.kind == ProjectType::Policy
//                         && p.effects.len() > 0
//                 })
//                 .unwrap();
//             p.set_points(10);
//             p.id
//         };
//
//         assert_eq!(state.projects[id].status, Status::Inactive);
//
//         // Start
//         state.start_project(id);
//         assert_eq!(state.projects[id].status, Status::Building);
//
//         // Build until the project is completed
//         let mut effects = vec![];
//         loop {
//             let (completed, _, effs) =
//                 state.step_projects(&mut rng);
//             if completed.contains(&id) {
//                 effects = effs
//                     .iter()
//                     .map(|(eff, _)| eff.clone())
//                     .collect();
//                 for npc_id in &state.projects[id].supporters {
//                     assert_eq!(
//                         state.npcs[*npc_id].relationship,
//                         3. + RELATIONSHIP_CHANGE_AMOUNT
//                     );
//                 }
//                 for npc_id in &state.projects[id].opposers {
//                     assert_eq!(
//                         state.npcs[*npc_id].relationship,
//                         3. - RELATIONSHIP_CHANGE_AMOUNT
//                     );
//                 }
//                 break;
//             }
//         }
//
//         // Stop
//         let uneffects = state.stop_project(id);
//         assert_eq!(state.projects[id].status, Status::Halted);
//         assert_eq!(effects.len(), uneffects.len());
//         for npc_id in &state.projects[id].supporters {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//         for npc_id in &state.projects[id].opposers {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//     }
//
//     #[test]
//     fn test_project_other() {
//         let mut rng = SmallRng::from_entropy();
//         let mut state = State::new(Difficulty::Normal);
//
//         let id = {
//             let p = state
//                 .projects
//                 .iter_mut()
//                 .find(|p| p.kind == ProjectType::Initiative)
//                 .unwrap();
//             p.set_points(10);
//             p.id
//         };
//
//         assert_eq!(state.projects[id].status, Status::Inactive);
//
//         // Start
//         state.start_project(id);
//         assert_eq!(state.projects[id].status, Status::Building);
//
//         // Build until the project is completed
//         let mut effects = vec![];
//         loop {
//             let (completed, _, effs) =
//                 state.step_projects(&mut rng);
//             if completed.contains(&id) {
//                 effects = effs
//                     .iter()
//                     .map(|(eff, _)| eff.clone())
//                     .collect();
//                 for npc_id in &state.projects[id].supporters {
//                     assert_eq!(
//                         state.npcs[*npc_id].relationship,
//                         3. + RELATIONSHIP_CHANGE_AMOUNT
//                     );
//                 }
//                 for npc_id in &state.projects[id].opposers {
//                     assert_eq!(
//                         state.npcs[*npc_id].relationship,
//                         3. - RELATIONSHIP_CHANGE_AMOUNT
//                     );
//                 }
//                 break;
//             }
//         }
//
//         // Stop
//         let uneffects = state.stop_project(id);
//         assert_eq!(state.projects[id].status, Status::Halted);
//         assert_eq!(effects.len(), uneffects.len());
//         for npc_id in &state.projects[id].supporters {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//         for npc_id in &state.projects[id].opposers {
//             assert_eq!(state.npcs[*npc_id].relationship, 3.);
//         }
//
//         // Start again
//         state.start_project(id);
//         assert_eq!(state.projects[id].status, Status::Building);
//
//         state.projects[id].set_points(10);
//         state.projects[id].build();
//
//         // Stop again, should be halted now
//         let uneffects = state.stop_project(id);
//         assert_eq!(state.projects[id].status, Status::Halted);
//         assert_eq!(effects.len(), uneffects.len());
//     }
//
//     #[test]
//     fn test_project_upgrades() {
//         let mut rng = SmallRng::from_entropy();
//         let mut state = State::new(Difficulty::Normal);
//
//         let id = {
//             let p = state
//                 .projects
//                 .iter_mut()
//                 .find(|p| {
//                     p.upgrades.len() > 0
//                         && p.kind == ProjectType::Policy
//                 })
//                 .unwrap();
//             p.set_points(10);
//             p.id
//         };
//
//         state.start_project(id);
//
//         let mut effects = vec![];
//         loop {
//             let (completed, _, effs) =
//                 state.step_projects(&mut rng);
//             if completed.contains(&id) {
//                 effects = effs
//                     .iter()
//                     .map(|(eff, _)| eff.clone())
//                     .collect();
//                 break;
//             }
//         }
//
//         let (uneffects, new_effects) =
//             state.upgrade_project(id);
//
//         assert_eq!(state.projects[id].level, 1);
//         assert!(effects.iter().eq(uneffects.iter()));
//         assert!(new_effects.len() > 0);
//     }
//
//     #[test]
//     fn test_project_no_upgrades() {
//         let mut rng = SmallRng::from_entropy();
//         let mut state = State::new(Difficulty::Normal);
//
//         let id = {
//             let p = state
//                 .projects
//                 .iter_mut()
//                 .find(|p| {
//                     p.upgrades.len() == 0
//                         && p.kind == ProjectType::Policy
//                 })
//                 .unwrap();
//             p.set_points(10);
//             p.id
//         };
//
//         state.start_project(id);
//
//         let mut effects = vec![];
//         loop {
//             let (completed, _, effs) =
//                 state.step_projects(&mut rng);
//             if completed.contains(&id) {
//                 effects = effs
//                     .iter()
//                     .map(|(eff, _)| eff.clone())
//                     .collect();
//                 break;
//             }
//         }
//
//         let (uneffects, new_effects) =
//             state.upgrade_project(id);
//
//         assert_eq!(state.projects[id].level, 0);
//         assert_eq!(uneffects.len(), 0);
//         assert_eq!(new_effects.len(), 0);
//     }
//
//     #[test]
//     fn test_requests() {
//         let mut rng = SmallRng::from_entropy();
//         let mut state = State::new(Difficulty::Normal);
//         state.requests.push((Request::Project, 0, true, 10));
//         state.requests.push((Request::Process, 0, true, 10));
//
//         let completed = state.check_requests();
//         assert_eq!(completed.len(), 0);
//
//         state.start_project(0);
//         let completed = state.check_requests();
//         assert_eq!(completed.len(), 0); // Project not yet finished
//
//         state.projects[0].set_points(10);
//         for _ in 0..100 {
//             // Should be plenty of time to finish any project
//             state.projects[0].build();
//         }
//
//         let completed = state.check_requests();
//         assert_eq!(completed.len(), 1);
//         assert_eq!(state.requests.len(), 1);
//
//         state.change_mix_share(0, 5);
//         let completed = state.check_requests();
//         assert_eq!(completed.len(), 1);
//         assert_eq!(state.requests.len(), 0);
//
//         state.requests.push((Request::Project, 0, false, 10));
//         state.requests.push((Request::Process, 0, false, 10));
//
//         state.stop_project(0);
//         state.change_mix_share(
//             0,
//             -(state.processes[0].mix_share as isize),
//         );
//         let completed = state.check_requests();
//         assert_eq!(completed.len(), 2);
//         assert_eq!(state.requests.len(), 0);
//     }
//
//     #[test]
//     fn test_calculate_demand() {
//         // This is a pretty simple test,
//         // real testing involves calibration
//         let mut state = State::new(Difficulty::Normal);
//
//         let (output_demand, resource_demand) =
//             state.calculate_demand();
//
//         state.output_demand_extras = outputs!(fuel: 100.);
//         let (other_output_demand, _) = state.calculate_demand();
//         assert_eq!(
//             output_demand.fuel + 100.,
//             other_output_demand.fuel
//         );
//
//         state.output_demand_modifier = outputs!(fuel: 2.);
//         let (other_output_demand, _) = state.calculate_demand();
//         assert_eq!(
//             2. * (output_demand.fuel + 100.),
//             other_output_demand.fuel
//         );
//     }
// }

#[cfg(test)]
mod fatigue_tests {
    use super::*;

    fn by_name(state: &State, name: &str) -> Id {
        state.world.projects.iter().find(|p| p.name == name).unwrap().id
    }

    #[test]
    fn rationing_fatigue_builds_and_repeal_restores() {
        let mut state = State::new(World::default());
        let id = by_name(&state, "Wartime Rationing");
        let outlooks =
            |s: &State| s.world.regions.iter().map(|r| r.outlook).collect::<Vec<_>>();
        let before = outlooks(&state);

        state.world.projects[&id].status = Status::Active;
        state.update_policy_fatigue();
        let one = state.policy_fatigue;
        for _ in 0..9 {
            state.update_policy_fatigue();
        }
        assert!(state.policy_fatigue > one * 5., "grinding fatigue builds up");
        let after = outlooks(&state);
        for ((region, b), a) in state.world.regions.iter().zip(&before).zip(&after) {
            let expected = state.policy_fatigue * region.income.level() as f32;
            assert!((b - a - expected).abs() < 1e-3);
        }

        // Repealing it gives the contentedness back.
        state.world.projects[&id].status = Status::Inactive;
        state.update_policy_fatigue();
        assert_eq!(state.policy_fatigue, 0.);
        for (b, a) in before.iter().zip(outlooks(&state)) {
            assert!((b - a).abs() < 1e-3);
        }
    }

    #[test]
    fn mandate_fatigue_fades() {
        let mut state = State::new(World::default());
        let id = by_name(&state, "Vegan Mandate");
        state.world.projects[&id].status = Status::Active;
        state.update_policy_fatigue();
        let first = state.policy_fatigue;
        for _ in 0..25 {
            state.update_policy_fatigue();
        }
        assert!(first > 1.5);
        assert_eq!(state.policy_fatigue, 0.);
    }
}
