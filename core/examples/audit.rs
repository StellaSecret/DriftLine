use peoplemodeler_core::*;
use std::time::Instant;

fn main() {
    let seeds = [DEFAULT_SEED, 1, 7, 42, 0xdead];
    let mut defects = 0;
    let mut spread_worst = 0.0f64;
    let start = Instant::now();
    for seed in seeds {
        for group in 0..chapter_count() {
            for level in generate_level_group(seed, group) {
                let anchors = release_points(&level);
                let step = level.recommended_step;
                let count = (((level.k_max - level.k_min) / step).ceil() as usize) + 1;
                let mut wins = 0;
                for i in 0..count {
                    let k = level.k_min + i as f64 * step;
                    if k <= level.k_max + 1e-9 && integrate(&level, k).reached() {
                        wins += 1;
                    }
                }
                let ratio = wins as f64 / count as f64;
                spread_worst = spread_worst.max(ratio);
                if wins == 0 || integrate(&level, 0.0).reached() {
                    defects += 1;
                    println!("DEFAUT {} seed={seed:x} ch={group}", level.focus);
                }
                for anchor in &anchors {
                    if integrate_from(&level, *anchor, 0.0).reached() {
                        defects += 1;
                        println!(
                            "DEFAUT ancre libre {} seed={seed:x} ch={group} a={anchor:?}",
                            level.focus
                        );
                        break;
                    }
                }
            }
        }
    }
    println!(
        "niveaux={} défauts={defects} pire_grille={:.0}% total={:?}",
        seeds.len() * total_level_count(),
        spread_worst * 100.0,
        start.elapsed()
    );
    let mut worst = std::collections::BTreeMap::<usize, f64>::new();
    for seed in seeds {
        for group in 0..chapter_count() {
            let t = Instant::now();
            let _ = generate_level_group(seed, group);
            let e = worst.entry(group).or_insert(0.0);
            *e = e.max(t.elapsed().as_secs_f64());
        }
    }
    println!("pire temps par chapitre : {:?}", worst);
}
