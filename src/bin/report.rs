//! `cargo run --release --bin report [-- --detail]` prints automated validation per level.
use vfield::modes::experiment;
use vfield::sim::Outcome;
use vfield::{levels, validate, Rules};

fn main() {
    let detail = std::env::args().any(|a| a == "--detail");
    for l in levels::all() {
        println!("{:<6} {:<22} solvable={:<5} {}", l.id, l.title, validate::solvable(&l), validate::summarize(&l));
        if !detail {
            continue;
        }
        match &l.rules {
            Rules::Position { slots, period, intensity } if period.is_none() && intensity.is_none() => {
                let z = slots[0].zone.bbox();
                for j in (0..13).rev() {
                    let mut row = String::new();
                    for i in 0..40 {
                        let p = vfield::v2(z.min.x + z.width() * i as f64 / 39.0, z.min.y + z.height() * j as f64 / 12.0);
                        if !slots[0].zone.contains(p) { row.push(' '); continue; }
                        let t = l.run(&l.field, p, 0.0);
                        row.push(if l.reached_goal(&t, slots[0].goal) { '#' } else if matches!(t.outcome, Outcome::Hazard(_)) { 'x' } else { '.' });
                    }
                    println!("    {}", row);
                }
            }
            Rules::Tempo { .. } => {
                let f = validate::tempo_success_by_phase(&l, 96);
                println!("    {}", f.iter().map(|b| if *b { '#' } else { '.' }).collect::<String>());
            }
            Rules::Experiment { .. } => {
                let s: String = experiment::sweep(&l, 100).iter().map(|(_, t)| match t.outcome {
                    Outcome::Reached(e) => l.regions[e].label, _ => '.' }).collect();
                println!("    {}", s);
            }
            Rules::Predict { zone, .. } => {
                let b = zone.bbox();
                for j in (0..9).rev() {
                    let mut row = String::new();
                    for i in 0..30 {
                        let p = vfield::v2(b.min.x + b.width() * i as f64 / 29.0, b.min.y + b.height() * j as f64 / 8.0);
                        if !zone.contains(p) { row.push(' '); continue; }
                        row.push(match l.run(&l.field, p, 0.0).outcome { Outcome::Reached(e) => l.regions[e].label, _ => '.' });
                    }
                    println!("    {}", row);
                }
            }
            _ => {}
        }
    }
}
