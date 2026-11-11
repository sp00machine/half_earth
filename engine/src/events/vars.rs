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

    /// Share of lithium recovered from retired batteries (absolute change).
    BatteryRecycling,

    /// Fractional change to how fast lithium mining can grow.
    LithiumMining,

    /// Fractional change to energy lost in storage round trips.
    StorageLosses,

    /// This year's CO2 from clearing land, in Gt (for conditions only).
    LandClearing,

    /// This year's CO2 taken up by regrowing land, in Gt (for conditions only).
    LandRegrowth,

    /// Share of needed battery storage that couldn't be built (for conditions only).
    StorageShortfall,
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
