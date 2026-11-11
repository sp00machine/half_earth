// Balance tool: prints the extinction rate a few years into a game
// with the starting plan, broken down by process.
//
//     cargo run --example extinction_breakdown -- assets/DEFAULT.world
use hes_engine::{State, World};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let world: World = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut state = State::new(world);
    for _ in 0..3 { state.step_year(state.world.temperature); }
    let land = state.world.starting_resources.land;
    let mut rows: Vec<(String, f32)> = state.world.processes.iter().map(|p| {
        let amt = state.produced.by_process.get(&p.id).copied().unwrap_or(0.);
        (p.name.clone(), p.extinction_rate(land) * amt)
    }).filter(|(_, v)| v.abs() > 0.5).collect();
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!("total extinction {:.1} (temp part {:.1}, modifier {:.1})", state.world.extinction_rate,
        state.world.tgav_extinction_rate() + state.world.slr_extinction_rate(), state.world.extinction_rate_modifier);
    for (n, v) in rows { println!("  {n:28} {v:6.1}"); }
}
