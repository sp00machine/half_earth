use std::{borrow::Cow, collections::BTreeMap};

use egui::{Color32, CornerRadius, Margin, Sense, TextWrapMode, emath::OrderedFloat};
use egui_taffy::TuiBuilderLogic;
use enum_map::EnumMap;
use hes_engine::{Id, Output, Process, Resource, State};
use numfmt::{Formatter, Precision, Scales};
use rust_i18n::t;
use strum::IntoEnumIterator;

use crate::{
    display::{self, AsText, DisplayValue, HasIcon, Icon, factors::factors_card, icons, intensity},
    parts::{
        calc_text_width, get_sizing, h_center, h_center_top, overlay, raised_frame,
        set_full_bg_image,
    },
    state::{FACTORS, StateExt},
    text::{scale_text, scale_text_styles},
    tips::{Tip, add_tip, tip},
    vars::Var,
};

use super::treemap::{TreeItem, treemap};

impl Var {
    pub fn color(&self) -> [u32; 2] {
        match self {
            Var::Land => [0xB7FF7A, 0x0E681F],
            Var::Water => [0x7DE1EF, 0x4560FF],
            Var::Energy => [0xFDCE4C, 0xE81224],
            Var::Emissions => [0xF2F7E2, 0x6CB30B],
            Var::Biodiversity => [0xEA8BCF, 0x6865F8],
            Var::Electricity => [0xFFFF1A, 0xFF8C1A],
            Var::Fuel => [0xF7F6C7, 0xD3753F],
            Var::AnimalCalories => [0xF8AD72, 0xCA5704],
            Var::PlantCalories => [0xB1EF8F, 0x06CA9B],
            Var::Contentedness => [0x000000, 0xFFFFFF],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum HistoryMetric {
    Temperature,
    Emissions,
    Extinction,
    LandUse,
}

pub struct Stats {
    breakdown_factor: Var,
    show_breakdown_menu: bool,
    history_metric: HistoryMetric,
}
impl Stats {
    pub fn new() -> Self {
        Self {
            breakdown_factor: Var::Land,
            show_breakdown_menu: false,
            history_metric: HistoryMetric::Temperature,
        }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        process_mix_changes: &EnumMap<Output, BTreeMap<Id, isize>>,
    ) {
        set_full_bg_image(
            ui,
            hes_images::background_image("dashboard.png"),
            egui::vec2(1600., 1192.),
        );
        ui.style_mut().visuals.override_text_color = Some(Color32::BLACK);

        let demand_for_outputs: EnumMap<Output, f32> = Output::iter()
            .map(|output| (output, state.output_demand.of(output)))
            .collect();

        let process_changes = state
            .world
            .processes
            .iter()
            .filter(|p| !p.locked)
            .filter_map(move |p| {
                let mix_change =
                    (*process_mix_changes[p.output].get(&p.id).unwrap_or(&0)) as f32 * 0.05;
                if mix_change != 0. {
                    let change = mix_change * demand_for_outputs[p.output];
                    Some((p.clone(), change))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        ui.vertical_centered(|ui| {
            ui.add_space(32.);
            ui.set_max_width(720.);

            let sizing = get_sizing(ui);
            let screen_width = ui.ctx().content_rect().width();
            if sizing.is_small {
                h_center_top(ui, "stats-top", |tui| {
                    tui.ui(|ui| {
                        stat(ui, |ui| render_temp(ui, state), t!("Temp. Anomaly"));
                    });
                    tui.ui(|ui| {
                        render_emissions(ui, state, &process_changes);
                    });
                });

                ui.add_space(32.);

                h_center_top(ui, "stats-middle", |tui| {
                    tui.ui(|ui| {
                        render_land(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_energy(ui, state, &process_changes);
                    });
                });

                ui.add_space(32.);

                h_center_top(ui, "stats-middle-2", |tui| {
                    tui.ui(|ui| {
                        render_water(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_biodiversity(ui, state, &process_changes);
                    });
                });

                ui.add_space(32.);

                h_center_top(ui, "stats-bottom", |tui| {
                    tui.ui(|ui| {
                        stat(
                            ui,
                            |ui| render_sea_level_rise(ui, state),
                            t!("Sea Level Rise"),
                        );
                    });
                    tui.ui(|ui| {
                        stat(ui, |ui| render_population(ui, state), t!("Population"));
                    });
                });

                ui.add_space(32.);

                h_center_top(ui, "stats-bottom-2", |tui| {
                    tui.ui(|ui| {
                        stat(
                            ui,
                            |ui| render_income(ui, state),
                            t!("Avg. Living Standards"),
                        );
                    });
                    tui.ui(|ui| {
                        stat(
                            ui,
                            |ui| render_habitability(ui, state),
                            t!("Avg. Habitability"),
                        );
                    });
                });
            } else if screen_width <= 720. {
                h_center_top(ui, "stats-top", |tui| {
                    tui.ui(|ui| {
                        stat(ui, |ui| render_temp(ui, state), t!("Temp. Anomaly"));
                    });
                    tui.ui(|ui| {
                        render_emissions(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_land(ui, state, &process_changes);
                    });
                });

                ui.add_space(32.);

                h_center_top(ui, "stats-middle", |tui| {
                    tui.ui(|ui| {
                        render_energy(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_water(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_biodiversity(ui, state, &process_changes);
                    });
                });

                ui.add_space(32.);

                if screen_width <= 520. {
                    h_center_top(ui, "stats-bottom", |tui| {
                        tui.ui(|ui| {
                            stat(
                                ui,
                                |ui| render_sea_level_rise(ui, state),
                                t!("Sea Level Rise"),
                            );
                        });
                        tui.ui(|ui| {
                            stat(ui, |ui| render_population(ui, state), t!("Population"));
                        });
                    });

                    ui.add_space(32.);

                    h_center_top(ui, "stats-bottom-2", |tui| {
                        tui.ui(|ui| {
                            stat(
                                ui,
                                |ui| render_income(ui, state),
                                t!("Avg. Living Standards"),
                            );
                        });
                        tui.ui(|ui| {
                            stat(
                                ui,
                                |ui| render_habitability(ui, state),
                                t!("Avg. Habitability"),
                            );
                        });
                    });
                } else {
                    h_center_top(ui, "stats-bottom", |tui| {
                        tui.ui(|ui| {
                            stat(
                                ui,
                                |ui| render_sea_level_rise(ui, state),
                                t!("Sea Level Rise"),
                            );
                        });
                        tui.ui(|ui| {
                            stat(ui, |ui| render_population(ui, state), t!("Population"));
                        });
                        tui.ui(|ui| {
                            stat(
                                ui,
                                |ui| render_income(ui, state),
                                t!("Avg. Living Standards"),
                            );
                        });
                        tui.ui(|ui| {
                            stat(
                                ui,
                                |ui| render_habitability(ui, state),
                                t!("Avg. Habitability"),
                            );
                        });
                    });
                }
            } else {
                h_center_top(ui, "stats-top", |tui| {
                    tui.ui(|ui| {
                        stat(ui, |ui| render_temp(ui, state), t!("Temp. Anomaly"));
                    });
                    tui.ui(|ui| {
                        render_emissions(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_land(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_energy(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        render_water(ui, state, &process_changes);
                    });
                });

                ui.add_space(32.);

                h_center_top(ui, "stats-bottom", |tui| {
                    tui.ui(|ui| {
                        render_biodiversity(ui, state, &process_changes);
                    });
                    tui.ui(|ui| {
                        stat(
                            ui,
                            |ui| render_sea_level_rise(ui, state),
                            t!("Sea Level Rise"),
                        );
                    });
                    tui.ui(|ui| {
                        stat(ui, |ui| render_population(ui, state), t!("Population"));
                    });
                    tui.ui(|ui| {
                        stat(
                            ui,
                            |ui| render_income(ui, state),
                            t!("Avg. Living Standards"),
                        );
                    });
                    tui.ui(|ui| {
                        stat(
                            ui,
                            |ui| render_habitability(ui, state),
                            t!("Avg. Habitability"),
                        );
                    });
                });
            }

            ui.add_space(32.);

            render_history(ui, state, &mut self.history_metric);

            ui.add_space(32.);

            render_breakdown(
                ui,
                state,
                self.breakdown_factor,
                &mut self.show_breakdown_menu,
            );

            ui.add_space(64.);
        });

        if self.show_breakdown_menu {
            overlay(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if let Some(factor) = render_breakdown_menu(ui) {
                        self.breakdown_factor = factor;
                        self.show_breakdown_menu = false;
                    }
                })
                .response
            });
        }
    }
}

fn stat(
    ui: &mut egui::Ui,
    inner: impl FnOnce(&mut egui::Ui),
    label: Cow<'static, str>,
) -> egui::Response {
    ui.vertical_centered(|ui| {
        ui.set_width(120.);
        raised_frame()
            .colors(
                Color32::from_rgb(0x96, 0x8a, 0x68),
                Color32::from_rgb(0xEF, 0xE5, 0xD2),
                Color32::from_rgb(0xEB, 0xDE, 0xC6),
            )
            .margin(Margin {
                left: 8,
                right: 8,
                top: -12,
                bottom: 18,
            })
            .show(ui, |ui| {
                ui.set_width(80.);
                inner(ui);
            });
        ui.label(label);
    })
    .response
}

fn render_temp(ui: &mut egui::Ui, state: &State) {
    let temp_anomaly = state.temp_anomaly();
    ui.vertical_centered(|ui| {
        ui.add(icons::WARMING.size(24.));
        ui.add_space(8.);
        render_stat_text(ui, &temp_anomaly, None);
    });
}

fn render_emissions(ui: &mut egui::Ui, state: &State, process_changes: &[(Process, f32)]) {
    let emissions_change = process_changes
        .iter()
        .map(|(p, mult)| p.adj_byproducts().gtco2eq() * mult)
        .sum::<f32>()
        .round();

    let tip = {
        let tip_text = t!("Current annual emissions, in gigatonnes of CO2 equivalent.");
        tip(icons::EMISSIONS, tip_text).card(factors_card(None, Var::Emissions, state))
    };
    let emissions = state.emissions.as_gtco2eq();
    let emissions_display = state.emissions.display();
    let emissions_changed = display::emissions(emissions_change + emissions);

    render_dashboard_item(
        ui,
        &t!("Emissions"),
        None,
        &emissions_display,
        &emissions_changed,
        tip,
        emissions_change,
        icons::EMISSIONS,
    );
}

fn render_land(ui: &mut egui::Ui, state: &State, process_changes: &[(Process, f32)]) {
    let available_land = state.world.starting_resources.land;
    let tip = tip(icons::LAND, t!("Current land use.")).card(factors_card(None, Var::Land, state));
    let land_use = state.land_use_percent();
    let land_demand = state.resource_demand.of(Resource::Land);

    let land_change = process_changes
        .iter()
        .map(|(p, mult)| p.adj_resources().land * mult)
        .sum::<f32>()
        .round();
    let land_changed = format!(
        "{:.0}%",
        display::land_use_percent(land_change + land_demand, available_land)
    );

    render_dashboard_item(
        ui,
        &t!("Land Use"),
        None,
        &land_use,
        &land_changed,
        tip,
        land_change,
        icons::LAND,
    );
}

fn render_energy(ui: &mut egui::Ui, state: &State, process_changes: &[(Process, f32)]) {
    let energy_change = process_changes
        .iter()
        .map(|(p, mult)| {
            let energy = p.adj_resources().energy();
            energy * mult
        })
        .sum::<f32>()
        .round();

    let tip =
        tip(icons::ENERGY, t!("Current energy use.")).card(factors_card(None, Var::Energy, state));
    let energy_use = state.energy_pwh();
    let energy_demand = state.output_demand.total().energy();
    let energy_changed = format!(
        "{}PWh",
        (display::pwh(energy_change + energy_demand)).round()
    );

    render_dashboard_item(
        ui,
        &t!("Energy Use"),
        None,
        &energy_use,
        &energy_changed,
        tip,
        energy_change,
        icons::ENERGY,
    );
}

fn render_water(ui: &mut egui::Ui, state: &State, process_changes: &[(Process, f32)]) {
    let available_water = state.resources.available.water;
    let water_change = process_changes
        .iter()
        .map(|(p, mult)| p.adj_resources().water * mult)
        .sum::<f32>()
        .round();

    let water_demand = state.resource_demand.of(Resource::Water);
    let current_water_stress = state.water_use_percent();
    let after_water_stress = format!(
        "{:.0}%",
        display::water_use_percent(water_change + water_demand, available_water)
    );

    let tip =
        tip(icons::WATER, t!("Current water demand.")).card(factors_card(None, Var::Water, state));

    render_dashboard_item(
        ui,
        &t!("Water Stress"),
        None,
        &current_water_stress,
        &after_water_stress,
        tip,
        water_change,
        icons::WATER,
    );
}

fn render_biodiversity(ui: &mut egui::Ui, state: &State, process_changes: &[(Process, f32)]) {
    let extinction = |amount: f32| {
        let int = intensity::scale(amount, intensity::Variable::Extinction);
        MiniCardData {
            label: intensity::describe(int),
            color: intensity::color(int, false),
        }
    };

    let available_land = state.world.starting_resources.land;
    let extinction_change = process_changes
        .iter()
        .map(|(p, mult)| p.extinction_rate(available_land) * mult)
        .sum::<f32>()
        .round();
    let extinction_rate = state.world.extinction_rate;
    let current = extinction(extinction_rate);
    let after_extinction = extinction(extinction_rate + extinction_change).label;

    let tip_text = t!(
        "The current biodiversity pressure. High land use and other factors increase this, and with it, the risk of ecological collapse."
    );
    let tip =
        tip(icons::EXTINCTION_RATE, tip_text).card(factors_card(None, Var::Biodiversity, state));
    render_dashboard_item(
        ui,
        &t!("Extinction Rate"),
        Some(current.color),
        &current.label,
        &after_extinction,
        tip,
        extinction_change,
        icons::EXTINCTION_RATE,
    );
}

fn render_sea_level_rise(ui: &mut egui::Ui, state: &State) {
    let sea_level_rise = state.world.sea_level_rise;
    let sea_level_rise_rate = state.world.sea_level_rise_rate();
    let rise = format!("{:.2}", sea_level_rise);
    let tip_text = t!(
        "Average sea levels have risen by %{rise}m and are rising at a rate of %{rate}mm per year.",
        rise = rise,
        rate = format!("{:.1}", sea_level_rise_rate * 1000.)
    );
    let tip: Tip = tip(icons::SEA_LEVEL_RISE, tip_text);

    add_tip(
        tip,
        ui.vertical_centered(|ui| {
            ui.add(icons::SEA_LEVEL_RISE.size(24.));
            ui.add_space(8.);
            render_stat_text(ui, &format!("{rise} m"), None);
        })
        .response,
    );
}

fn render_population(ui: &mut egui::Ui, state: &State) {
    let population = state.world.regions.population();
    let mut f = Formatter::default()
        .scales(Scales::short())
        .precision(Precision::Decimals(1));
    let pop_fmted = f.fmt2(population as f64).to_string();

    ui.vertical_centered(|ui| {
        ui.add(icons::POPULATION.size(24.));
        ui.add_space(8.);
        render_stat_text(ui, &pop_fmted, None);
    });
}

fn render_income(ui: &mut egui::Ui, state: &State) {
    let income = state.avg_income_level();
    let income = MiniCardData {
        label: intensity::describe(income - 1),
        color: intensity::color(income, true),
    };

    ui.vertical_centered(|ui| {
        ui.add(icons::WEALTH.size(24.));
        ui.add_space(8.);
        render_stat_text(ui, &income.label, Some(income.color));
    });
}

fn render_habitability(ui: &mut egui::Ui, state: &State) {
    let habitability = state.world.regions.habitability().max(0.);
    let habitability = {
        let int = intensity::scale(habitability, intensity::Variable::Habitability);
        MiniCardData {
            label: intensity::describe(habitability as usize),
            color: intensity::color(int, true),
        }
    };

    ui.vertical_centered(|ui| {
        ui.add(icons::HABITABILITY.size(24.));
        ui.add_space(8.);
        render_stat_text(ui, &habitability.label, Some(habitability.color));
    });
}

fn render_breakdown_menu(ui: &mut egui::Ui) -> Option<Var> {
    let mut selected = None;
    raised_frame()
        .colors(
            Color32::from_rgb(0xF7, 0xF4, 0xEF),
            Color32::from_rgb(0xA7, 0x9A, 0x82),
            Color32::from_rgb(0xEB, 0xDE, 0xC6),
        )
        .margin(0)
        .show(ui, |ui| {
            let max_width = Var::iter()
                .map(|var| {
                    OrderedFloat::from(calc_text_width(
                        ui,
                        t!(var.title()).to_string(),
                        18.0,
                        egui::FontFamily::Name("TimesTen".into()),
                    ))
                })
                .max()
                .expect("non-empty")
                .into_inner()
                + 24.;
            let n = Var::iter().count();

            ui.style_mut().spacing.item_spacing.y = 0.;
            ui.style_mut().visuals.override_text_color = Some(Color32::BLACK);
            for (i, var) in Var::iter().enumerate() {
                let rounding = if i == 0 {
                    CornerRadius {
                        nw: 5,
                        ne: 5,
                        ..Default::default()
                    }
                } else if i == n - 1 {
                    CornerRadius {
                        sw: 5,
                        se: 5,
                        ..Default::default()
                    }
                } else {
                    0.into()
                };

                let mut frame = egui::Frame::NONE
                    .inner_margin(8.)
                    .corner_radius(rounding)
                    .begin(ui);

                frame.content_ui.horizontal(|ui| {
                    ui.set_width(max_width);
                    ui.vertical(|ui| {
                        ui.add_space(3.); // Manual alignment
                        ui.image(var.icon());
                    });
                    ui.label(egui::RichText::new(t!(var.title())).heading().size(18.));
                });

                let resp = frame.allocate_space(ui);
                if resp.hovered() {
                    frame.frame.fill = Color32::from_rgb(0xD7, 0xC5, 0xA5);
                }
                frame.paint(ui);

                if resp.interact(Sense::click()).clicked() {
                    selected = Some(var);
                }

                if i < n - 1 {
                    let rect = egui::Rect::from_min_size(
                        ui.cursor().left_top() - egui::vec2(0., 1.),
                        egui::vec2(resp.rect.width(), 1.),
                    );
                    ui.painter()
                        .rect_filled(rect, 0., Color32::from_rgb(0xAA, 0x9E, 0x88));
                    let rect = egui::Rect::from_min_size(
                        ui.cursor().left_top(),
                        egui::vec2(resp.rect.width(), 1.),
                    );
                    ui.painter()
                        .rect_filled(rect, 0., Color32::from_rgb(0xF7, 0xF4, 0xEF));
                }
            }
        });
    selected
}

fn render_breakdown(ui: &mut egui::Ui, state: &State, factor: Var, show_breakdown_menu: &mut bool) {
    let available_land = state.world.starting_resources.land;

    let dataset = {
        let mut total = 0.;
        let mut data: BTreeMap<String, (f32, String)> = BTreeMap::default();
        let factors = FACTORS.read();
        for fac in &factors[factor] {
            let name = t!(fac.name());
            data.insert(name.to_string(), (fac.amount(), fac.display()));
            total += fac.amount();
        }
        if factor == Var::Land {
            let name = t!("Unused");
            let unused = available_land - total;
            data.insert(
                name.to_string(),
                (
                    unused,
                    format!(
                        "{}%",
                        display::percent(
                            display::land_use_percent(unused, available_land,) / 100.,
                            true
                        )
                    ),
                ),
            );
        }
        data
    };

    let resp = raised_frame()
        .colors(
            Color32::WHITE,
            Color32::from_gray(0xBB),
            Color32::from_gray(0xEE),
        )
        .hover(Color32::from_gray(0xCC))
        .margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_width(160.);
            ui.set_height(24.);
            ui.add_space(3.);
            h_center(ui, "breakdown-button", |tui| {
                tui.ui(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(factor.icon().size(18.));
                        ui.add(
                            egui::Label::new(egui::RichText::new(format!(
                                "{}▼",
                                t!(factor.title())
                            )))
                            .wrap_mode(TextWrapMode::Extend),
                        )
                    });
                });
            });
        });
    if resp.interact(Sense::click()).clicked() {
        *show_breakdown_menu = true;
    }

    ui.vertical_centered(|ui| {
        ui.set_width(320.);
        render_chart(ui, &dataset, factor.color());

        scale_text_styles(ui.style_mut(), 0.8);
        let table_data = factors_card(None, factor, state);

        raised_frame()
            .colors(
                Color32::from_rgb(0xb0, 0xa4, 0x8d),
                Color32::from_rgb(0xfc, 0xf4, 0xe3),
                Color32::from_rgb(0xEB, 0xDE, 0xC6),
            )
            .margin(0)
            .show(ui, |ui| {
                ui.set_height(320.);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    table_data.render(ui);
                });
            });

        ui.label(t!("Only direct impacts are shown."));
    });
}

struct MiniCardData {
    label: Cow<'static, str>,
    color: Color32,
}

fn render_dashboard_item(
    ui: &mut egui::Ui,
    label: &str,
    color: Option<Color32>,
    display_value: &str,
    display_changed_value: &str,
    item_tip: Tip,
    change: f32,
    icon: Icon,
) {
    add_tip(
        item_tip,
        stat(
            ui,
            |ui| {
                ui.add(icon.size(24.));
                ui.add_space(8.);
                render_stat_text(ui, display_value, color);

                if change != 0. {
                    let change_tip = tip(
                        icon,
                        t!("The estimated value after production changes have finished."),
                    );
                    add_tip(
                        change_tip,
                        ui.label(
                            egui::RichText::new(format!("» {display_changed_value}")).size(11.),
                        ),
                    );
                }
            },
            Cow::Owned(label.to_string()),
        ),
    );
}

fn render_chart(ui: &mut egui::Ui, dataset: &BTreeMap<String, (f32, String)>, colors: [u32; 2]) {
    let n = dataset.len() as f32;
    let items: Vec<_> = dataset
        .iter()
        .enumerate()
        .map(|(i, (k, (v, display)))| {
            let (r, g, b) = lerp_color(colors[0], colors[1], i as f32 / n);
            TreeItem {
                label: k,
                value: *v,
                display,
                color: Color32::from_rgb(r, g, b),
            }
        })
        .collect();

    treemap(ui, "breakdown-chart", (320., 200.), items);
}

fn lerp_color(from: u32, to: u32, ratio: f32) -> (u8, u8, u8) {
    let ar = ((from & 0xFF0000) >> 16) as f32;
    let ag = ((from & 0x00FF00) >> 8) as f32;
    let ab = (from & 0x0000FF) as f32;

    let br = ((to & 0xFF0000) >> 16) as f32;
    let bg = ((to & 0x00FF00) >> 8) as f32;
    let bb = (to & 0x0000FF) as f32;

    let rr = ar + ratio * (br - ar);
    let rg = ag + ratio * (bg - ag);
    let rb = ab + ratio * (bb - ab);

    (rr.round() as u8, rg.round() as u8, rb.round() as u8)
}

fn render_stat_text(ui: &mut egui::Ui, text: &str, color: Option<Color32>) {
    let mut text = egui::RichText::new(text).monospace();
    if let Some(color) = color {
        text = text.color(color);
    }
    scale_text(ui, egui::vec2(80., 28.), text, 24.);
}

/// Warming per Gt of CO2, for the simple "if emissions stay at today's
/// level" forecast (IPCC AR6 best estimate, 0.45C per 1000 GtCO2).
const TCRE_PER_GT: f32 = 0.45e-3;
const FORECAST_END: usize = 2100;

/// A line chart of one value over the game's history.
fn render_history(ui: &mut egui::Ui, state: &State, metric: &mut HistoryMetric) {
    let history = &state.history;
    if history.len() < 2 {
        return;
    }

    egui::Frame::NONE
        .fill(Color32::from_white_alpha(200))
        .corner_radius(6)
        .inner_margin(10)
        .show(ui, |ui| {
            // Size from the screen, not the available width, which can be
            // wider than the screen on small devices.
            let screen_width = ui.ctx().content_rect().width() - 48.;
            let width = ui.available_width().min(screen_width).min(680.).max(200.);
            ui.set_width(width);
            ui.set_max_width(width);
            ui.horizontal_wrapped(|ui| {
                for (m, label) in [
                    (HistoryMetric::Temperature, t!("Temperature")),
                    (HistoryMetric::Emissions, t!("Emissions")),
                    (HistoryMetric::Extinction, t!("Extinction")),
                    (HistoryMetric::LandUse, t!("Land Use")),
                ] {
                    let text = egui::RichText::new(label).size(14.);
                    let text = if *metric == m { text.strong().underline() } else { text };
                    if ui.add(egui::Label::new(text).sense(egui::Sense::click())).clicked() {
                        *metric = m;
                    }
                    ui.add_space(8.);
                }
            });
            ui.add_space(6.);

            let value = |r: &hes_engine::YearRecord| match metric {
                HistoryMetric::Temperature => r.temperature,
                HistoryMetric::Emissions => r.emissions,
                HistoryMetric::Extinction => r.extinction_rate,
                HistoryMetric::LandUse => r.land_use * 100.,
            };
            let points: Vec<(f32, f32)> = history.iter().map(|r| (r.year as f32, value(r))).collect();

            // Temperature: project forward with today's CO2 emissions.
            let last = history.last().unwrap();
            let forecast: Vec<(f32, f32)> = if *metric == HistoryMetric::Temperature {
                let co2 = last.co2 + last.land;
                (last.year..=FORECAST_END)
                    .step_by(1)
                    .map(|y| (y as f32, last.temperature + TCRE_PER_GT * co2 * (y - last.year) as f32))
                    .collect()
            } else {
                vec![]
            };
            // Win thresholds.
            let target = match metric {
                HistoryMetric::Temperature => Some((1.0, t!("Goal"))),
                HistoryMetric::Emissions => Some((0.0, t!("Goal"))),
                HistoryMetric::Extinction => Some((20.0, t!("Goal"))),
                HistoryMetric::LandUse => None,
            };

            let all = points.iter().chain(forecast.iter());
            let (mut y_min, mut y_max) = all.clone().fold((f32::MAX, f32::MIN), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
            if let Some((t, _)) = &target {
                y_min = y_min.min(*t);
                y_max = y_max.max(*t);
            }
            let pad = ((y_max - y_min) * 0.1).max(0.1);
            let (y_min, y_max) = (y_min - pad, y_max + pad);
            let x_min = points[0].0;
            let x_max = forecast.last().map(|p| p.0).unwrap_or(points.last().unwrap().0).max(x_min + 1.);

            let height = 170.;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let plot = rect.shrink2(egui::vec2(28., 12.));
            let to_screen = |x: f32, y: f32| {
                egui::pos2(
                    plot.left() + (x - x_min) / (x_max - x_min) * plot.width(),
                    plot.bottom() - (y - y_min) / (y_max - y_min) * plot.height(),
                )
            };
            let font = egui::FontId::proportional(11.);
            let axis = Color32::from_gray(90);

            // Axes and labels.
            painter.line_segment([plot.left_bottom(), plot.right_bottom()], egui::Stroke::new(1., axis));
            painter.line_segment([plot.left_top(), plot.left_bottom()], egui::Stroke::new(1., axis));
            let fmt = |v: f32| if (y_max - y_min) < 5. { format!("{v:.1}") } else { format!("{v:.0}") };
            for v in [y_min + pad, (y_min + y_max) / 2., y_max - pad] {
                let p = to_screen(x_min, v);
                painter.text(egui::pos2(rect.left(), p.y), egui::Align2::LEFT_CENTER, fmt(v), font.clone(), axis);
                painter.line_segment([p, egui::pos2(plot.right(), p.y)], egui::Stroke::new(0.5, Color32::from_gray(200)));
            }
            for x in [x_min, points.last().unwrap().0, x_max] {
                let p = to_screen(x, y_min);
                painter.text(egui::pos2(p.x, rect.bottom()), egui::Align2::CENTER_BOTTOM, format!("{x:.0}"), font.clone(), axis);
            }

            // Goal line.
            if let Some((t, label)) = target {
                let (a, b) = (to_screen(x_min, t), to_screen(x_max, t));
                dashed(&painter, a, b, egui::Stroke::new(1., Color32::from_rgb(0x2e, 0x8b, 0x57)));
                painter.text(egui::pos2(b.x, a.y - 2.), egui::Align2::RIGHT_BOTTOM, label, font.clone(), Color32::from_rgb(0x2e, 0x8b, 0x57));
            }

            // History and forecast.
            let line_color = Color32::from_rgb(0xc0, 0x39, 0x2b);
            let line: Vec<egui::Pos2> = points.iter().map(|(x, y)| to_screen(*x, *y)).collect();
            painter.add(egui::Shape::line(line, egui::Stroke::new(2., line_color)));
            if forecast.len() > 1 {
                for w in forecast.windows(2).step_by(2) {
                    let (a, b) = (to_screen(w[0].0, w[0].1), to_screen(w[1].0, w[1].1));
                    painter.line_segment([a, b], egui::Stroke::new(1.5, line_color.gamma_multiply(0.6)));
                }
                let end = forecast.last().unwrap();
                painter.text(
                    to_screen(end.0, end.1) + egui::vec2(-2., -4.),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("{:+.1}°C", end.1),
                    font.clone(),
                    line_color,
                );
            }

            ui.add_space(4.);
            let note = match metric {
                HistoryMetric::Temperature => t!("Dotted: where temperature heads by 2100 if CO₂ emissions stay at this year's level."),
                HistoryMetric::Emissions => t!("Annual emissions, including land use, in gigatonnes of CO₂ equivalent."),
                HistoryMetric::Extinction => t!("Biodiversity pressure. Below 20 is a goal."),
                HistoryMetric::LandUse => t!("Share of all land used for farming and infrastructure."),
            };
            ui.scope(|ui| {
                ui.set_max_width(width);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.add(egui::Label::new(egui::RichText::new(note).size(11.)).wrap());
            });
        });
}

fn dashed(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: egui::Stroke) {
    let len = a.distance(b);
    let dir = (b - a) / len.max(1.);
    let mut d = 0.;
    while d < len {
        let e = (d + 5.).min(len);
        painter.line_segment([a + dir * d, a + dir * e], stroke);
        d += 9.;
    }
}
