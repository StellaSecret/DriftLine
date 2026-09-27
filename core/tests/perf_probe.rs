// TEMPORARY diagnostic. Deleted before commit.
use std::time::Instant;

use peoplemodeler_core::{
    chapter_count, generate_level_group, perf_extra, perf_gates, perf_levels, perf_reset,
    perf_snapshot, perf_timeouts, tutorial_level_for, Chapter,
};

#[test]
fn profile_generation() {
    let seed = 0x5EED_D11E;
    let mut tutorial = 0.0;
    let mut game = 0.0;
    for group in 0..chapter_count() {
        let chapter = Chapter::all()[group];
        let start = Instant::now();
        let teaching = tutorial_level_for(chapter, 0);
        let tutorial_ms = start.elapsed().as_secs_f64() * 1e3;
        perf_reset();
        let start = Instant::now();
        let levels = generate_level_group(seed, group);
        let game_ms = start.elapsed().as_secs_f64() * 1e3;
        let (fine, fast, fine_steps, fast_steps) = perf_snapshot();
        let (solves, fallbacks, shapes, accepts, paths) = perf_extra();
        let (fine_to, fast_to) = perf_timeouts();
        let gates = perf_gates();
        let (random_ok, band_checks, zone_checks) = perf_levels();
        tutorial += tutorial_ms;
        game += game_ms;
        println!(
            "{:>2} {:<10} tutorial {:>8.1} ms   game {:>8.1} ms  ({} levels, window {:.4})",
            group + 1,
            chapter.title(),
            tutorial_ms,
            game_ms,
            levels.len(),
            levels[0].k_window
        );
        println!(
            "     fine {:>7} ({:>8} steps, {:>5} capped)  fast {:>8} ({:>9} steps, {:>5} capped)  paths {:>6}  shapes {:>6}  accepts {:>5}",
            fine, fine_steps, fine_to, fast, fast_steps, fast_to, paths, shapes, accepts
        );
        println!(
            "     level: accepted {} zone-checks {} band-checks {}  rejects: win {} window {} freewin {} spread {} ghost {} knobs {} dial {} band {}",
            random_ok, zone_checks, band_checks,
            gates[0], gates[1], gates[2], gates[3], gates[4], gates[5], gates[6], gates[7]
        );
    }
    println!("TOTAL tutorial {tutorial:.1} ms   game {game:.1} ms");
}
