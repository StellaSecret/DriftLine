//! Moteur du jeu : champs de tangentes, intégration RK4 de la solution
//! unique issue de A (Cauchy-Lipschitz), détection de collision/victoire.
//! Aucune dépendance UI ici : testable indépendamment de Dioxus.

pub const XMIN: f64 = -6.0;
pub const XMAX: f64 = 6.0;
pub const YMIN: f64 = -4.0;
pub const YMAX: f64 = 4.0;
pub const WIN_R: f64 = 0.55;
pub const OBJ_R: f64 = 0.18;

/// f(x, y, k) -> dy/dx
pub type FieldFn = fn(f64, f64, f64) -> f64;

#[derive(Clone, Copy)]
pub struct Obstacle {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

#[derive(Clone)]
pub struct Level {
    pub title: &'static str,
    pub desc: &'static str,
    pub field: FieldFn,
    pub k_min: f64,
    pub k_max: f64,
    pub k_def: f64,
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub obstacles: &'static [Obstacle],
}

fn field1(x: f64, _y: f64, k: f64) -> f64 {
    k * x / 3.0
}
fn field2(_x: f64, y: f64, k: f64) -> f64 {
    k - y * 0.5
}
fn field3(x: f64, _y: f64, k: f64) -> f64 {
    x.sin() + k * 0.5
}
fn field4(x: f64, y: f64, k: f64) -> f64 {
    k * (x * 0.5).cos() - 0.3 * y
}

pub fn levels() -> Vec<Level> {
    vec![
        Level {
            title: "Courant calme",
            desc: "Un courant régulier traverse la zone. Règle son intensité pour que la sonde dérive jusqu'à la balise.",
            field: field1,
            k_min: -3.0, k_max: 3.0, k_def: 0.0,
            a: (-5.0, 0.0), b: (4.0, 1.0),
            obstacles: &[],
        },
        Level {
            title: "Zone de retenue",
            desc: "Le courant ramène toujours la sonde vers une hauteur d'équilibre. Contourne l'astéroïde central.",
            field: field2,
            k_min: -3.0, k_max: 3.0, k_def: 0.0,
            a: (-5.0, -3.0), b: (4.0, 1.5),
            obstacles: &[Obstacle { x: 0.0, y: -0.5, r: 0.9 }],
        },
        Level {
            title: "Houle",
            desc: "Le courant ondule sur toute la zone. Slalome entre les deux astéroïdes.",
            field: field3,
            k_min: -3.0, k_max: 3.0, k_def: 0.0,
            a: (-5.0, 0.0), b: (5.0, -1.0),
            obstacles: &[
                Obstacle { x: -1.0, y: 1.6, r: 0.7 },
                Obstacle { x: 2.0, y: -1.6, r: 0.7 },
            ],
        },
        Level {
            title: "Tourbillon",
            desc: "Un courant instable, très sensible au réglage. Vise juste.",
            field: field4,
            k_min: -3.0, k_max: 3.0, k_def: 0.0,
            a: (-5.0, 3.0), b: (5.0, -2.0),
            obstacles: &[
                Obstacle { x: 0.0, y: 0.0, r: 1.0 },
                Obstacle { x: 3.0, y: 2.0, r: 0.6 },
            ],
        },
    ]
}

pub struct SimResult {
    pub points: Vec<(f64, f64)>,
    pub collided: bool,
    pub reached: bool,
}

/// Intègre par RK4 l'unique solution passant par `level.a` pour le champ
/// réglé à l'intensité `k`.
pub fn integrate(level: &Level, k: f64) -> SimResult {
    let h = 0.04;
    let (mut x, mut y) = level.a;
    let mut points = vec![(x, y)];
    let mut collided = false;
    let mut reached = false;
    let f = level.field;

    for _ in 0..800 {
        if x >= XMAX {
            let (bx, by) = level.b;
            if ((x - bx).powi(2) + (y - by).powi(2)).sqrt() < WIN_R {
                reached = true;
            }
            break;
        }
        let k1 = f(x, y, k);
        let k2 = f(x + h / 2.0, y + h / 2.0 * k1, k);
        let k3 = f(x + h / 2.0, y + h / 2.0 * k2, k);
        let k4 = f(x + h, y + h * k3, k);
        y += h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
        x += h;
        points.push((x, y));

        if !(YMIN - 1.0..=YMAX + 1.0).contains(&y) {
            break;
        }
        for o in level.obstacles {
            if ((x - o.x).powi(2) + (y - o.y).powi(2)).sqrt() < o.r + OBJ_R {
                collided = true;
                break;
            }
        }
        if collided {
            break;
        }
        let (bx, by) = level.b;
        if ((x - bx).powi(2) + (y - by).powi(2)).sqrt() < WIN_R {
            reached = true;
            break;
        }
    }
    SimResult {
        points,
        collided,
        reached,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_levels_load() {
        assert_eq!(levels().len(), 4);
    }

    #[test]
    fn level1_is_winnable() {
        let lv = &levels()[0];
        let mut k = lv.k_min;
        let mut won = false;
        while k <= lv.k_max {
            if integrate(lv, k).reached {
                won = true;
                break;
            }
            k += 0.05;
        }
        assert!(won, "aucun k ne permet d'atteindre B sur le niveau 1");
    }

    #[test]
    fn same_k_gives_same_unique_path() {
        // Cauchy-Lipschitz : à champ fixé (k fixé), une seule solution issue de A.
        let lv = &levels()[2];
        let r1 = integrate(lv, 1.0);
        let r2 = integrate(lv, 1.0);
        assert_eq!(r1.points, r2.points);
    }
}
