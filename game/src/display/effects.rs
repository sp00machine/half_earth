use std::collections::BTreeMap;

use crate::{
    consts,
    display::{
        self, AsText, DisplayEvent, Icon,
        factors::factors_card,
        icons::{self, HasIcon},
    },
    text::bbcode,
    tips::{Tip, TipCard, add_tip, tip},
};
use egui::ahash::HashSet;
use hes_engine::*;
use rust_i18n::t;
use serde::{Deserialize, Serialize};

pub fn is_hidden(effect: &Effect) -> bool {
    matches!(effect, Effect::AddEvent(..) | Effect::TriggerEvent(..))
}

fn outcome_effects(project: &Project) -> Vec<DisplayEffect> {
    struct Count {
        effect: DisplayEffect,
        count: usize,
        hashes: HashSet<Vec<u8>>,
    }
    let mut all_effects: BTreeMap<String, Count> = BTreeMap::default();
    let n_outcomes = project.outcomes.len();
    for outcome in &project.outcomes {
        for effect in &outcome.effects {
            let key = effect.fingerprint();
            let mut efx = DisplayEffect::from(effect);
            let hash = bincode::serialize(&effect).unwrap();
            efx.likelihood = Some(outcome.probability.likelihood);
            let count = all_effects.entry(key).or_insert_with(|| Count {
                effect: efx,
                count: 0,
                hashes: HashSet::default(),
            });
            count.count += 1;
            count.hashes.insert(hash);
        }
    }

    all_effects
        .into_values()
        .map(
            |Count {
                 mut effect,
                 count,
                 hashes,
             }| {
                effect.is_unknown = count != n_outcomes || hashes.len() > 1;
                effect
            },
        )
        .collect()
}

pub fn active_effects(project: &Project) -> Vec<DisplayEffect> {
    let mut effects = vec![];

    if project.kind == ProjectType::Policy && !project.is_active() {
        // Project outcome effects are secret and delayed
        effects.extend(project.effects.iter().map(DisplayEffect::from));
    } else if project.status == Status::Inactive || project.status == Status::Building {
        effects.extend(project.effects.iter().map(DisplayEffect::from));
        effects.extend(outcome_effects(project));
    } else {
        effects.extend(project.active_effects().iter().map(DisplayEffect::from));
        if let Some(id) = project.active_outcome {
            effects.extend(project.outcomes[id].effects.iter().map(DisplayEffect::from));
        }
    }

    effects
}

pub fn visible_effect(d: &DisplayEffect) -> bool {
    !matches!(
        d.effect,
        Effect::ProjectRequest(..) | Effect::ProcessRequest(..)
    )
}

/// The lines a set of effects shows as, with effects that say the same
/// thing about several processes or regions merged into one line (e.g.
/// "Reduces output for A, B and C by 20%"), so cards stay short.
pub fn effect_lines(state: &State, effects: &[DisplayEffect]) -> Vec<EffectTip> {
    let visible: Vec<&DisplayEffect> = effects.iter().filter(|e| !e.is_hidden).collect();
    let mut lines = vec![];
    let mut used = vec![false; visible.len()];

    for i in 0..visible.len() {
        if used[i] || visible[i].is_unknown {
            continue;
        }
        match &visible[i].effect {
            Effect::OutputForProcess(_, amount) => {
                let group: Vec<usize> = (i..visible.len())
                    .filter(|&j| !used[j] && !visible[j].is_unknown)
                    .filter(|&j| matches!(visible[j].effect, Effect::OutputForProcess(_, a) if a == *amount))
                    .collect();
                if group.len() > 1 {
                    let processes: Vec<Process> = group
                        .iter()
                        .map(|&j| match visible[j].effect {
                            Effect::OutputForProcess(id, _) => state.world.processes[&id].clone(),
                            _ => unreachable!(),
                        })
                        .collect();
                    let names: Vec<String> = processes
                        .iter()
                        .map(|p| icon_card_tag(&t!(&p.name), p.output.icon()))
                        .collect();
                    lines.push(EffectTip {
                        tip: tip(
                            processes[0].output.icon(),
                            t!(
                                "Changes the output for these processes by %{percent}% with no change in impacts.",
                                percent = display::signed_percent(*amount, true),
                            ),
                        )
                        .card(processes.clone()),
                        text: icon_text(
                            "output",
                            &t!(
                                "%{changeDir} output for %{names} by [b]%{percent}%.[/b]",
                                changeDir = visible[i].change_dir(*amount),
                                names = names.join(", "),
                                percent = display::percent(amount.abs(), true),
                            ),
                        ),
                    });
                    group.iter().for_each(|&j| used[j] = true);
                }
            }
            Effect::RegionHabitability(_, amount) => {
                let group: Vec<usize> = (i..visible.len())
                    .filter(|&j| !used[j] && !visible[j].is_unknown)
                    .filter(|&j| matches!(visible[j].effect, Effect::RegionHabitability(_, a) if a == *amount))
                    .collect();
                if group.len() > 1 {
                    let kinds: Vec<String> = group
                        .iter()
                        .map(|&j| match &visible[j].effect {
                            Effect::RegionHabitability(lat, _) => t!(lat.lower()).to_string(),
                            _ => unreachable!(),
                        })
                        .collect();
                    lines.push(EffectTip {
                        tip: tip(
                            icons::HABITABILITY,
                            t!("Lower habitability means unhappier people who may need to migrate to more hospitable locales."),
                        ),
                        text: icon_text(
                            "habitability",
                            &t!(
                                "%{changeDir} habitability in %{kinds} regions by %{amount}.",
                                changeDir = visible[i].change_dir(*amount),
                                kinds = kinds.join(", "),
                                amount = visible[i].fmt_param(*amount),
                            ),
                        ),
                    });
                    group.iter().for_each(|&j| used[j] = true);
                }
            }
            _ => {}
        }
    }
    for (i, effect) in visible.iter().enumerate() {
        if !used[i] {
            if let Ok(tip) = effect.tip(state) {
                lines.push(tip);
            }
        }
    }
    lines
}

pub fn render_effects(ui: &mut egui::Ui, state: &State, effects: &[DisplayEffect]) {
    let all_effects = effects;
    let mut effects = effect_lines(state, effects);
    effects.sort_by_key(|effect| effect.text.clone());
    let summary = land_summary(state, all_effects);
    ui.vertical(|ui| {
        for effect in effects {
            let resp = ui.add(bbcode(&effect.text));
            add_tip(effect.tip, resp);
        }
        if let Some(summary) = summary {
            let resp = ui.add(bbcode(&format!("[i]{}[/i] {summary}", icons::LAND)));
            if let Some(detail) = land_detail(state, all_effects) {
                add_tip(tip(icons::LAND, detail), resp);
            }
        }
    });
}

trait AsKey {
    fn as_key(&self) -> &'static str;
}
impl AsKey for Resource {
    fn as_key(&self) -> &'static str {
        match self {
            Resource::Land => "land",
            Resource::Fuel => "fuel",
            Resource::Water => "water",
            Resource::Electricity => "electricity",
        }
    }
}
impl AsKey for Output {
    fn as_key(&self) -> &'static str {
        match self {
            Output::Electricity => "electricity",
            Output::Fuel => "fuel",
            Output::PlantCalories => "plant_calories",
            Output::AnimalCalories => "animal_calories",
        }
    }
}
impl AsKey for Byproduct {
    fn as_key(&self) -> &'static str {
        match self {
            Byproduct::Co2 => "co2",
            Byproduct::N2o => "n2o",
            Byproduct::Ch4 => "ch4",
            Byproduct::Biodiversity => "biodiversity",
            Byproduct::Fgas => "emissions",
        }
    }
}
impl AsKey for Feedstock {
    fn as_key(&self) -> &'static str {
        match self {
            Feedstock::Coal => "coal",
            Feedstock::Lithium => "lithium",
            Feedstock::NaturalGas => "natural_gas",
            Feedstock::Oil => "oil",
            Feedstock::Uranium => "uranium",
            Feedstock::Thorium => "thorium",
            Feedstock::Soil => "soil",
            Feedstock::Other => "other",
        }
    }
}
impl AsKey for ProjectType {
    fn as_key(&self) -> &'static str {
        match self {
            ProjectType::Policy => "policy",
            ProjectType::Research => "research",
            ProjectType::Initiative => "initiative",
        }
    }
}

fn icon_card_tag(name: &str, icon: Icon) -> String {
    format!(r#"[c][i]{icon}[/i]{name}[/c]"#)
}
fn card_tag(name: &str) -> String {
    format!(r#"[c]{name}[/c]"#)
}

fn icon_text(icon: &'static str, text: &str) -> String {
    format!("[{icon}] {text}")
}

/// What changing these processes' output would do to land: more land
/// needed (and the clearing emissions), or output lost where a process is
/// frozen. Empty if it barely changes land.
fn land_effect_note(state: &State, changes: &[(&Process, f32)], frozen_ids: &[Id]) -> String {
    let mut extra_land = 0.;
    let mut frozen_loss = false;
    for (process, change) in changes {
        let produced = state
            .produced
            .by_process
            .get(&process.id)
            .copied()
            .unwrap_or(0.);
        let land = process.resources.land;
        if produced <= 0. || land <= 0. {
            continue;
        }
        let m = process.output_modifier;
        // Frozen now, or frozen by the same card.
        let frozen = process.frozen_limit.is_some() || frozen_ids.contains(&process.id);
        if frozen && *change < 0. {
            frozen_loss = true;
            continue;
        }
        // Land per unit of output is land / (1 + modifier).
        let before = 1. / (1. + m).max(0.01);
        let after = 1. / (1. + m + change).max(0.01);
        extra_land += produced * land * (after - before);
    }
    let mut note = String::new();
    if frozen_loss {
        note.push_str(&format!(" {}", t!("Can't expand, so output falls.")));
    }
    if extra_land > 1e10 {
        let from = state.resource_demand.of(Resource::Land);
        let total = state.world.starting_resources.land;
        let co2 = state
            .land_carbon
            .estimate_clearing(from, from + extra_land, total);
        note.push_str(&format!(
            " {}",
            t!(
                "+%{area}M km² farmland: emissions +%{percent}% for %{years} yrs.",
                area = format!("{:.1}", extra_land * 1e-12),
                percent = {
                    let years = (extra_land / 3e11).ceil().max(1.);
                    let yearly = state.emissions.as_co2eq();
                    display::percent(
                        if yearly > 0. {
                            co2 / years / yearly
                        } else {
                            0.
                        },
                        true,
                    )
                },
                years = format!("{:.0}", (extra_land / 3e11).ceil().max(1.)),
            )
        ));
    } else if extra_land < -1e10 {
        note.push_str(&format!(
            " {}",
            t!(
                "Frees %{area}M km² of farmland.",
                area = format!("{:.1}", -extra_land * 1e-12),
            )
        ));
    }
    note
}

/// The farmland and clearing emissions behind a card's land summary, in Gt.
fn land_detail(state: &State, effects: &[DisplayEffect]) -> Option<String> {
    let mut extra_land = 0.;
    for e in effects.iter().filter(|e| !e.is_unknown) {
        let changes: Vec<(&Process, f32)> = match &e.effect {
            Effect::OutputForProcess(id, amount) => vec![(&state.world.processes[id], *amount)],
            Effect::OutputForFeature(feat, amount) => state
                .world
                .processes
                .iter()
                .filter(|p| !p.locked && p.features.contains(feat))
                .map(|p| (p, *amount))
                .collect(),
            _ => vec![],
        };
        for (process, change) in changes {
            if process.frozen_limit.is_some()
                || effects
                    .iter()
                    .any(|f| f.effect == Effect::FreezeProcess(process.id))
            {
                continue;
            }
            let produced = state
                .produced
                .by_process
                .get(&process.id)
                .copied()
                .unwrap_or(0.);
            let m = process.output_modifier;
            extra_land += produced
                * process.resources.land
                * (1. / (1. + m + change).max(0.01) - 1. / (1. + m).max(0.01));
        }
    }
    if extra_land <= 1e10 {
        return None;
    }
    let from = state.resource_demand.of(Resource::Land);
    let co2 = state.land_carbon.estimate_clearing(
        from,
        from + extra_land,
        state.world.starting_resources.land,
    );
    Some(
        t!(
            "Clearing about %{area} million km² of land for farms releases about %{amount}Gt of CO₂, most of it right away; land can only be cleared so fast, so it's spread over about %{years} years.",
            area = format!("{:.1}", extra_land * 1e-12),
            amount = format!("{:.0}", co2 * 1e-15),
            years = format!("{:.0}", (extra_land / 3e11).ceil().max(1.)),
        )
        .to_string(),
    )
}

/// One line on what a card's output changes, taken together, do to land.
pub fn land_summary(state: &State, effects: &[DisplayEffect]) -> Option<String> {
    let frozen_ids: Vec<Id> = effects
        .iter()
        .filter_map(|e| match e.effect {
            Effect::FreezeProcess(id) => Some(id),
            _ => None,
        })
        .collect();
    let mut changes: Vec<(&Process, f32)> = vec![];
    for e in effects.iter().filter(|e| !e.is_unknown) {
        match &e.effect {
            Effect::OutputForProcess(id, amount) => {
                changes.push((&state.world.processes[id], *amount))
            }
            Effect::OutputForFeature(feat, amount) => changes.extend(
                state
                    .world
                    .processes
                    .iter()
                    .filter(|p| !p.locked && p.features.contains(feat))
                    .map(|p| (p, *amount)),
            ),
            _ => {}
        }
    }
    let note = land_effect_note(state, &changes, &frozen_ids);
    let note = note.trim();
    (!note.is_empty()).then(|| note.to_string())
}

pub fn flag_tip(flag: Flag, demand: &OutputMap) -> Tip {
    let demand = display::outputs(demand);
    match flag {
        Flag::Electrified => {
            let changed_demand = demand.fuel * 0.8;
            tip(
                icons::ELECTRICITY,
                t!(
                    r#"Fuel demand will change from [i]%{iconFuel}[/i]%{prevDemandFuel} to [i]%{iconFuel}[/i]%{nextDemandFuel} and electricity demand will change from [i]%{iconElec}[/i]%{prevDemandElec} to [i]%{iconElec}[/i]%{nextDemandElec}."#,
                    iconFuel = icons::FUEL,
                    iconElec = icons::ELECTRICITY,
                    prevDemandFuel = display::rounded(demand.fuel),
                    nextDemandFuel = display::rounded(demand.fuel - changed_demand),
                    prevDemandElec = display::rounded(demand.electricity),
                    nextDemandElec = display::rounded(demand.electricity + changed_demand * 0.5),
                ),
            )
        }
        Flag::Vegan => {
            let changed_demand = demand.animal_calories * 0.9;
            tip(
                icons::PLANT_CALORIES,
                t!(
                    r#"Animal calorie demand will change from [i]%{iconACals}[/i]%{prevDemandACals} to [i]%{iconACals}[/i]%{nextDemandACals} and plant calorie demand will change from [i]%{iconPCals}[/i]%{prevDemandPCals} to [i]%{iconPCals}[/i]%{nextDemandPCals}."#,
                    iconACals = icons::ANIMAL_CALORIES,
                    iconPCals = icons::PLANT_CALORIES,
                    prevDemandACals = display::rounded(demand.animal_calories),
                    nextDemandACals = display::rounded(demand.animal_calories - changed_demand),
                    prevDemandPCals = display::rounded(demand.plant_calories),
                    nextDemandPCals = display::rounded(demand.plant_calories + changed_demand),
                ),
            )
        }
        Flag::Vegetarian => {
            let changed_demand = demand.animal_calories * 0.75;
            tip(
                icons::PLANT_CALORIES,
                t!(
                    r#"Animal calorie demand will change from [i]%{iconACals}[/i]%{prevDemandACals} to [i]%{iconACals}[/i]%{nextDemandACals} and plant calorie demand will change from [i]%{iconPCals}[/i]%{prevDemandPCals} to [i]%{iconPCals}[/i]%{nextDemandPCals}."#,
                    iconACals = icons::ANIMAL_CALORIES,
                    iconPCals = icons::PLANT_CALORIES,
                    prevDemandACals = display::rounded(demand.animal_calories),
                    nextDemandACals = display::rounded(demand.animal_calories - changed_demand),
                    prevDemandPCals = display::rounded(demand.plant_calories),
                    nextDemandPCals = display::rounded(demand.plant_calories + changed_demand),
                ),
            )
        }
        Flag::ClosedBorders => tip(
            icons::CLOSED_BORDERS,
            t!(
                "Migrations will have less of an impact when they occur. But there might be other consequences."
            ),
        ),
        Flag::HyperResearch => tip(
            icons::RESEARCH,
            t!(
                r#"Research points are 1[i]%{iconPC}[/i] cheaper."#,
                iconPC = icons::PLANT_CALORIES
            ),
        ),
        Flag::StopDevelopment => tip(
            icons::BAN,
            t!("Stops regional development throughout the world."),
        ),
        Flag::FastDevelopment => tip(
            icons::DEVELOPMENT,
            t!("Accelerates regional development throughout the world."),
        ),
        Flag::Degrowth => tip(
            icons::DEGROWTH,
            t!("Contract the economies of the wealthiest regions."),
        ),
        Flag::DeepSeaMining => tip(icons::OCEAN, t!("Prevents or stops metal shortages.")),
        Flag::MetalsShortage => tip(
            icons::IRON_AND_STEEL,
            t!("Infrastructure projects take 20% longer to finish."),
        ),
        Flag::ParliamentSuspended => tip(
            icons::THE_AUTHORITARIAN,
            t!("A parliamentary majority is no longer required for any project."),
        ),
        Flag::MoreLabor => tip(
            icons::LABOR,
            t!("Research and infrastructure take 10% less time to complete."),
        ),
        Flag::MoreLeisure => tip(
            icons::LABOR,
            t!("Research and infrastructure take 10% more time to complete."),
        ),
        Flag::MoreAutomation => tip(
            icons::LABOR,
            t!("Research and infrastructure take 10% less time to complete."),
        ),
        Flag::EcosystemModeling => tip(
            icons::BIRB,
            t!("Restoration projects take 10% less time to complete."),
        ),
        Flag::LaborResistance => tip(
            icons::LABOR,
            t!("Research and infrastructure take 5% more time to complete."),
        ),
        Flag::LaborSabotage => tip(
            icons::LABOR,
            t!("Research and infrastructure take 5% more time to complete."),
        ),
        Flag::AlienEncounter => tip(icons::ALERT, t!("You had an extraterrestrial encounter.")),
        Flag::BailedOut => tip(icons::ALERT, t!("You were bailed out.")),
        Flag::RepeatTutorial => tip(icons::ALERT, t!("Repeat the tutorial.")),
        Flag::SkipTutorial => tip(icons::ALERT, t!("Skip the tutorial.")),
        Flag::WarFooting => tip(
            icons::THE_AUTHORITARIAN,
            t!("Energy, electrification and buildings projects take 20% less time to complete, and production can shift 30% per planning cycle instead of 25%."),
        ),
        Flag::FossilMoratorium => tip(
            icons::BAN,
            t!("Fossil fuel processes can no longer be expanded."),
        ),
        Flag::SodiumIonBatteries => tip(
            icons::ELECTRICITY,
            t!("Sodium-ion batteries take over 40% of grid battery storage, needing no lithium. That lets lithium support more wind and solar."),
        )
        .card(TipCard::Storage),
        Flag::PumpedHydroStorage => tip(
            icons::ELECTRICITY,
            t!("Pumped hydro takes over 15% of grid storage, needing no lithium, but loses 20% of the energy stored (lithium batteries lose 10%)."),
        )
        .card(TipCard::Storage),
        Flag::LongDurationStorage => tip(
            icons::ELECTRICITY,
            t!("Long-duration storage takes over 30% of grid storage, needing no lithium, but loses over half the energy stored."),
        )
        .card(TipCard::Storage),
        Flag::ScheduledBlackouts => tip(
            icons::ELECTRICITY,
            t!("Power is cut on a schedule at night instead of storing wind and solar for those hours. Half the energy that would need storing isn't supplied, so grids need half the storage, and wind and solar can go further."),
        )
        .card(TipCard::Storage),
        Flag::LifeGoesOn => tip(icons::ALERT, t!("The game never ends.")),
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayEffect {
    pub effect: Effect,
    likelihood: Option<Likelihood>,
    is_unknown: bool,
    pub is_hidden: bool,
}

impl From<&Effect> for DisplayEffect {
    fn from(value: &Effect) -> Self {
        DisplayEffect {
            effect: value.clone(),
            likelihood: None,
            is_hidden: is_hidden(value),
            is_unknown: false,
        }
    }
}

pub struct EffectTip {
    pub tip: Tip,
    pub text: String,
}

// We use this to avoid doing
// `t!(&format!(...))` as with this it's harder
// to parse out the translation strings with need.
macro_rules! prefix_probs {
    ($prob:ident, $post:literal) => {
        match $prob {
            Likelihood::Guaranteed => {
                t!(concat!("Will", $post))
            }
            Likelihood::Likely => {
                t!(concat!("Likely to", $post))
            }
            Likelihood::Random => t!(concat!("Could", $post)),
            Likelihood::Unlikely => {
                t!(concat!("Unlikely to", $post))
            }
            Likelihood::Rare => {
                t!(concat!("Small chance to", $post))
            }
            Likelihood::Improbable => {
                t!(concat!("Tiny chance to", $post))
            }
            Likelihood::Impossible => {
                t!(concat!("Won't", $post))
            }
        }
    };
}

impl DisplayEffect {
    fn fmt_param(&self, value: f32) -> String {
        if self.is_unknown {
            r#"[u]?[/u]"#.into()
        } else {
            format!("[b]{}[/b]", value.abs())
        }
    }

    fn change_dir(&self, change: f32) -> String {
        if self.is_unknown {
            t!("Changes").to_string()
        } else if let Some(prob) = self.likelihood {
            if change < 0. {
                prefix_probs!(prob, " reduce").to_string()
            } else {
                prefix_probs!(prob, " increase").to_string()
            }
        } else {
            let term = if change < 0. {
                t!("Reduces")
            } else {
                t!("Increases")
            };
            format!("[b]{term}[/b]")
        }
    }

    // Hacky, but using `Result` as a way to short-circuit an
    // effect that shouldn't be displayed.
    pub fn tip(&self, state: &State) -> Result<EffectTip, ()> {
        let (tip, text) = match &self.effect {
            Effect::WorldVariable(var, amount) => match var {
                WorldVariable::Outlook => (
                    tip(
                        icons::CONTENTEDNESS,
                        format!(
                            "{} {}",
                            t!(
                                r#"Current world contentedness is %{contentedness}[t]/%{maxContentedness}[/t]."#,
                                contentedness = state.outlook().round(),
                                maxContentedness = consts::MAX_CONTENTEDNESS,
                            ),
                            t!(
                                "A quarter of this (%{regional}) is felt in every region too.",
                                regional = self.fmt_param(amount * 0.25),
                            ),
                        ),
                    ),
                    icon_text(
                        "contentedness",
                        &t!(
                            "%{changeDir} world contentedness by %{amount}.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(*amount),
                        ),
                    ),
                ),
                WorldVariable::Emissions => {
                    let emissions = state.emissions.as_gtco2eq();
                    (
                        tip(
                            icons::EMISSIONS,
                            t!(
                                "This will directly change annual emissions by %{amount}.%{percent}",
                                amount = if self.is_unknown {
                                    t!("an unknown amount").to_string()
                                } else {
                                    format!("{:+}", amount)
                                },
                                percent = if self.is_unknown || emissions == 0. {
                                    "".into()
                                } else {
                                    let percent = display::signed_percent(
                                        amount / state.emissions.as_gtco2eq(),
                                        true,
                                    );
                                    format!(
                                        " {}",
                                        t!("That's a %{percent}% change.", percent = percent)
                                    )
                                }
                            ),
                        ),
                        icon_text(
                            "emissions",
                            &t!(
                                "%{changeDir} emissions by %{amount}.",
                                changeDir = self.change_dir(*amount),
                                amount = self.fmt_param(*amount)
                            ),
                        ),
                    )
                }
                WorldVariable::ExtinctionRate => (
                    tip(
                        icons::EXTINCTION_RATE,
                        t!(
                            r#"Current biodiversity pressure is %{amount}[t]/%{maxAmount}[/t]."#,
                            amount = state.world.extinction_rate.round(),
                            maxAmount = consts::MAX_BIODIVERSITY
                        ),
                    ),
                    icon_text(
                        "extinction_rate",
                        &t!(
                            "%{changeDir} biodiversity pressure by %{amount}.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(*amount)
                        ),
                    ),
                ),
                WorldVariable::Temperature => (
                    tip(
                        icons::WARMING,
                        t!(
                            "This will directly change the global temperature anomaly by %{amount}[b]°c[/b].",
                            amount = format!("{:+}", amount)
                        ),
                    ),
                    icon_text(
                        "warming",
                        &t!(
                            "%{changeDir} the global temperature by %{amount}[b]°c[/b].",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(*amount)
                        ),
                    ),
                ),
                WorldVariable::Precipitation => (
                    tip(
                        icons::WATER,
                        t!(
                            "This will directly change global precipitation by %{amount}[b]cm/yr[/b].",
                            amount = format!("{:+}", amount)
                        ),
                    ),
                    icon_text(
                        "water",
                        &t!(
                            "%{changeDir} global precipitation by %{amount}[b]cm/yr[/b].",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(*amount)
                        ),
                    ),
                ),
                WorldVariable::PopulationGrowth => (
                    tip(icons::POPULATION, t!("The number of people on the planet.")),
                    icon_text(
                        "population",
                        &t!(
                            "%{changeDir} global population growth by %{amount}[b]%.[/b]",
                            changeDir = self.change_dir(*amount),
                            amount = display::percent(amount.abs(), false)
                        ),
                    ),
                ),
                WorldVariable::Population => (
                    tip(icons::POPULATION, t!("The number of people on the planet.")),
                    icon_text(
                        "population",
                        &t!(
                            "%{changeDir} global population by %{amount}.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(*amount)
                        ),
                    ),
                ),
                WorldVariable::SeaLevelRiseRate => (
                    tip(
                        icons::SEA_LEVEL_RISE,
                        t!(
                            "The amount of sea level rise is currently %{amount}m.",
                            amount = format!("{:.2}", state.world.sea_level_rise)
                        ),
                    ),
                    icon_text(
                        "sea_level_rise",
                        &t!(
                            "%{changeDir} the rate of sea level rise by %{amount}mm/year.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 1000.)
                        ),
                    ),
                ),
                WorldVariable::SeaLevelRise => (
                    tip(
                        icons::SEA_LEVEL_RISE,
                        t!(
                            "The amount of sea level rise is currently %{amount}m.",
                            amount = format!("{:.2}", state.world.sea_level_rise)
                        ),
                    ),
                    icon_text(
                        "sea_level_rise",
                        &t!(
                            "%{changeDir} the amount of sea level rise by %{amount}mm/year.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 1000.)
                        ),
                    ),
                ),
                WorldVariable::Deforestation => (
                    tip(
                        icons::LAND,
                        t!(
                            "Land is cleared every year even when farmland isn't growing, from logging, degradation and shifting farms. This releases about %{amount}Gt of CO2 each year.",
                            amount = format!("{:.1}", (state.land_carbon.last.deforestation + state.land_carbon.last.soil) * 1e-15)
                        ),
                    ),
                    icon_text(
                        "land",
                        &t!(
                            "%{changeDir} background deforestation by %{amount}%.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 100.)
                        ),
                    ),
                ),
                WorldVariable::Regrowth => (
                    tip(
                        icons::LAND,
                        t!("Farmland that is no longer used slowly grows back into wild land, pulling carbon out of the air as it does."),
                    ),
                    icon_text(
                        "land",
                        &t!(
                            "%{changeDir} how fast freed land regrows by %{amount}%.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 100.)
                        ),
                    ),
                ),
                WorldVariable::SoilHealth => (
                    tip(
                        icons::LAND,
                        t!(
                            "Healthy soils hold more water, nutrients and carbon, so farms need less land and fertilizer. Soil health is now %{health}%.",
                            health = format!("{:.0}", state.land_carbon.soil_health * 100.)
                        ),
                    ),
                    icon_text(
                        "land",
                        &t!(
                            "%{changeDir} soil health by %{amount} points.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount.abs() * 100.)
                        ),
                    ),
                ),
                WorldVariable::SoilRegeneration => (
                    tip(
                        icons::LAND,
                        t!(
                            "Intensive farming (synthetic fertilizer, bare fields between crops, heavy tilling) slowly wears soils down. Regenerative practices undo part of that damage, so soils recover over the following decades. Soil health is now %{health}%, heading toward %{target}% with farming as it is.",
                            health = format!("{:.0}", state.land_carbon.soil_health * 100.),
                            target = format!("{:.0}", state.land_carbon.soil_health_target(state.intensive_farming_share()) * 100.)
                        ),
                    ),
                    icon_text(
                        "land",
                        &t!(
                            "%{changeDir} soil damage from intensive farming by %{amount}%.",
                            changeDir = if *amount > 0. { t!("Reduces") } else { t!("Increases") },
                            amount = self.fmt_param(amount.abs() * 100.)
                        ),
                    ),
                ),
                WorldVariable::BatteryRecycling => (
                    tip(
                        icons::ELECTRICITY,
                        t!(
                            "Lithium from worn-out batteries can be recovered for new ones. Currently %{percent}% is recovered.",
                            percent = format!("{:.0}", state.storage.recycling * 100.)
                        ),
                    )
                    .card(TipCard::Storage),
                    icon_text(
                        "electricity",
                        &t!(
                            "%{changeDir} battery recycling by %{amount}%.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 100.)
                        ),
                    ),
                ),
                WorldVariable::LithiumMining => (
                    tip(
                        icons::ELECTRICITY,
                        t!(
                            "Batteries need lithium, and new mines take time to open. About %{amount}kt can be mined this year.",
                            amount = format!("{:.0}", state.storage.mining_capacity * 1e-9)
                        ),
                    )
                    .card(TipCard::Storage),
                    icon_text(
                        "electricity",
                        &t!(
                            "%{changeDir} how fast lithium mining expands, and how big a stockpile mines build up, by %{amount}%.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 100.)
                        ),
                    ),
                ),
                WorldVariable::StorageLosses => (
                    tip(
                        icons::ELECTRICITY,
                        t!("Grids with lots of wind and solar store energy for later, losing some on every round trip."),
                    )
                    .card(TipCard::Storage),
                    icon_text(
                        "electricity",
                        &t!(
                            "%{changeDir} energy lost in storage by %{amount}%.",
                            changeDir = self.change_dir(*amount),
                            amount = self.fmt_param(amount * 100.)
                        ),
                    ),
                ),
                WorldVariable::Year
                | WorldVariable::LandClearing
                | WorldVariable::LandRegrowth
                | WorldVariable::RegrowingLand
                | WorldVariable::FreedLand
                | WorldVariable::StorageShortfall => return Err(()),
            },
            Effect::PlayerVariable(var, amount) => match var {
                PlayerVariable::ResearchPoints => (
                    tip(
                        icons::RESEARCH,
                        t!("Research points: Allocate them to research projects!",),
                    ),
                    icon_text(
                        "research",
                        &if self.is_unknown {
                            t!(
                                "Possible +%{amount} research points.",
                                amount = self.fmt_param(*amount)
                            )
                        } else {
                            t!(
                                "+%{amount} research points.",
                                amount = self.fmt_param(*amount)
                            )
                        },
                    ),
                ),
                PlayerVariable::PoliticalCapital => (
                    tip(
                        icons::POLITICAL_CAPITAL,
                        t!(
                            r#"How much political capital you have. Political capital is what you spend to implement your plans. [w]If you run out you'll be pushed out of government.[/w]"#,
                        ),
                    ),
                    icon_text(
                        "political_capital",
                        &if self.is_unknown {
                            t!(
                                "Possible +%{amount} political capital.",
                                amount = self.fmt_param(*amount)
                            )
                        } else {
                            t!(
                                "+%{amount} political capital.",
                                amount = self.fmt_param(*amount)
                            )
                        },
                    ),
                ),
                PlayerVariable::YearsToDeath => return Err(()),
            },
            Effect::ProcessLimit(id, amount) => {
                let process = &state.world.processes[id];
                let change = if let Some(limit) = process.limit {
                    let p = (amount / limit).abs();
                    format!("{}%", display::percent(p, true))
                } else {
                    amount.round().abs().to_string()
                };
                let text = t!(
                    "%{changeDir} maximum output for %{process} by [b]%{amount}[/b].",
                    amount = change,
                    process = t!(&process.name),
                    changeDir = self.change_dir(*amount),
                );
                (tip(icons::ALERT, text.clone()), text.to_string())
            }
            Effect::FreezeProcess(id) => {
                let process = &state.world.processes[id];
                let text = t!(
                    "Freezes output of %{process} at its current level. Demand beyond that goes unmet unless it's reduced or met another way.",
                    process = t!(&process.name),
                );
                (tip(icons::ALERT, text.clone()), text.to_string())
            }
            Effect::RegionHabitability(lat, amount) => (
                tip(
                    icons::HABITABILITY,
                    t!(
                        "Lower habitability means unhappier people who may need to migrate to more hospitable locales."
                    ),
                ),
                icon_text(
                    "habitability",
                    &t!(
                        "%{changeDir} habitability in %{kind} regions by %{amount}.",
                        changeDir = self.change_dir(*amount),
                        amount = self.fmt_param(*amount),
                        kind = t!(lat.lower()),
                    ),
                ),
            ),
            Effect::Resource(resource, amount) => {
                let fmtted = display::resource(*amount, *resource, state.resources.available).abs();
                let fmtted = if matches!(resource, Resource::Water | Resource::Land) {
                    format!("{}%", fmtted.round())
                } else {
                    fmtted.to_string()
                };
                (
                    tip(
                        resource.icon(),
                        t!(
                            r#"%{changeDir} %{name} supply by [i]%{icon}[/i]%{amount}."#,
                            amount = fmtted,
                            icon = resource.icon(),
                            name = t!(resource.lower()),
                            changeDir = self.change_dir(*amount),
                        ),
                    )
                    .card(factors_card(None, (*resource).into(), state)),
                    icon_text(
                        resource.as_key(),
                        &t!(
                            "%{changeDir} %{name} supply by [i]%{icon}[/i]%{amount}.",
                            amount = fmtted,
                            name = t!(resource.lower()),
                            changeDir = self.change_dir(*amount),
                            icon = resource.as_key(),
                        ),
                    ),
                )
            }
            Effect::Output(output, amount) => {
                let base = display::output(state.produced.of(*output), *output);
                let changed = base * (1. + amount);
                (
                    tip(
                        output.icon(),
                        t!(
                            r#"Global %{name} output will change from [i]%{icon}[/i]%{base} to [i]%{icon}[/i]%{changed} with no change in impacts."#,
                            changed = changed.round(),
                            base = base.round(),
                            icon = output.icon(),
                            name = output.lower(),
                        ),
                    ),
                    icon_text(
                        output.as_key(),
                        &t!(
                            "%{changeDir} all %{name} production by [b]%{percent}%.[/b]",
                            percent = display::percent(amount.abs(), true),
                            name = t!(output.lower()),
                            changeDir = self.change_dir(*amount),
                            icon = output.as_key(),
                        ),
                    ),
                )
            }
            Effect::YieldShock(output, amount, years) => (
                tip(
                    output.icon(),
                    t!(
                        "Field farming yields for %{name} drop, then recover over %{years} years. While they're down, more land is needed to grow the same food.",
                        name = t!(output.lower()),
                        years = years
                    ),
                ),
                icon_text(
                    output.as_key(),
                    &t!(
                        "%{changeDir} %{name} farm yields by [b]%{percent}%[/b] for a while.",
                        percent = display::percent(amount.abs(), true),
                        name = t!(output.lower()),
                        changeDir = self.change_dir(*amount),
                    ),
                ),
            ),
            Effect::OutputForProcess(id, amount) => {
                let process = &state.world.processes[id];
                (tip(
                        process.output.icon(),
                        t!(
                            "Changes the output for this process by %{percent}% with no change in impacts.",
                            percent = display::signed_percent(*amount, true),
                        )).card(process.clone()), icon_text(
                    process.output.as_key(),
                    &(t!(
                        "%{changeDir} %{tag} output by [b]%{percent}%.[/b]",
                        percent = display::percent(amount.abs(), true),
                        tag = icon_card_tag(&t!(&process.name), process.output.icon()),
                        changeDir = self.change_dir(*amount),
                        icon = process.output.icon(),
                    ).to_string())))
            }
            Effect::OutputForFeature(feat, amount) => {
                let processes: Vec<_> = state
                    .world
                    .processes
                    .iter()
                    .filter(|p| !p.locked && p.features.contains(feat))
                    .cloned()
                    .collect();
                (tip(
                        feat.icon(),
                        t!("Changes the output for these processes by %{percent}% without changing their impacts.",
                            percent = display::signed_percent(*amount, true)),
                ).card(processes.clone()),
                icon_text(
                    "output",
                    &(t!(
                        r#"%{changeDir} output for [e][i]%{icon}[/i][b]%{feature}[/b][/e] by [b]%{percent}%.[/b]"#,
                        percent = display::percent(amount.abs(), true),
                        icon = feat.icon(),
                        feature = t!(feat.lower()),
                        changeDir = self.change_dir(*amount),
                    ).to_string()))) 
            }
            Effect::CO2ForFeature(feat, amount) => {
                let processes: Vec<_> = state
                    .world
                    .processes
                    .iter()
                    .filter(|p| !p.locked && p.features.contains(feat))
                    .cloned()
                    .collect();
                (
                    tip(
                        feat.icon(),
                        t!(
                            "%{changeDir} the CO2 emissions for these processes by [b]%{percent}%.[/b]",
                            percent = display::percent(amount.abs(), true),
                            changeDir = self.change_dir(*amount),
                        )
                    )
                    .card(processes),
                    icon_text(
                        "emissions",
                        &t!(
                            r#"%{changeDir} CO2 emissions for [e][i]%{icon}[/i]%{feature}[/e] by [b]%{percent}%.[/b]"#,
                            percent = display::percent(amount.abs(), true),
                            icon = feat.icon(),
                            feature = t!(feat.lower()),
                            changeDir = self.change_dir(*amount),
                        )),
                )
            }
            Effect::BiodiversityPressureForFeature(feat, amount) => {
                let processes: Vec<_> = state
                    .world
                    .processes
                    .iter()
                    .filter(|p| !p.locked && p.features.contains(feat))
                    .cloned()
                    .collect();
                (tip(
                        feat.icon(),
                        t!(
                            "Changes the biodiversity pressure for these processes by [b]%{amount}.[/b]",
                            amount = format!("{:+}", amount),
                        )
                ).card(processes), icon_text(
                "biodiversity",
                &t!(
                    r#"%{changeDir} biodiversity pressure for [e][i]%{icon}[/i]%{feature}[/e] by [b]%{amount}.[/b]"#,
                    amount = amount.abs(),
                    icon = feat.icon(),
                    feature = t!(feat.lower()),
                    changeDir = self.change_dir(*amount),
                )))
            }
            Effect::Demand(output, amount) => {
                let demand = display::outputs(&state.output_demand.total());
                let current_demand = demand[*output];
                let after_demand = demand[*output] * (1. + amount);
                (
                    tip(
                        output.icon(),
                        t!(
                            r#"This changes %{name} demand from [i]%{icon}[/i]%{currentDemand} to [i]%{icon}[/i]%{afterDemand}."#,
                            afterDemand = after_demand.round(),
                            currentDemand = current_demand.round(),
                            icon = output.icon(),
                            name = t!(output.lower()),
                        ),
                    ),
                    icon_text(
                        output.as_key(),
                        &t!(
                            "%{changeDir} demand for %{name} by [b]%{percent}%[/b].",
                            percent = display::percent(amount.abs(), true),
                            changeDir = self.change_dir(*amount),
                            name = t!(output.lower()),
                        ),
                    ),
                )
            }
            Effect::DemandAmount(output, amount) => {
                let demand = display::outputs(&state.output_demand.total());
                let amount = display::output(*amount, *output);
                let current_demand = demand[*output];
                let after_demand = demand[*output] + amount;
                let demand_change = (after_demand - current_demand) / current_demand;
                (
                    tip(
                        output.icon(),
                        t!(
                            r#"This changes %{name} demand from [i]%{icon}[/i]%{currentDemand} to [i]%{icon}[/i]%{afterDemand}. This is a %{percent}% change of all %{name} demand."#,
                            percent = display::signed_percent(demand_change.abs(), true),
                            afterDemand = after_demand,
                            currentDemand = current_demand,
                            icon = output.icon(),
                            name = t!(output.lower()),
                        ),
                    ),
                    icon_text(
                        output.as_key(),
                        &t!(
                            r#"%{changeDir} demand for %{name} by [i]%{icon}[/i]%{amount}."#,
                            amount = amount.abs(),
                            icon = output.icon(),
                            name = t!(output.lower()),
                            changeDir = self.change_dir(amount),
                        ),
                    ),
                )
            }
            Effect::UnlocksProject(id) => {
                let project = &state.world.projects[id];
                let prob = if self.is_unknown
                    && let Some(prob) = self.likelihood
                {
                    prob
                } else {
                    Likelihood::Guaranteed
                };
                let tag = icon_card_tag(&t!(&project.name), project.kind.icon());
                let text = if self.is_unknown
                    && let Some(prob) = self.likelihood
                {
                    prefix_probs!(prob, " unlock the %{tag} project.")
                } else {
                    t!("[b]Unlocks[/b] the %{tag} project.")
                };

                (
                    tip(icons::UNLOCKS, prefix_probs!(prob, " unlock this project:"))
                        .card(project.clone()),
                    icon_text("unlocks", &t!(text.to_string(), tag = tag,)),
                )
            }
            Effect::UnlocksProcess(id) => {
                let process = &state.world.processes[id];
                let prob = if self.is_unknown
                    && let Some(prob) = self.likelihood
                {
                    prob
                } else {
                    Likelihood::Guaranteed
                };
                let tag = icon_card_tag(&t!(&process.name), process.output.icon());
                let text = if self.is_unknown
                    && let Some(prob) = self.likelihood
                {
                    prefix_probs!(prob, " unlock the %{tag} process.")
                } else {
                    t!("[b]Unlocks[/b] the %{tag} process.")
                };

                (
                    tip(icons::UNLOCKS, prefix_probs!(prob, " unlock this process:"))
                        .card(process.clone()),
                    icon_text("unlocks", &t!(text.to_string(), tag = tag,)),
                )
            }
            Effect::UnlocksNPC(id) => {
                let npc = &state.npcs[id];
                let text = if self.is_unknown
                    && let Some(prob) = self.likelihood
                {
                    prefix_probs!(prob, " unlock %{name}.")
                } else {
                    t!("[b]Unlocks[/b] %{name}.")
                };
                (
                    tip(icons::UNLOCKS, t!("This new character will be unlocked:"))
                        .card(npc.clone()),
                    icon_text("unlocks", &t!(text.to_string(), name = t!(&npc.name),)),
                )
            }
            Effect::ProjectCostModifier(id, amount) => {
                let project = &state.world.projects[id];

                let tag = icon_card_tag(&t!(&project.name), project.kind.icon());
                let kind = match project.kind {
                    ProjectType::Policy => t!("cost"),
                    ProjectType::Research => {
                        t!("research time")
                    }
                    ProjectType::Initiative => {
                        t!("development time")
                    }
                };
                let tip_amount = if self.is_unknown {
                    t!("an unknown amount")
                } else if project.is_policy() {
                    format!(
                        "{}% [political_capital]",
                        display::percent(amount.abs(), true)
                    )
                    .into()
                } else {
                    format!("{}%", display::percent(amount.abs(), true)).into()
                };
                (
                    tip(
                        icons::COST,
                        t!(
                            "This effect %{changeDir} the %{kind} of this project by %{tipAmount}.",
                            tipAmount = tip_amount,
                            kind = kind,
                            changeDir = self.change_dir(*amount).to_lowercase(),
                        ),
                    )
                    .card(project.clone()),
                    icon_text(
                        "cost",
                        &t!(
                            "%{changeDir} %{kind} of %{tag} by [b]%{tipAmount}[/b].",
                            changeDir = self.change_dir(*amount),
                            kind = kind,
                            tag = tag,
                            tipAmount = tip_amount,
                        ),
                    ),
                )
            }
            Effect::ProjectRequest(id, active, bounty) => {
                let project = &state.world.projects[id];
                if *active {
                    (
                        tip(
                            icons::REQUEST,
                            t!("You received a request to implement this project:"),
                        )
                        .card(project.clone()),
                        icon_text(
                            "implement",
                            &t!(
                                "I request that you implement %{name}. (+%{pc}PC)",
                                name = t!(&project.name),
                                pc = bounty,
                            ),
                        ),
                    )
                } else {
                    (
                        tip(
                            icons::REQUEST,
                            t!("You received a request to stop this project:"),
                        )
                        .card(project.clone()),
                        icon_text(
                            "ban",
                            &t!(
                                "I request that you stop %{name}. (+%{pc}PC)",
                                name = t!(&project.name),
                                pc = bounty,
                            ),
                        ),
                    )
                }
            }
            Effect::ProcessRequest(id, active, bounty) => {
                let process = &state.world.processes[id];
                if *active {
                    (
                        tip(
                            icons::REQUEST,
                            t!("You received a request to promote this process:"),
                        )
                        .card(process.clone()),
                        icon_text(
                            "implement",
                            &t!(
                                "I request that you implement %{name}. (+%{pc}PC)",
                                name = t!(&process.name),
                                pc = bounty,
                            ),
                        ),
                    )
                } else {
                    (
                        tip(
                            icons::REQUEST,
                            t!("You received a request to ban this process:"),
                        )
                        .card(process.clone()),
                        icon_text(
                            "ban",
                            &t!(
                                "I request that you stop %{name}. (+%{pc}PC)",
                                name = t!(&process.name),
                                pc = bounty,
                            ),
                        ),
                    )
                }
            }
            Effect::ModifyIndustryDemand(id, amount) => {
                let industry = &state.world.industries[id];
                let tag = card_tag(&t!(&industry.name));
                let tip_text = if self.is_unknown {
                    t!(
                        "Changes demand for %{name} by an unknown amount.",
                        name = t!(&industry.name)
                    )
                } else {
                    t!(
                        "Changes demand for %{name} by [b]%{percent}%.[/b]",
                        name = t!(&industry.name),
                        percent = display::percent(*amount, true),
                    )
                };
                (
                    tip(icons::DEMAND, tip_text).card(industry.clone()),
                    icon_text(
                        "demand",
                        &t!(
                            "%{changeDir} demand for %{tag} by %{amount}.",
                            amount = if self.is_unknown {
                                self.fmt_param(*amount)
                            } else {
                                format!("{}%", display::percent(amount.abs(), true))
                            },
                            changeDir = self.change_dir(*amount),
                            tag = tag,
                        ),
                    ),
                )
            }
            Effect::ModifyIndustryResources(id, resource, amount) => {
                let industry = &state.world.industries[id];
                let lic_pop = state.world.lic_population();
                let current_demand = industry.total_demand_for_resource(lic_pop, *resource);
                let current_demand =
                    display::resource(current_demand, *resource, state.resources.available);
                let after_demand = current_demand * (1. + amount);
                let total_demand = display::resource(
                    state.resource_demand.of(*resource),
                    *resource,
                    state.resources.available,
                );
                let change = after_demand - current_demand;
                let demand_change = (total_demand + change) / total_demand - 1.;
                let tag = card_tag(&t!(&industry.name));
                let tip = if self.is_unknown {
                    tip(
                        resource.icon(),
                        t!(
                            "This will change %{resource} demand for %{name} by some unknown amount.",
                            name = t!(&industry.name),
                            resource = t!(resource.lower()),
                        ),
                    )
                } else {
                    tip(
                        resource.icon(),
                        t!(
                            r#"This will change %{resource} demand for %{name} from [i]%{icon}[/i]%{demandBefore} to [i]%{icon}[/i]%{demandAfter}. This is a %{percent}% change of all %{resource} demand."#,
                            name = t!(&industry.name),
                            resource = t!(resource.lower()),
                            icon = resource.icon(),
                            percent = display::signed_percent(demand_change, true),
                            demandAfter = display::rounded(after_demand),
                            demandBefore = display::rounded(current_demand),
                        ),
                    )
                };
                (
                    tip.card(industry.clone()),
                    icon_text(
                        resource.as_key(),
                        &t!(
                            "%{changeDir} %{resource} demand for %{tag} by %{amount}.",
                            tag = tag,
                            resource = t!(resource.lower()),
                            amount = if self.is_unknown {
                                self.fmt_param(*amount)
                            } else {
                                format!("{}%", display::percent(amount.abs(), true))
                            },
                            changeDir = self.change_dir(*amount),
                        ),
                    ),
                )
            }
            Effect::ModifyIndustryResourcesAmount(id, resource, amount) => {
                let industry = &state.world.industries[id];
                let lic_pop = state.world.lic_population();
                let demand = industry.demand(lic_pop);
                let current_demand = display::resource(
                    industry.resources[*resource] * demand,
                    *resource,
                    state.resources.available,
                );
                let after_demand = display::resource(
                    (industry.resources[*resource] + amount) * demand,
                    *resource,
                    state.resources.available,
                );
                let total_demand = display::resource(
                    state.resource_demand.of(*resource),
                    *resource,
                    state.resources.available,
                );
                let change = after_demand - current_demand;
                let demand_change = (total_demand + change) / total_demand - 1.;
                let tag = card_tag(&t!(&industry.name));
                let tip = if self.is_unknown {
                    tip(
                        resource.icon(),
                        t!(
                            "This will change %{resource} demand for %{name} by some unknown amount.",
                            name = t!(&industry.name),
                            resource = t!(resource.lower()),
                        ),
                    )
                } else {
                    tip(
                        resource.icon(),
                        t!(
                            r#"This will change %{resource} demand for %{name} from [i]%{icon}[/i]%{demandBefore} to [i]%{icon}[/i]%{demandAfter}. This is a %{percent}% change of all %{resource} demand."#,
                            name = t!(&industry.name),
                            resource = t!(resource.lower()),
                            icon = resource.icon(),
                            percent = display::signed_percent(demand_change, true),
                            demandAfter = display::rounded(after_demand),
                            demandBefore = display::rounded(current_demand),
                        ),
                    )
                };
                // Electric vehicles need batteries, which need lithium.
                let lithium_note = if industry.name == "Road Transport"
                    && *resource == Resource::Electricity
                    && *amount > 0.
                    && !self.is_unknown
                {
                    let lithium = hes_engine::storage::ev_lithium_for(amount * demand);
                    format!(
                        " {}",
                        t!(
                            "Their batteries need about %{amount} of lithium. The grid gets lithium first; vehicles that can't get batteries keep running on fuel until mining catches up.",
                            amount = if lithium >= 1e12 {
                                format!("{:.1} Mt", lithium * 1e-12)
                            } else {
                                format!("{:.0} kt", lithium * 1e-9)
                            }
                        )
                    )
                } else {
                    String::new()
                };
                (
                    tip.card(industry.clone()),
                    icon_text(
                        resource.as_key(),
                        &(t!(
                            "%{changeDir} %{resource} demand for %{tag} by %{amount}.",
                            tag = tag,
                            resource = t!(resource.lower()),
                            amount = if self.is_unknown {
                                self.fmt_param(*amount)
                            } else {
                                (after_demand - current_demand).round().abs().to_string()
                            },
                            changeDir = self.change_dir(*amount),
                        )
                        .to_string()
                            + &lithium_note),
                    ),
                )
            }
            Effect::ModifyIndustryByproducts(id, byproduct, amount) => {
                let industry = &state.world.industries[id];
                let lic_pop = state.world.lic_population();
                let current = industry.total_byproducts(lic_pop).gtco2eq();
                let after = current * (1. + amount);
                let change = (after - current) / state.emissions.as_gtco2eq();
                let tag = card_tag(&t!(&industry.name));
                let tip_text = if self.is_unknown {
                    t!(
                        "Changes emissions for %{name} by an unknown amount.",
                        name = t!(&industry.name)
                    )
                } else {
                    t!(
                        r#"This will change emissions for %{name} from [i]%{icon}[/i]%{emissionsBefore} to [i]%{icon}[/i]%{emissionsAfter}. This is a %{emissionsChange}% change of all emissions."#,
                        name = t!(&industry.name),
                        icon = icons::EMISSIONS,
                        emissionsChange = display::percent(change, true),
                        emissionsAfter = display::rounded(after),
                        emissionsBefore = display::rounded(current),
                    )
                };
                (
                    tip(icons::EMISSIONS, tip_text).card(industry.clone()),
                    icon_text(
                        "emissions",
                        &t!(
                            "%{changeDir} %{kind} emissions for %{tag} by [b]%{amount}[/b].",
                            tag = tag,
                            kind = t!(byproduct.lower()),
                            amount = if self.is_unknown {
                                self.fmt_param(*amount)
                            } else {
                                format!("{}%", display::percent(amount.abs(), true))
                            },
                            changeDir = self.change_dir(*amount),
                        ),
                    ),
                )
            }
            Effect::ModifyProcessByproducts(id, byproduct, amount) => {
                let process = &state.world.processes[id];
                let label = match byproduct {
                    Byproduct::Biodiversity => {
                        t!("biodiversity pressure")
                    }
                    Byproduct::Co2 => {
                        t!("%{kind} emissions", kind = t!("CO2"))
                    }
                    Byproduct::N2o => {
                        t!("%{kind} emissions", kind = t!("N2O"))
                    }
                    Byproduct::Fgas => {
                        t!("%{kind} emissions", kind = t!("F-gas"))
                    }
                    Byproduct::Ch4 => {
                        t!("%{kind} emissions", kind = t!("CH4"))
                    }
                };
                let tag = card_tag(&t!(&process.name));
                let tip_text = if self.is_unknown {
                    t!(
                        "Changes %{label} for %{name} by an unknown amount.",
                        name = t!(&process.name),
                        label = label,
                    )
                } else {
                    let change = match byproduct {
                        Byproduct::Biodiversity => {
                            let current = process.adj_byproducts().biodiversity;
                            let mut after_process = process.clone();
                            after_process.byproduct_modifiers.biodiversity += amount;
                            let after = after_process.adj_byproducts().biodiversity;
                            t!(
                                r#"%{fromAmount} to %{toAmount}[i]%{icon}[/i]."#,
                                icon = byproduct.icon(),
                                toAmount = after,
                                fromAmount = current,
                            )
                        }
                        _ => {
                            let current = process.adj_byproducts().gtco2eq()
                                * state.produced.by_process.get(id).unwrap_or(&0.);
                            let after = current * (1. + amount);
                            let change = (after - current) / state.emissions.as_gtco2eq();
                            t!(
                                r#"%{emissionsBefore} to [i]%{icon}[/i]%{emissionsAfter}. This is a %{emissionsChange}% change of all emissions."#,
                                icon = icons::EMISSIONS,
                                emissionsChange = display::percent(change, true),
                                emissionsAfter = display::rounded(after),
                                emissionsBefore = display::rounded(current)
                            )
                        }
                    };

                    t!(
                        r#"This will change %{short} for %{name} from [i]%{icon}[/i]%{change}"#,
                        name = t!(&process.name),
                        icon = byproduct.icon(),
                        short = match byproduct {
                            Byproduct::Biodiversity => t!("biodiversity pressure"),
                            _ => t!("emissions"),
                        },
                        change = change,
                    )
                };
                (
                    tip(byproduct.icon(), tip_text),
                    icon_text(
                        match byproduct {
                            Byproduct::Biodiversity => "biodiversity",
                            _ => "emissions",
                        },
                        &t!(
                            "%{changeDir} %{label} for %{tag} by [b]%{amount}[/b].",
                            tag = tag,
                            label = label,
                            icon = byproduct.icon(),
                            amount = if self.is_unknown {
                                self.fmt_param(*amount)
                            } else {
                                format!("{}%", display::percent(amount.abs(), true))
                            },
                            changeDir = self.change_dir(*amount),
                        ),
                    ),
                )
            }
            Effect::DemandOutlookChange(output, amount) => {
                let change = mean_demand_outlook_change(*amount, output, state).round();
                (
                    tip(
                        icons::CONTENTEDNESS,
                        t!(
                            r#"This changes regional contentedness based on demand for %{name}. Current world contentedeness is %{amount}[t]/%{maxAmount}[/t]."#,
                            name = t!(output.lower()),
                            maxAmount = consts::MAX_CONTENTEDNESS,
                            amount = state.outlook().round(),
                        ),
                    ),
                    icon_text(
                        "contentedness",
                        &t!(
                            "%{changeDir} world contentedness by [b]%{amount}[/b].",
                            amount = change.abs(),
                            changeDir = self.change_dir(*amount),
                        ),
                    ),
                )
            }
            Effect::IncomeOutlookChange(amount) => {
                let change = mean_income_outlook_change(*amount, state).round();
                (
                    tip(
                        icons::CONTENTEDNESS,
                        t!(
                            r#"This changes regional contentedness based on income level (wealthier regions will feel it more). Current world contentedeness is %{contentedness}[t]/%{maxContentedness}[/t]."#,
                            maxContentedness = consts::MAX_CONTENTEDNESS,
                            contentedness = state.outlook().round(),
                            amount = amount
                        ),
                    ),
                    icon_text(
                        "contentedness",
                        &t!(
                            "%{changeDir} contentedness by [b]%{amount}[/b].",
                            amount = change.abs(),
                            changeDir = self.change_dir(*amount),
                        ),
                    ),
                )
            }
            Effect::OutlookByIncome(amounts) => {
                let fmt = |v: f32| format!("{}", display::rounded(v));
                let fmt_signed = |v: f32| if v > 0. { format!("+{}", fmt(v)) } else { fmt(v) };
                let parts = format!(
                    "{} {}, {} {}, {} {}, {} {}",
                    t!("low"), fmt_signed(amounts[0]),
                    t!("lower-middle"), fmt_signed(amounts[1]),
                    t!("upper-middle"), fmt_signed(amounts[2]),
                    t!("high"), fmt_signed(amounts[3]),
                );
                (
                    tip(
                        icons::CONTENTEDNESS,
                        t!(
                            "This changes contentedness in each region depending on its income level. Tap for contentedness by income level."
                        ),
                    )
                    .card(crate::tips::TipCard::Contentedness(None)),
                    icon_text(
                        "contentedness",
                        &t!("Contentedness by income: %{parts}.", parts = parts),
                    ),
                )
            }
            Effect::ModifyEventProbability(id, amount) => {
                let event = &state.event_pool.events[id];
                let amount_label = if self.is_unknown {
                    self.fmt_param(*amount)
                } else {
                    format!("{}%", display::percent(amount.abs(), true))
                };
                let text = if !self.is_unknown && *amount <= -0.999 {
                    t!(r#"Prevents "%{event}"."#, event = t!(&event.name))
                } else {
                    t!(
                        r#"%{changeDir} the chance of "%{event}" by %{amount}."#,
                        event = t!(&event.name),
                        amount = amount_label,
                        changeDir = self.change_dir(*amount),
                    )
                };
                (
                    tip(icons::CHANCE, t!(text.to_string())).card(DisplayEvent::new(
                        ResolvedEvent {
                            event: event.clone(),
                            region: None,
                        },
                        state,
                    )),
                    icon_text("chance", &text),
                )
            }
            Effect::AddFlag(flag) => {
                let tip = flag_tip(*flag, &state.output_demand.total());
                let text = format!("[b]{}[/b]", t!(flag.to_string()));
                (tip, text)
            }
            Effect::ProtectLand(amount) => {
                let before = state.protected_land;
                let after = state.protected_land + amount;
                (
                    tip(
                        icons::LAND,
                        t!(
                            "This will limit the amount of land that processes can use. The amount of land under protection will change from %{before}% to %{after}%.",
                            before = display::percent(before, true),
                            after = display::percent(after, true),
                        ),
                    ),
                    icon_text(
                        "land",
                        &t!(
                            "Change the amount of land under protection by [b]%{percent}%[/b].",
                            percent = display::signed_percent(*amount, true),
                        ),
                    ),
                )
            }
            Effect::RestoreLand(amount) => {
                let before = state.protected_land;
                let after = state.protected_land + amount;
                (
                    tip(
                        icons::LAND,
                        t!(
                            "Restoration protects land that is regrowing first, then farmland as it comes out of use, rather than land that is already wild. Protected regrowing land grows back faster and is never cleared again. The amount of land under protection will change from %{before}% to %{after}%.",
                            before = display::percent(before, true),
                            after = display::percent(after, true),
                        ),
                    ),
                    icon_text(
                        "land",
                        &t!(
                            "Restore and protect [b]%{percent}%[/b] of land, regrowing land first.",
                            percent = display::percent(*amount, true),
                        ),
                    ),
                )
            }
            Effect::ModifyProcessResourcesAmount(id, resource, amount) => {
                let process = &state.world.processes[id];
                let tag = card_tag(&t!(&process.name));
                let produced = state.produced.by_process.get(id).copied().unwrap_or(0.);
                let total = amount * produced;
                let (label, detail) = match resource {
                    Resource::Electricity | Resource::Fuel => {
                        let gw = display::round_energy(display::to_energy_units(total));
                        let share = total / state.output_demand.of(Output::Electricity).max(1.);
                        (
                            t!("electricity"),
                            t!(
                                "At today's output that's about %{amount}PWh a year, %{percent}% of the world's electricity.",
                                amount = gw,
                                percent = display::percent(share, true)
                            ),
                        )
                    }
                    other => (t!(other.to_string()), String::new().into()),
                };
                (
                    tip(
                        icons::ELECTRICITY,
                        t!(
                            "This changes how much %{label} %{name} uses for each unit it makes. %{detail}",
                            label = label,
                            name = t!(&process.name),
                            detail = detail,
                        ),
                    ),
                    icon_text(
                        "electricity",
                        &t!(
                            "%{changeDir} %{label} used by %{tag}.",
                            changeDir = self.change_dir(*amount),
                            label = label,
                            tag = tag,
                        ),
                    ),
                )
            }
            Effect::Feedstock(feedstock, amount) => {
                let estimate = match feedstock {
                    Feedstock::Other | Feedstock::Soil => None,
                    other => {
                        let est = state.feedstocks.until_exhaustion(*other);
                        Some(est.round())
                    }
                };
                let available = state.feedstocks.available[*feedstock];
                let text = match estimate {
                    None => {
                        t!("We aren't tracking this feedstock.")
                    }
                    Some(0.) => {
                        t!("This feedstock has been depleted.")
                    }
                    Some(est) => {
                        if est.is_infinite() {
                            t!(
                                "At current usage rates the estimated supply is expected to last indefinitely."
                            )
                        } else {
                            t!(
                                "At current usage rates the estimated supply is expected to last %{years} year(s).",
                                years = est
                            )
                        }
                    }
                };
                (
                    tip(feedstock.icon(), t!(text.to_string())),
                    icon_text(
                        feedstock.as_key(),
                        &t!(
                            "%{changeDir} %{name} supply by [b]%{percent}%.[/b]",
                            name = t!(feedstock.lower()),
                            percent = display::percent((amount / available).abs(), true),
                            changeDir = self.change_dir(*amount),
                        ),
                    ),
                )
            }
            Effect::LocksProject(id) => {
                let project = &state.world.projects[id];
                let tag = icon_card_tag(&t!(&project.name), project.kind.icon());
                (
                    tip(
                        icons::ALERT,
                        t!(
                            "%{name} will be unavailable while this project is active.",
                            name = t!(&project.name),
                        ),
                    )
                    .card(project.clone()),
                    icon_text("locks", &t!("[b]Locks[/b] %{tag}", tag = tag,)),
                )
            }
            Effect::EnactPolicy(id) | Effect::RepealPolicy(id) => {
                let project = &state.world.projects[id];
                let tag = icon_card_tag(&t!(&project.name), project.kind.icon());
                let enact = matches!(self.effect, Effect::EnactPolicy(..));
                (
                    tip(
                        icons::POLITICAL_CAPITAL,
                        if enact {
                            t!("%{name} is passed right away, at no political capital cost.", name = t!(&project.name))
                        } else {
                            t!("%{name} is repealed right away.", name = t!(&project.name))
                        },
                    )
                    .card(project.clone()),
                    icon_text(
                        "political_capital",
                        &if enact {
                            t!("[b]Passes[/b] %{tag}", tag = tag)
                        } else {
                            t!("[b]Repeals[/b] %{tag}", tag = tag)
                        },
                    ),
                )
            }
            Effect::TerminationShock => {
                if let Some(project) = state
                    .world
                    .projects
                    .iter()
                    .find(|p| p.name.contains("Solar Radiation Management"))
                {
                    let effects = project.active_effects();
                    let mut temp_change = 0.;
                    if let Some(temp_effect) = effects.iter().find_map(|eff| match eff {
                        Effect::WorldVariable(WorldVariable::Temperature, val) => Some(*val),
                        _ => None,
                    }) {
                        temp_change -= temp_effect;
                    }
                    (
                        tip(
                            icons::WARMING,
                            t!(
                                "This will directly change the global temperature anomaly by %{amount}[b]°c[/b].",
                                amount = format!("{:+}", temp_change),
                            ),
                        ),
                        icon_text(
                            "warming",
                            &t!(
                                "%{changeDir} the global temperature by %{amount}[b]°c[/b].",
                                changeDir = self.change_dir(temp_change),
                                amount = self.fmt_param(temp_change),
                            ),
                        ),
                    )
                } else {
                    return Err(());
                }
            }
            Effect::RemoveRegionFlag(flag) => (
                tip(
                    icons::CONTENTEDNESS,
                    t!("Regions in unrest calm down a step. Unrest can return if contentedness falls again."),
                ),
                icon_text(
                    "contentedness",
                    &match flag {
                        RegionFlag::Revolts => t!("Ends revolts."),
                        RegionFlag::Riots => t!("Ends riots."),
                        RegionFlag::Protests => t!("Ends protests."),
                    },
                ),
            ),
            Effect::AddEvent(..)
            | Effect::TriggerEvent(..)
            | Effect::RegionLeave
            | Effect::Migration
            | Effect::AddRegionFlag(..)
            | Effect::GameOver
            | Effect::BailOut(..)
            | Effect::NPCRelationship(..) => {
                return Err(());
            }
        };
        Ok(EffectTip { tip, text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_lite::Regex;

    /// For comparing against floats
    /// but also "<1" which is what we use to
    /// indicate a small but non-zero value.
    #[derive(Debug, PartialEq, Clone, Copy)]
    enum Value {
        Small, // <1
        Exact(f32),
    }
    impl PartialEq<f32> for Value {
        fn eq(&self, other: &f32) -> bool {
            match self {
                Self::Small => false,
                Self::Exact(value) => value == other,
            }
        }
    }
    const SMALL: Value = Value::Small;

    fn extract_numbers(input: &str) -> Vec<Value> {
        let re = Regex::new(r"<?-?\d+(\.\d+)?").unwrap();
        let mut numbers = vec![];
        for cap in re.captures_iter(input) {
            if let Some(matched) = cap.get(0) {
                let s = matched.as_str();
                if s == "<1" {
                    numbers.push(Value::Small);
                } else if let Ok(num) = s.parse::<f32>() {
                    numbers.push(Value::Exact(num));
                }
            }
        }
        numbers
    }

    fn effect_values(state: &State, effect: Effect) -> (Vec<Value>, Vec<Value>) {
        println!("{:?}", effect);
        let effect = DisplayEffect {
            effect,
            is_unknown: false,
            ..Default::default()
        };
        let tip = effect.tip(state).unwrap();
        println!("  {}", tip.text);
        println!("  {}", tip.tip.text);
        (extract_numbers(&tip.text), extract_numbers(&tip.tip.text))
    }

    #[test]
    fn test_electrified_flag_tip() {
        // Formatting is in PWh per year (1.5e12 kWh/yr is 1.5 PWh/yr).
        let demand = OutputMap {
            fuel: 1.5e12,
            electricity: 2.5e12,
            ..Default::default()
        };

        // Expect 80% of fuel to go to electricity.
        // let expected_fuel =
        let tip = flag_tip(Flag::Electrified, &demand);
        println!("{}", tip.text);

        // Because of rounding these values will be
        // slightly different.
        let vals = extract_numbers(&tip.text);
        let prev_fuel_demand = vals[0];
        let next_fuel_demand = vals[1];
        let prev_elec_demand = vals[2];
        let next_elec_demand = vals[3];
        assert_eq!(prev_fuel_demand, 1.5);
        assert_eq!(next_fuel_demand, 0.3);
        assert_eq!(prev_elec_demand, 2.5);
        assert_eq!(next_elec_demand, 3.1);
    }

    #[test]
    fn test_vegan_flag_tip() {
        // Formatting is per 20,000 TCals.
        let demand = OutputMap {
            animal_calories: 1.5e9 * 2e4,
            plant_calories: 2.5e9 * 2e4,
            ..Default::default()
        };

        // Expect 80% of fuel to go to electricity.
        // let expected_fuel =
        let tip = flag_tip(Flag::Vegan, &demand);
        println!("{}", tip.text);

        let vals = extract_numbers(&tip.text);
        let prev_anim_demand = vals[0];
        let next_anim_demand = vals[1];
        let prev_plant_demand = vals[2];
        let next_plant_demand = vals[3];
        assert_eq!(prev_anim_demand, 1.5);
        assert_eq!(next_anim_demand, 0.15);
        assert_eq!(prev_plant_demand, 2.5);
        assert_eq!(next_plant_demand, 3.9);
    }

    #[test]
    fn test_vegetarian_flag_tip() {
        // Formatting is per 20,000 TCals.
        let demand = OutputMap {
            animal_calories: 3e9 * 2e4,
            plant_calories: 5e9 * 2e4,
            ..Default::default()
        };

        // Expect 80% of fuel to go to electricity.
        // let expected_fuel =
        let tip = flag_tip(Flag::Vegetarian, &demand);
        println!("{}", tip.text);

        let vals = extract_numbers(&tip.text);
        let prev_anim_demand = vals[0];
        let next_anim_demand = vals[1];
        let prev_plant_demand = vals[2];
        let next_plant_demand = vals[3];
        assert_eq!(prev_anim_demand, 3.);
        assert_eq!(next_anim_demand, 0.75);
        assert_eq!(prev_plant_demand, 5.);
        assert_eq!(next_plant_demand, 7.3);
    }

    #[test]
    fn test_world_variables() {
        let get_values = move |var: WorldVariable, amount: f32| {
            let state = State::default();
            effect_values(&state, Effect::WorldVariable(var, amount))
        };

        let (text_vals, _) = get_values(WorldVariable::Outlook, 12.);
        assert_eq!(text_vals[0], 12.);

        let (text_vals, tip_vals) = get_values(WorldVariable::Emissions, 2.5);
        assert_eq!(text_vals[0], 2.5);
        assert_eq!(tip_vals[0], 2.5);
        assert_eq!(tip_vals[1], 5.0);

        let (text_vals, tip_vals) = get_values(WorldVariable::ExtinctionRate, 7.);
        assert_eq!(text_vals[0], 7.);
        assert_eq!(tip_vals[0], 99.0);

        let (text_vals, tip_vals) = get_values(WorldVariable::Temperature, 1.5);
        assert_eq!(text_vals[0], 1.5);
        assert_eq!(tip_vals[0], 1.5);

        let (text_vals, tip_vals) = get_values(WorldVariable::Precipitation, 0.2);
        assert_eq!(text_vals[0], 0.2);
        assert_eq!(tip_vals[0], 0.2);

        let (text_vals, _) = get_values(WorldVariable::PopulationGrowth, 0.5);
        assert_eq!(text_vals[0], 50.);

        let (text_vals, _) = get_values(WorldVariable::Population, 120.);
        assert_eq!(text_vals[0], 120.);

        let (text_vals, tip_vals) = get_values(WorldVariable::SeaLevelRiseRate, 0.5);
        assert_eq!(text_vals[0], 500.);
        assert_eq!(tip_vals[0], 0.09);

        let (text_vals, tip_vals) = get_values(WorldVariable::SeaLevelRise, 0.25);
        assert_eq!(text_vals[0], 250.);
        assert_eq!(tip_vals[0], 0.09);
    }

    #[test]
    fn test_process_limit() {
        let state = State::default();
        let process_id = state.world.processes.by_idx(3).id;
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::ProcessLimit(process_id, 1.539e15));
        assert_eq!(text_vals[0], 50.);
        assert_eq!(tip_vals[0], 50.);
    }

    #[test]
    fn test_regional_habitability() {
        let state = State::default();
        let (text_vals, _) =
            effect_values(&state, Effect::RegionHabitability(Latitude::Tropic, 3.));
        assert_eq!(text_vals[0], 3.);
    }

    #[test]
    fn test_resource() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(&state, Effect::Resource(Resource::Water, 1e15));
        assert_eq!(text_vals[0], 2.);
        assert_eq!(tip_vals[0], 2.);
    }

    #[test]
    fn test_output() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(&state, Effect::Output(Output::Fuel, 0.2));
        assert_eq!(text_vals[0], 20.);
        assert_eq!(tip_vals[0], 79.0);
        assert_eq!(tip_vals[1], 95.0);
    }

    #[test]
    fn test_output_for_process() {
        let state = State::default();
        let process_id = state.world.processes.by_idx(7).id;
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::OutputForProcess(process_id, 0.2));
        assert_eq!(text_vals[0], 20.);
        assert_eq!(tip_vals[0], 20.);
    }

    #[test]
    fn test_output_for_feature() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::OutputForFeature(ProcessFeature::UsesLivestock, 0.25),
        );
        assert_eq!(text_vals[0], 25.);
        assert_eq!(tip_vals[0], 25.);
    }

    #[test]
    fn test_co2_for_feature() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::CO2ForFeature(ProcessFeature::UsesLivestock, 0.15),
        );
        assert_eq!(text_vals[1], 15.);
        assert_eq!(tip_vals[1], 15.);
    }

    #[test]
    fn test_biodiversity_pressure_for_feature() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::BiodiversityPressureForFeature(ProcessFeature::UsesLivestock, 2.),
        );
        assert_eq!(text_vals[0], 2.);
        assert_eq!(tip_vals[0], 2.);
    }

    #[test]
    fn test_demand() {
        let state = State::default();
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::Demand(Output::Electricity, 0.25));
        assert_eq!(text_vals[0], 25.);
        assert_eq!(tip_vals[0], 26.0);
        assert_eq!(tip_vals[1], 33.0);
    }

    #[test]
    fn test_demand_amount() {
        let state = State::default();
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::DemandAmount(Output::Electricity, 3e12));
        assert_eq!(text_vals[0], 3.0);
        assert_eq!(tip_vals[0], 26.0);
        assert_eq!(tip_vals[1], 29.0);
        assert_eq!(tip_vals[2], 12.);
    }

    #[test]
    fn test_project_cost_modifier() {
        let state = State::default();

        let project = state
            .world
            .projects
            .iter()
            .find(|p| p.kind == ProjectType::Initiative)
            .unwrap();
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::ProjectCostModifier(project.id, 0.5));
        assert_eq!(text_vals[0], 50.);
        assert_eq!(tip_vals[0], 50.);

        let project = state
            .world
            .projects
            .iter()
            .find(|p| p.kind == ProjectType::Research)
            .unwrap();
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::ProjectCostModifier(project.id, 0.25));
        assert_eq!(text_vals[0], 25.);
        assert_eq!(tip_vals[0], 25.);

        let project = state
            .world
            .projects
            .iter()
            .find(|p| p.kind == ProjectType::Policy)
            .unwrap();
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::ProjectCostModifier(project.id, 2.));
        assert_eq!(text_vals[0], 200.);
        assert_eq!(tip_vals[0], 200.);
    }

    #[test]
    fn test_modify_industry_demand() {
        let state = State::default();
        let industry_id = state.world.industries.by_idx(2).id;
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::ModifyIndustryDemand(industry_id, 0.05));
        assert_eq!(text_vals[0], 5.);
        assert_eq!(tip_vals[0], 5.);
    }

    #[test]
    fn test_modify_industry_resources() {
        let state = State::default();
        let industry_id = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Shipping")
            .unwrap()
            .id;
        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::ModifyIndustryResources(industry_id, Resource::Fuel, 0.2),
        );
        assert_eq!(text_vals[0], 20.);
        assert_eq!(tip_vals[0], 3.9);
        assert_eq!(tip_vals[1], 4.7);
        assert_eq!(tip_vals[2], 1.);
    }

    #[test]
    fn test_modify_industry_resources_amount() {
        let state = State::default();
        let industry_id = state
            .world
            .industries
            .iter()
            .find(|i| i.name == "Shipping")
            .unwrap()
            .id;
        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::ModifyIndustryResourcesAmount(industry_id, Resource::Fuel, 1000.),
        );
        assert_eq!(text_vals[0], 42.0);
        assert_eq!(tip_vals[0], 3.9);
        assert_eq!(tip_vals[1], 46.0);
        assert_eq!(tip_vals[2], 54.);
    }

    #[test]
    fn test_modify_industry_byproducts() {
        let mut state = State::default();

        // The concrete industry, which has direct CO2 emissions. But even
        // then we need to increase direct emissions to have a more
        // noticeable effect.
        let industry = state
            .world
            .industries
            .iter_mut()
            .find(|i| i.name == "Concrete")
            .unwrap();
        let industry_id = industry.id;
        industry.byproducts.co2 *= 10000.;

        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::ModifyIndustryByproducts(industry_id, Byproduct::Co2, 0.6),
        );
        assert_eq!(text_vals[1], 60.);
        assert_eq!(tip_vals[0], 16497.);
        assert_eq!(tip_vals[1], 26396.);
        assert_eq!(tip_vals[2], 17907.);
    }

    #[test]
    fn test_modify_process_byproducts() {
        let state = State::default();
        let process_id = state
            .world
            .processes
            .iter()
            .find(|p| p.name == "Coal")
            .unwrap()
            .id;
        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::ModifyProcessByproducts(process_id, Byproduct::Co2, 0.6),
        );
        assert_eq!(text_vals[1], 60.);
        assert_eq!(tip_vals[0], 3.4);
        assert_eq!(tip_vals[1], 5.4);
        assert_eq!(tip_vals[2], 4.);

        let (text_vals, tip_vals) = effect_values(
            &state,
            Effect::ModifyProcessByproducts(process_id, Byproduct::Biodiversity, 0.6),
        );
        assert_eq!(text_vals[0], 60.);
        assert_eq!(tip_vals[0], 1.);
        assert_eq!(tip_vals[1], 1.6);
    }

    #[test]
    fn test_demand_outlook_change() {
        let state = State::default();
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::DemandOutlookChange(Output::Fuel, 1.));
        assert_eq!(text_vals[0], 3.);
        assert_eq!(tip_vals[0], 30.);
        assert_eq!(tip_vals[1], 40.);
    }

    #[test]
    fn test_income_outlook_change() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(&state, Effect::IncomeOutlookChange(3.));
        assert_eq!(text_vals[0], 5.);
        assert_eq!(tip_vals[0], 30.);
        assert_eq!(tip_vals[1], 40.);
    }

    #[test]
    fn test_modify_event_probability() {
        let state = State::default();
        let event_id = state.world.events.by_idx(15).id;
        let (text_vals, tip_vals) =
            effect_values(&state, Effect::ModifyEventProbability(event_id, 0.2));
        assert_eq!(text_vals[0], 20.);
        assert_eq!(tip_vals[0], 20.);
    }

    #[test]
    fn test_protect_land() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(&state, Effect::ProtectLand(0.25));
        assert_eq!(text_vals[0], 25.);
        assert_eq!(tip_vals[0], 10.);
        assert_eq!(tip_vals[1], 35.);
    }

    #[test]
    fn test_feedstock() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(&state, Effect::Feedstock(Feedstock::Oil, 7e15));
        assert_eq!(text_vals[0], 863.);
        assert_eq!(tip_vals[0], 60.);
    }

    #[test]
    fn test_termination_shock() {
        let state = State::default();
        let (text_vals, tip_vals) = effect_values(&state, Effect::TerminationShock);
        assert_eq!(text_vals[0], 0.5);
        assert_eq!(tip_vals[0], 0.5);
    }
}

#[cfg(test)]
mod card_length {
    //! Cards and outcome screens have to fit on a phone: the effects text
    //! for every project, outcome and upgrade level, in every language the
    //! game ships with, stays under a cap.
    use super::*;

    /// Visible characters on a phone-width card before it gets unwieldy
    /// (about ten lines). Outcome screens scroll past this, but cards
    /// shrink their text to fit, which gets hard to read.
    const MAX_VISIBLE_CHARS: usize = 450;

    /// Visible text, without bbcode tags; an icon counts as one character.
    fn visible(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        let mut in_icon = false;
        while let Some(c) = chars.next() {
            if c == '[' {
                let tag: String = chars.by_ref().take_while(|c| *c != ']').collect();
                if tag == "i" {
                    in_icon = true;
                    out.push('#');
                }
                if tag == "/i" {
                    in_icon = false;
                }
                continue;
            }
            if !in_icon {
                out.push(c);
            }
        }
        out
    }

    /// Each set of effects shown together: a project's own effects plus
    /// one of its outcomes, and each upgrade level.
    fn effect_sets(state: &State) -> Vec<(String, Vec<Effect>)> {
        let mut sets = vec![];
        for p in state.world.projects.iter() {
            let outcomes: Vec<Vec<Effect>> = if p.outcomes.is_empty() {
                vec![vec![]]
            } else {
                p.outcomes.iter().map(|o| o.effects.clone()).collect()
            };
            for (i, o) in outcomes.iter().enumerate() {
                let mut effects = p.effects.clone();
                effects.extend(o.iter().cloned());
                sets.push((format!("{} (outcome {})", p.name, i + 1), effects));
            }
            for (i, u) in p.upgrades.iter().enumerate() {
                sets.push((format!("{} (level {})", p.name, i + 2), u.effects.clone()));
            }
        }
        sets
    }

    fn text_length(state: &State, effects: &[Effect]) -> usize {
        let effects: Vec<DisplayEffect> = effects.iter().map(DisplayEffect::from).collect();
        let lines: usize = effect_lines(state, &effects)
            .iter()
            .map(|t| visible(&t.text).chars().count())
            .sum();
        let summary = land_summary(state, &effects).map_or(0, |s| s.chars().count() + 1);
        lines + summary
    }

    #[test]
    fn card_effects_fit_on_a_phone_in_every_language() {
        // Switching languages is global, and tests run in parallel; do the
        // check in a child process so it can't change other tests' text.
        const CHILD: &str = "HES_CARD_LENGTH_CHILD";
        if std::env::var(CHILD).is_err() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "display::effects::card_length::card_effects_fit_on_a_phone_in_every_language",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let state = State::default();
        let mut locales: Vec<String> = rust_i18n::available_locales!()
            .into_iter()
            .map(|l| l.to_string())
            .collect();
        locales.push("en".into());
        let mut too_long = vec![];
        for locale in &locales {
            rust_i18n::set_locale(locale);
            for (name, effects) in effect_sets(&state) {
                let len = text_length(&state, &effects);
                if len > MAX_VISIBLE_CHARS {
                    too_long.push(format!("[{locale}] {name}: {len} characters"));
                }
            }
        }
        rust_i18n::set_locale("en");
        assert!(
            too_long.is_empty(),
            "Card text over {MAX_VISIBLE_CHARS} characters:\n{}",
            too_long.join("\n")
        );
    }
}
