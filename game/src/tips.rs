use egui::{Color32, LayerId, Order, PopupAnchor, PopupCloseBehavior};
use egui_taffy::TuiBuilderLogic;
use hes_engine::{Industry, NPC, Process, Project};

use crate::{
    display::{DisplayEvent, Icon, icons},
    parts::{glow_fill, h_center, raised_frame},
    state::GameState,
    text::bbcode,
    views::{Card, Cards, Changes, FactorsCard, render_event_card},
};

#[derive(Clone)]
pub struct Tip {
    pub text: String,
    icon: Icon,
    card: Option<TipCard>,
}
impl Tip {
    pub fn card(mut self, card: impl Into<TipCard>) -> Self {
        self.card = Some(card.into());
        self
    }

    /// A tip displayed on hover (rather than on click).
    /// Doesn't support cards.
    fn render_as_hover(&self, ui: &mut egui::Ui) {
        raised_frame().shadow().show(ui, |ui| {
            set_tip_width(ui);
            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            ui.horizontal_top(|ui| {
                ui.add(self.icon.size(24.));
                ui.add(bbcode(&self.text));
            });
        });
    }

    pub fn render(&mut self, ui: &mut egui::Ui, state: &mut GameState) -> bool {
        let mut clicked_outside = false;
        if !self.text.is_empty() {
            h_center(ui, "tip", |tui| {
                tui.ui(|ui| {
                    let resp = raised_frame().shadow().show(ui, |ui| {
                        set_tip_width(ui);
                        ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
                        ui.horizontal_top(|ui| {
                            ui.add(self.icon.size(24.));
                            ui.add(bbcode(&self.text));
                        });
                    });
                    clicked_outside = resp.clicked_elsewhere();
                });
            });
        }

        if let Some(card) = &mut self.card {
            ui.add_space(24.);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            h_center(ui, "tip-card", |tui| {
                tui.ui(|ui| {
                    let resp = match card {
                        TipCard::Project(card) => card.render(ui, state, false),
                        TipCard::Process(card) => card.render(ui, state, false),
                        TipCard::Processes(cards) => {
                            // TODO horizontal scrolling, but seems to interfere with taffy
                            ui.horizontal(|ui| {
                                ui.style_mut().spacing.item_spacing.x = 32.;
                                ui.set_width(ui.available_width());
                                for card in cards {
                                    card.render(ui, state, false);
                                }
                            })
                            .response
                        }
                        TipCard::Industry(card) => card.render(ui, state, false),
                        TipCard::Factors(factors_card) => {
                            set_tip_width(ui);
                            factors_card.render(ui)
                        }
                        TipCard::Event(event) => {
                            set_tip_width(ui);
                            render_event_card(ui, state, event)
                        }
                        TipCard::NPC(card) => card.render(ui, state, false),
                        TipCard::Changes(changes) => ui.add(&(*changes)),
                        TipCard::Storage => {
                            set_tip_width(ui);
                            render_storage_card(ui, state)
                        }
                        TipCard::Contentedness(region) => {
                            set_tip_width(ui);
                            render_contentedness_card(ui, state, *region)
                        }
                        TipCard::EditableProject(cards) => {
                            ui.push_id("editable-project", |ui| {
                                ui.set_height(ui.ctx().content_rect().height().min(640.));
                                set_tip_width(ui);
                                cards.render(ui, state)
                            })
                            .response
                        }
                    };
                    if !clicked_outside {
                        clicked_outside = resp.clicked_elsewhere();
                    }
                });
            });
        }

        clicked_outside
    }
}

/// Define a tooltip.
pub fn tip(icon: Icon, text: impl Into<String>) -> Tip {
    Tip {
        icon,
        text: text.into(),
        card: None,
    }
}

/// Add a tooltip displayed when the response is clicked.
pub fn add_tip(tip: Tip, resp: egui::Response) -> egui::Response {
    let rect = resp.rect;
    if resp.contains_pointer() {
        let painter = resp.ctx.layer_painter(resp.layer_id);
        glow_fill(&painter, rect, Color32::WHITE);
    }

    if resp.interact(egui::Sense::click()).clicked() {
        open_tip(&resp, tip);
    }
    resp
}

pub fn add_editable_project_card(project: Project, resp: egui::Response) -> egui::Response {
    if resp.interact(egui::Sense::click()).clicked() {
        let mut tip = tip(icons::OTHER, "");
        tip.card = Some(TipCard::editable_project(project));
        open_tip(&resp, tip);
    }
    resp
}

/// Add a hover tooltip to a response.
pub fn add_hover_tip(tip: Tip, resp: egui::Response) -> egui::Response {
    if resp.contains_pointer() {
        egui::Popup::from_response(&resp)
            .frame(egui::Frame::NONE)
            .gap(8.)
            .show(|ui| {
                tip.render_as_hover(ui);
            });
    }
    resp
}

fn set_tip_width(ui: &mut egui::Ui) {
    let width = (ui.ctx().content_rect().width() - 36.).min(420.);
    ui.set_max_width(width);
}

fn open_tip(resp: &egui::Response, tip: Tip) {
    let popup_id = egui::Id::new("tip");
    let opened_id = popup_id.with("just-opened");
    let card_id = popup_id.with("card");

    egui::Popup::open_id(&resp.ctx, popup_id);
    resp.ctx.memory_mut(|mem| {
        // Need to track if the tip was opened this frame
        // so that the click doesn't immediately close it
        // the same frame.
        mem.data.insert_temp(opened_id, true);

        // Track the current tip card separately, as
        // we want to continue to display it if clicking
        // tips within the current tip.
        // E.g. if I have a tip open for `Solar PV` and
        // I click on an NPC in the card, I want the `Solar PV`
        // card to remain visible.
        if let Some(card) = &tip.card {
            mem.data.insert_temp(card_id, card.clone());
        }

        mem.data.insert_temp(popup_id, tip);
    });
}

pub fn tip_is_open(ctx: &egui::Context) -> bool {
    let popup_id = egui::Id::new("tip");
    egui::Popup::is_id_open(ctx, popup_id)
}

/// Render the currently-open tip, if any.
pub fn render_tip(ctx: &egui::Context, state: &mut GameState) {
    let popup_id = egui::Id::new("tip");
    let opened_id = popup_id.with("just-opened");
    let card_id = popup_id.with("card");

    if let Some(mut tip) = ctx.memory(|mem| mem.data.get_temp::<Tip>(popup_id)) {
        let screen_size = ctx.content_rect().size();
        egui::Popup::new(
            popup_id,
            ctx.clone(),
            PopupAnchor::Position(egui::Pos2::ZERO),
            LayerId::new(Order::Tooltip, popup_id),
        )
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .open_memory(None)
        .width(screen_size.x)
        .frame(
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(220))
                .inner_margin(egui::Margin::symmetric(0, 18)),
        )
        .show(|ui| {
            // Overlay takes up full screen.
            ui.set_max_size(screen_size); // Needed for resizing
            ui.set_width(ui.available_width());
            ui.set_height(screen_size.y);

            if let Some(card) = ctx.memory(|mem| mem.data.get_temp::<TipCard>(card_id)) {
                tip.card = Some(card);
            }

            // Close only if the tip wasn't just opened this frame.
            // Otherwise this will register the click that opened the tip
            // as a click that will close it immediately.
            let should_close = tip.render(ui, state);
            let just_opened = ctx
                .memory(|mem| mem.data.get_temp::<bool>(opened_id))
                .unwrap_or_default();
            if should_close && !just_opened {
                // Close and clear the current tip card.
                egui::Popup::close_id(ctx, popup_id);
                ctx.memory_mut(|mem| {
                    mem.data.remove::<TipCard>(card_id);
                });
            } else if let Some(card) = &tip.card {
                ctx.memory_mut(|mem| {
                    mem.data.insert_temp(card_id, card.clone());
                });
            }

            // No longer just-opened this frame.
            ctx.memory_mut(|mem| {
                mem.data.insert_temp(opened_id, false);
            });
        });
    }
}

#[derive(Clone)]
pub enum TipCard {
    Project(Card<Project>),
    Process(Card<Process>),
    Processes(Vec<Card<Process>>),
    Industry(Card<Industry>),
    Factors(FactorsCard),
    Event(DisplayEvent),
    NPC(Card<NPC>),
    Changes(Changes),
    EditableProject(Cards<Project>),
    /// Grid storage and lithium.
    Storage,
    /// What's behind contentedness: one region's breakdown (if given) and
    /// an overview by income level.
    Contentedness(Option<hes_engine::Id>),
}
impl From<Project> for TipCard {
    fn from(value: Project) -> Self {
        TipCard::Project(Card::new(value))
    }
}
impl From<Process> for TipCard {
    fn from(value: Process) -> Self {
        TipCard::Process(Card::new(value))
    }
}
impl From<NPC> for TipCard {
    fn from(value: NPC) -> Self {
        TipCard::NPC(Card::new(value))
    }
}
impl From<Industry> for TipCard {
    fn from(value: Industry) -> Self {
        TipCard::Industry(Card::new(value))
    }
}
impl From<Vec<Process>> for TipCard {
    fn from(value: Vec<Process>) -> Self {
        TipCard::Processes(value.into_iter().map(Card::new).collect())
    }
}
impl From<FactorsCard> for TipCard {
    fn from(value: FactorsCard) -> Self {
        TipCard::Factors(value)
    }
}
impl From<DisplayEvent> for TipCard {
    fn from(value: DisplayEvent) -> Self {
        TipCard::Event(value)
    }
}
impl From<Changes> for TipCard {
    fn from(value: Changes) -> Self {
        TipCard::Changes(value)
    }
}
impl TipCard {
    pub fn editable_project(value: Project) -> Self {
        TipCard::EditableProject(Cards::new(std::iter::once(value)))
    }
}

fn outlook_source_name(source: &hes_engine::OutlookSource) -> String {
    use hes_engine::OutlookSource;
    use rust_i18n::t;
    match source {
        OutlookSource::Start => t!("Starting contentedness").to_string(),
        OutlookSource::Temperature => t!("Temperature change").to_string(),
        OutlookSource::Project(name) => t!(name.as_str()).to_string(),
        OutlookSource::Fatigue(name) => format!("{} ({})", t!(name.as_str()), t!("fatigue")),
        OutlookSource::Event(name) => t!(name.as_str()).to_string(),
        OutlookSource::Recovery => t!("Recovery over time & other").to_string(),
    }
}

/// Unrest a region is in, and what the next step would take.
fn unrest_status(region: &hes_engine::Region) -> (String, String) {
    use hes_engine::RegionFlag;
    use rust_i18n::t;
    let has = |f: RegionFlag| region.flags.contains(&f);
    if region.seceded {
        (t!("Seceded").to_string(), String::new())
    } else if has(RegionFlag::Revolts) {
        (
            t!("In revolt").to_string(),
            t!("At 0 or below, it secedes 5 years later.").to_string(),
        )
    } else if has(RegionFlag::Riots) {
        (
            t!("Rioting").to_string(),
            t!("Revolts can start at 1 or below, and will at 0 or below.").to_string(),
        )
    } else if has(RegionFlag::Protests) {
        (
            t!("Protesting").to_string(),
            t!("Riots are likely at 2 or below, and certain at 1 or below.").to_string(),
        )
    } else {
        (
            t!("Calm").to_string(),
            t!("Protests can start at 4 or below, are likely at 3, and certain at 2.").to_string(),
        )
    }
}

/// Contentedness by region and income level.
fn render_contentedness_card(
    ui: &mut egui::Ui,
    state: &GameState,
    region_id: Option<hes_engine::Id>,
) -> egui::Response {
    use crate::display::AsText;
    use hes_engine::{Income, OutlookSource, RegionFlag};
    use rust_i18n::t;
    use strum::IntoEnumIterator;

    egui::Frame::NONE
        .inner_margin(8)
        .fill(Color32::from_black_alpha(160))
        .corner_radius(4)
        .show(ui, |ui| {
            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let row = |ui: &mut egui::Ui, label: String, value: String| {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(value).strong());
                    });
                });
            };
            let small = |ui: &mut egui::Ui, text: String| {
                ui.label(egui::RichText::new(text).size(11.).color(Color32::from_gray(190)));
            };

            if let Some(id) = region_id {
                let region = &state.world.regions[&id];
                ui.horizontal(|ui| {
                    ui.add(icons::CONTENTEDNESS.size(24.));
                    ui.label(egui::RichText::new(t!(&region.name)).size(18.));
                });
                let (status, next) = unrest_status(region);
                row(ui, t!("Contentedness").to_string(), format!("{:.1}", region.outlook));
                row(ui, t!("Status").to_string(), status);
                if !next.is_empty() {
                    small(ui, next);
                }
                ui.separator();
                for (source, amount) in state.region_outlook_breakdown(&id) {
                    let value = if source == OutlookSource::Start {
                        format!("{amount:.0}")
                    } else {
                        format!("{amount:+.1}")
                    };
                    row(ui, outlook_source_name(&source), value);
                }
                ui.separator();
            }

            ui.horizontal(|ui| {
                ui.add(icons::CONTENTEDNESS.size(20.));
                ui.label(egui::RichText::new(t!("By income level")).size(16.));
            });
            for income in Income::iter().rev() {
                let regions: Vec<_> = state
                    .world
                    .regions
                    .iter()
                    .filter(|r| r.income == income && !r.seceded)
                    .collect();
                if regions.is_empty() {
                    continue;
                }
                let n = regions.len() as f32;
                let avg = regions.iter().map(|r| r.outlook).sum::<f32>() / n;
                let count = |f: RegionFlag| regions.iter().filter(|r| r.flags.contains(&f)).count();
                ui.add_space(4.);
                row(
                    ui,
                    format!("{} ({})", t!(income.title()), regions.len()),
                    format!("{avg:.1}"),
                );
                let (protests, riots, revolts) =
                    (count(RegionFlag::Protests), count(RegionFlag::Riots), count(RegionFlag::Revolts));
                if protests + riots + revolts > 0 {
                    small(
                        ui,
                        t!(
                            "%{protests} protesting, %{riots} rioting, %{revolts} in revolt",
                            protests = protests,
                            riots = riots,
                            revolts = revolts
                        )
                        .to_string(),
                    );
                }

                // The biggest average drivers, other than the starting value.
                let mut totals: Vec<(OutlookSource, f32)> = vec![];
                for region in &regions {
                    for (source, amount) in state.region_outlook_breakdown(&region.id) {
                        if source == OutlookSource::Start {
                            continue;
                        }
                        match totals.iter_mut().find(|(s, _)| *s == source) {
                            Some((_, total)) => *total += amount,
                            None => totals.push((source, amount)),
                        }
                    }
                }
                totals.sort_by(|a, b| a.1.abs().total_cmp(&b.1.abs()).reverse());
                for (source, total) in totals.into_iter().take(4) {
                    let avg = total / n;
                    if avg.abs() >= 0.1 {
                        row(ui, format!("  {}", outlook_source_name(&source)), format!("{avg:+.1}"));
                    }
                }
            }
            ui.add_space(4.);
            small(ui, t!("Every region starts at 10 and slowly recovers toward it. Policies that cost contentedness per income level hit richer regions harder.").to_string());
        })
        .response
}

/// A summary of grid storage and lithium supply.
fn render_storage_card(ui: &mut egui::Ui, state: &GameState) -> egui::Response {
    use hes_engine::{Feedstock, storage::StorageMix};
    use rust_i18n::t;

    let storage = &state.storage;
    let report = storage.last;
    let reserves = state.feedstocks.available[Feedstock::Lithium];
    let years = state.feedstocks.until_exhaustion(Feedstock::Lithium);
    let mix = StorageMix::new(state.storage_tech());

    egui::Frame::NONE
        .inner_margin(8)
        .fill(Color32::from_black_alpha(160))
        .corner_radius(4)
        .show(ui, |ui| {
            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.horizontal(|ui| {
                ui.add(icons::LITHIUM.size(24.));
                ui.label(egui::RichText::new(t!("Storage & Lithium")).size(18.));
            });
            ui.add_space(4.);
            let row = |ui: &mut egui::Ui, label: String, value: String| {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(value).strong());
                    });
                });
            };
            let li = |g: f32| {
                if g.abs() >= 1e12 {
                    format!("{:.1} Mt", g * 1e-12)
                } else {
                    format!("{:.0} kt", g * 1e-9)
                }
            };
            let heading = |ui: &mut egui::Ui, text: String| {
                ui.label(egui::RichText::new(text).strong().color(Color32::from_gray(200)));
            };

            // The plan, over the next planning cycle.
            let outlook = {
                use crate::views::scanner::process::{planned_electricity, planned_intermittent_share};
                let changes = &state.ui.process_mix_changes;
                let electricity = planned_electricity(&state.core, changes);
                (state.storage_outlook(planned_intermittent_share(&state.core, changes), electricity), electricity)
            };
            let (outlook, electricity) = outlook;
            heading(ui, t!("Next five years, as planned").to_string());
            row(ui, t!("Wind & solar share of electricity").to_string(), format!("{:.0}%", outlook.intermittent_share * 100.));
            row(ui, t!("Electricity demand").to_string(), crate::display::format_power(electricity));
            row(ui, t!("Grid batteries needed").to_string(), format!("{:.1} TWh", outlook.grid_capacity * 1e-9));
            row(ui, t!("Lithium the grid needs").to_string(), li(outlook.grid_lithium));
            row(ui, t!("Lithium the grid can get").to_string(), li(outlook.for_grid));
            row(ui, t!("Wind & solar this supports").to_string(), format!("{:.0}%", outlook.supported_share * 100.));
            row(ui, t!("Lithium vehicles need").to_string(), li(outlook.ev_lithium));
            row(ui, t!("Vehicles that can go electric").to_string(), format!("{:.0}%", outlook.ev_coverage * 100.));
            ui.label(
                egui::RichText::new(t!("The grid gets lithium first: batteries in use, the stockpile and five years of mining. Vehicles get what's left; those that can't get batteries keep running on fuel."))
                    .size(11.)
                    .color(Color32::from_gray(190)),
            );

            ui.separator();
            heading(ui, t!("This year").to_string());
            row(ui, t!("Wind & solar share of electricity").to_string(), format!("{:.0}%", state.intermittent_share() * 100.));
            row(ui, t!("Energy passed through storage").to_string(), crate::display::format_power(report.stored));
            row(ui, t!("Storage needed that's built").to_string(), format!("{:.0}%", (1. - report.shortfall) * 100.));
            row(ui, t!("Lost in storage round trips").to_string(), crate::display::format_power(report.losses));
            row(ui, t!("Undelivered for lack of storage").to_string(), crate::display::format_power(report.curtailed));
            if state.flags.contains(&hes_engine::Flag::ScheduledBlackouts) {
                let cut = hes_engine::storage::scheduled_cut(
                    state.produced.amount.electricity,
                    state.intermittent_share(),
                    state.storage_tech(),
                );
                row(ui, t!("Cut by scheduled blackouts").to_string(), crate::display::format_power(cut));
            }
            row(ui, t!("Lithium in grid batteries").to_string(), format!("{} / {}", li(storage.grid_in_use), li(report.grid_lithium_needed)));
            row(ui, t!("Lithium in vehicle batteries").to_string(), format!("{} / {}", li(storage.lithium_in_use - storage.grid_in_use), li(report.ev_lithium_needed)));
            row(ui, t!("Vehicles still on fuel for lack of batteries").to_string(), format!("{:.0}%", report.ev_shortfall * 100.));
            row(ui, t!("Lithium stockpile").to_string(), li(storage.stockpile));
            row(ui, t!("Lithium mined").to_string(), li(report.lithium_mined));
            row(ui, t!("Mining capacity per year").to_string(), li(storage.mining_capacity));
            row(ui, t!("Recovered by recycling").to_string(), format!("{:.0}%", storage.recycling.clamp(0., 1.) * 100.));
            row(
                ui,
                t!("Reserves left").to_string(),
                if years.is_finite() {
                    format!("{} ({years:.0} yrs)", li(reserves))
                } else {
                    li(reserves)
                },
            );
            ui.separator();
            ui.label(t!("Grid storage mix"));
            for (name, share) in [
                (t!("Lithium-ion batteries"), mix.lithium),
                (t!("Sodium-ion batteries"), mix.sodium),
                (t!("Pumped hydro"), mix.pumped_hydro),
                (t!("Long-duration storage"), mix.long_duration),
            ] {
                if share > 0. {
                    row(ui, format!("  {name}"), format!("{:.0}%", share * 100.));
                }
            }
        })
        .response
}
