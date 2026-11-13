use egui::{Align, Color32, Layout, Stroke};
use egui_taffy::TuiBuilderLogic;
use rust_i18n::t;

use crate::{
    consts,
    display::{
        self, HasIcon, Icon,
        factors::{Factor, Gases, LandSplit},
        icons,
        intensity::intensity_bar,
    },
    parts::flex_justified,
    state::FACTORS,
    vars::Var,
};

#[derive(Debug, Clone, PartialEq)]
pub struct FactorsCard {
    pub icon: Icon,
    pub kind: Var,
    pub total: f32,
    pub current: Option<String>,
    /// For emissions, the total split by gas.
    pub gases: Option<Gases>,
    /// For land, how it's split between uses.
    pub land: Option<LandSplit>,
}

/// A bar showing how land is used: farmed, regrowing, protected, wild.
fn land_bar(ui: &mut egui::Ui, land: &LandSplit) {
    let parts = [
        (
            land.farmed,
            Color32::from_rgb(0xd9, 0xa4, 0x41),
            t!("Farmed & built"),
        ),
        (
            land.regrowing,
            Color32::from_rgb(0x9c, 0xd3, 0x6a),
            t!("Regrowing"),
        ),
        (
            land.protected,
            Color32::from_rgb(0x4c, 0xc0, 0x7d),
            t!("Protected"),
        ),
        (land.wild, Color32::from_rgb(0x7f, 0xa8, 0x8e), t!("Wild")),
    ];
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.), egui::Sense::hover());
    let mut x = rect.left();
    for (share, color, _) in &parts {
        let w = rect.width() * share.max(0.);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height())),
            0.,
            *color,
        );
        x += w;
    }
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().spacing.item_spacing.x = 8.;
        for (share, color, name) in parts {
            if share >= 0.005 {
                ui.label(
                    egui::RichText::new(format!("{name} {:.0}%", share * 100.))
                        .color(color)
                        .small(),
                );
            }
        }
    });
}

const CO2_COLOR: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);
const CH4_COLOR: Color32 = Color32::from_rgb(0xf2, 0xa9, 0x3b);
const N2O_COLOR: Color32 = Color32::from_rgb(0x9b, 0x7b, 0xd4);
const FGAS_COLOR: Color32 = Color32::from_rgb(0x4f, 0xb6, 0xc9);

/// A thin bar showing how emissions split between gases, plus a short label.
fn gas_bar(ui: &mut egui::Ui, gases: &Gases, label: bool) {
    let parts = [
        (gases.co2, CO2_COLOR, "CO₂"),
        (gases.ch4, CH4_COLOR, "CH₄"),
        (gases.n2o, N2O_COLOR, "N₂O"),
        (gases.fgas, FGAS_COLOR, "F-gases"),
    ];
    let positive: f32 = parts.iter().map(|(v, ..)| v.max(0.)).sum();
    if positive > 0. {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.), egui::Sense::hover());
        let mut x = rect.left();
        for (value, color, _) in parts {
            if value > 0. {
                let w = rect.width() * value / positive;
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(w, rect.height()),
                    ),
                    0.,
                    color,
                );
                x += w;
            }
        }
    }
    if label {
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 8.;
            for (value, color, name) in parts {
                if value.abs() >= 0.05 {
                    ui.label(
                        egui::RichText::new(format!("{name} {value:+.1}"))
                            .color(color)
                            .small(),
                    );
                }
            }
        });
    }
}
impl FactorsCard {
    pub fn render(&self, ui: &mut egui::Ui) -> egui::Response {
        egui::Frame::NONE
            .inner_margin(8)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_max_height(ui.ctx().content_rect().height() - 148.);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                    let relation = {
                        let relation = match self.kind {
                            Var::Emissions => "makes",
                            Var::Biodiversity => "causes",
                            _ => "uses",
                        };
                        t!(relation)
                    };

                    let cur_name = self.current.as_ref();

                    egui::Frame::NONE
                        .inner_margin(4)
                        .fill(Color32::from_black_alpha(64))
                        .stroke(Stroke::new(1f32, Color32::from_black_alpha(96)))
                        .corner_radius(4)
                        .show(ui, |ui| {
                            flex_justified(ui, "factors-header", |tui| {
                                tui.label(format!("{} :", t!("Total")));
                                tui.ui(|ui| {
                                    let max_value = match self.kind {
                                        Var::Biodiversity => Some(consts::MAX_BIODIVERSITY),
                                        Var::Contentedness => Some(consts::MAX_CONTENTEDNESS),
                                        _ => None,
                                    };
                                    let total = self.total_formatted();

                                    ui.horizontal(|ui| {
                                        if let Some(max_value) = max_value {
                                            ui.label(format!("{total}/{max_value}"));
                                        } else {
                                            ui.label(total);
                                        }
                                        ui.add(self.icon.size(18.));
                                    });
                                });
                            });
                            if let Some(land) = &self.land {
                                land_bar(ui, land);
                            }
                            if let Some(gases) = &self.gases {
                                gas_bar(ui, gases, true);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(t!(
                                            "CO₂ stays in the air for centuries. Methane (CH₄) fades in about 12 years; nitrous oxide (N₂O) in about 110."
                                        ))
                                        .small(),
                                    )
                                    .wrap(),
                                );
                            }
                        });

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let ranked = FACTORS.read();
                        let ranked = ranked[self.kind].iter().filter(|user| match user {
                            Factor::Industry { produced, .. }
                            | Factor::Process { produced, .. } => *produced != 0.,
                            _ => true,
                        });

                        ui.style_mut().spacing.item_spacing.y = 0.;
                        for user in ranked {
                            let highlight = cur_name.is_some_and(|name| name == user.name());
                            let name = user.name();
                            egui::Frame::NONE
                                .inner_margin(4)
                                .corner_radius(4)
                                .fill(if highlight {
                                    Color32::from_rgb(0xf5, 0xf9, 0xc7)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .show(ui, |ui| {
                                    if highlight {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(Color32::BLACK);
                                    }
                                    ui.label(t!(name));
                                    render_factor_line(ui, name, user, &relation, self.icon);
                                    if let Factor::Process { gases: Some(gases), .. }
                                    | Factor::Industry { gases: Some(gases), .. } = user
                                    {
                                        // Only spell out the gases when it's not almost all CO2.
                                        let non_co2 = gases.ch4.max(0.) + gases.n2o.max(0.) + gases.fgas.max(0.);
                                        let label = non_co2 > 0.05 * gases.total().abs().max(1e-6);
                                        gas_bar(ui, gases, label);
                                    }
                                });
                        }
                    });
                });
            })
            .response
    }

    pub fn total_formatted(&self) -> String {
        match self.kind {
            Var::Emissions => display::emissions(self.total),
            Var::Biodiversity => display::extinction(self.total),
            Var::Land => {
                format!("{}%", display::percent(self.total / 100., true))
            }
            Var::Water => {
                format!("{}%", display::percent(self.total / 100., true))
            }
            Var::Energy | Var::Electricity | Var::Fuel => {
                format!("{}GW", display::round_energy(self.total))
            }
            Var::PlantCalories => {
                format!("{}", self.total.round())
            }
            Var::AnimalCalories => {
                format!("{}", self.total.round())
            }
            Var::Contentedness => {
                format!("{}", self.total)
            }
        }
    }
}

fn render_factor_line(ui: &mut egui::Ui, name: &str, factor: &Factor, relation: &str, icon: Icon) {
    match factor {
        Factor::Region {
            intensity, display, ..
        } => {
            ui.horizontal(|ui| {
                ui.horizontal_centered(|ui| {
                    ui.set_width(ui.available_width() / 2.);
                    ui.add(icons::WEALTH.size(18.));
                    ui.add(intensity_bar(*intensity).pips(4));
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.;
                    ui.set_width(ui.available_width() / 2.);
                    ui.add(icon.size(18.));
                    ui.label(display);
                });
            });
        }
        Factor::Project { display, .. } => {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.;
                    ui.add(icon.size(18.));
                    ui.label(display);
                });
            });
        }
        Factor::Event {
            display, amount, ..
        } => {
            let display = display.clone().unwrap_or_else(|| amount.to_string());
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.;
                    ui.add(icon.size(18.));
                    ui.label(display);
                });
            });
        }
        Factor::Process {
            intensity,
            display_produced,
            display,
            output,
            ..
        } => {
            flex_justified(ui, name, |tui| {
                tui.ui(|ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add(icon.size(18.));
                        ui.add(intensity_bar(*intensity).pips(4));
                    });
                });
                tui.ui(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.;
                    ui.horizontal_centered(|ui| {
                        ui.label(display_produced);
                        ui.add(output.icon().size(18.));
                        ui.label(relation);
                        ui.label(display);
                        ui.add(icon.size(18.));
                    });
                });
            });
        }
        Factor::Industry {
            intensity, display, ..
        } => {
            ui.horizontal(|ui| {
                ui.horizontal_centered(|ui| {
                    ui.set_width(ui.available_width() / 2.);
                    ui.add(icon.size(18.));
                    ui.add(intensity_bar(*intensity).pips(4));
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.;
                    ui.set_width(ui.available_width() / 2.);
                    ui.add(icon.size(18.));
                    ui.label(display);
                });
            });
        }
    }
}
