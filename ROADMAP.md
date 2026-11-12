# Roadmap

Planned work for the Three-Quarters Earth fork, roughly in priority order.
Done work lives in the git history; each change's reasoning is in the
`notes` of the objects it touched in `engine/assets/DEFAULT.world`.

## Transparency

- **Data visualization design pass.** Plan where charts and breakdowns
  belong across the whole game, then build them well. The goal: from any
  number, one tap shows what it's made of and how it's changed over time.
  For example:
  - an industry (say Iron and Steel) over time, with markers for what
    pushed it up or down (construction from a process change or project,
    an event, a policy);
  - that industry's share of fuel use, emissions, electricity at a glance;
  - one more tap: fuel use (or emissions, land, electricity) over time,
    stacked by industry and process;
  - the same for contentedness by region and income level, lithium and
    storage, land use, extinction.

  Needs recording more history each year (per industry, per process, per
  region) and a consistent chart style. To do on a PC, with care.
- **Planner warnings for fuels running out** (uranium, oil, gas, lithium):
  "runs out in ~6 years at this mix", before it happens.
- **Phase in contentedness hits** over a few years instead of all at once,
  so falling contentedness is visible before riots start (open question).

## Model

- **Soil health:** intensive farming slowly degrades soils (lower yields,
  more fertilizer); regenerative practices restore them. Make organic
  farming's N2O easier to see.
- **Guaranteed Basic Allotments** as a policy, in place of price controls.
- **Black markets:** a persistent parallel dirty economy that grows when
  rationing and bans bite.
- **Rework space and megaproject cards** (Space Elevator, Asteroid Mining,
  Relocate Industry to Space, Orbital Settlements, Antarctic Pumping,
  Mission to Titan): give them construction costs and realistic effects,
  or keep them clearly as sci-fi.

## Tooling

- **Balance scenario runner:** scripted strategies played to 2100, so
  balance changes can be checked against many playthroughs at once.

## Presentation

- **Rename** to Three-Quarters Earth Socialism.
- **Three-Fifths Earth ending** (an achievement for protecting 60%).
- Credits and attribution for the fork; link previews; a Report a Bug
  link.

## Later

- **The Half-World Economic Forum:** a separate parody game from the
  capitalist side, with a fictional host rather than a real person.
