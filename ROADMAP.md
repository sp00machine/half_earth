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
- **Tap through from breakdowns.** In the resource and emissions views,
  tapping an industry or process opens its card (e.g. Other Industry in
  fuel use, Cellular Meat in electricity use).
- **Two rows of impacts on process cards.** The existing row (land,
  water, energy, emissions, extinction) shows efficiency when tapped:
  "uses X of land to make Y of power". A second row below shows the
  process's share of the world total, current -> planned (e.g. land
  2% -> 0%); tapping it opens that resource's view, as the first row does
  now.

## Model

- **Industrial vs. organic farming check:** now that fertilizer, soil
  health and pesticides are all modeled, play it through and check the
  trade-off is real: industrial uses less land; organic and smallholder
  farming are kinder to soils and biodiversity per hectare.
- **Soil health on farm process cards:** show each farming process's push
  on soil health (intensive vs. restorative) on its card.
- **Authoritarian path pass:** make it possible, with care, to win the
  Authoritarian over early and rule without parliament (Suspend
  Parliament), as a real alternative to building a coalition.
- **Guaranteed Basic Allotments** as a policy, in place of price controls.
- **Black markets:** a persistent parallel dirty economy that grows when
  rationing and bans bite.
- **Rework space and megaproject cards** (Space Elevator, Asteroid Mining,
  Relocate Industry to Space, Orbital Settlements, Antarctic Pumping,
  Mission to Titan): give them construction costs and realistic effects,
  or keep them clearly as sci-fi.
- **Atlantropa** (Accelerationist megaproject): dam the Strait of
  Gibraltar and lower the Mediterranean, as Herman Sörgel proposed in the
  1920s. Huge hydropower and new farmland, catastrophic for extinction and
  for Mediterranean regions. Goes with the megaproject rework.
- **More utopian-fantasy cards** in the spirit of Esperanto, Decimal Time
  and Dialogue with Dolphins: one for each faction that doesn't have one
  yet (Malthusian, Environmentalist), avoiding anything too grim.

- **Balance pass** (after the rest of the model work): play through with
  every system in place and tune costs, effects and contentedness
  together. Consider making policies cheaper in the first planning session,
  so the opening turn isn't starved of political capital.

## Tooling

- **Balance scenario runner:** scripted strategies played to 2100, so
  balance changes can be checked against many playthroughs at once.

## Presentation

- **Watch the world after winning:** after a win, keep running years
  without planning sessions, just watching the world go by, ending with a
  closing dialogue scene.
- **Ending screen with your stats:** the stats view, with its detail
  views and history charts, on the ending screen.

- **Rename** to Three-Quarters Earth Socialism.
- **Three-Fifths Earth ending** (an achievement for protecting 60%).
- Credits and attribution for the fork; link previews; a Report a Bug
  link.

## Later

- **The Half-World Economic Forum:** a separate parody game from the
  capitalist side, with a fictional host rather than a real person.
