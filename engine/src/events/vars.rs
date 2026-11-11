use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    IntoStaticStr,
    EnumIter,
    EnumString,
    Display,
)]
pub enum WorldVariable {
    Year,
    Population,
    Emissions,
    ExtinctionRate,
    Outlook,
    Temperature,
    SeaLevelRise,
    SeaLevelRiseRate,
    Precipitation,
    PopulationGrowth,

    /// Fractional change to background deforestation (-0.3 is 30% less).
    Deforestation,

    /// Fractional change to how fast freed land regrows (0.2 is 20% faster).
    Regrowth,
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    IntoStaticStr,
    EnumIter,
    EnumString,
    Display,
)]
pub enum LocalVariable {
    Population,
    Outlook,
    Habitability,
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    IntoStaticStr,
    EnumIter,
    EnumString,
    Display,
)]
pub enum PlayerVariable {
    PoliticalCapital,
    ResearchPoints,
    YearsToDeath,
}
