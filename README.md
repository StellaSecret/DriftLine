# Courants — Rust + Dioxus

Portage du prototype HTML en Rust/Dioxus, structuré pour coller à ta CI existante.

## Structure
- `core/` — logique pure (niveaux, champs vectoriels 2D, intégration RK4, diagnostics, tests). C'est ce
  que `cargo test --workspace` exécute dans le job `rust-core`.
- `app/` — crate `peoplemodeler-app` (Dioxus). Rendu du champ en SVG (portable
  web + mobile, pas de canvas/web-sys). C'est la cible de
  `cargo check --target wasm32-unknown-unknown -p peoplemodeler-app` et de
  `dx build --release --package peoplemodeler-app`.

## Lancer en local
```bash
cargo test --workspace          # logique et application
dx serve --package peoplemodeler-app   # web, http://localhost:8080
dx build --package peoplemodeler-app --platform android  # APK/AAB (job build-android)
```

## Ce qui diffère de la version JS
- Moteur de champs vectoriels 2D : les quatre niveaux utilisent des vitesses `(vx, vy)` et une intégration RK4 temporelle.
- Chaque session génère une variation de chaque thème : sonde, balise, obstacles et paramètres du courant restent dans les bornes du terrain.
- Le générateur rejette les candidates invalides ou sans réglage gagnant, avec un niveau de repli déterministe; aucune variation mission n'est livrée sans solution.
- La graine hexadécimale est visible, copiable et réapplicable pour retrouver exactement le même set de niveaux.
- Le réglage propose des pas précis, les trois dernières trajectoires restent visibles et les échecs indiquent le passage le plus proche ou le point d'impact.
- Le mode Mission conserve les objectifs et collisions; le mode Exploration libre désactive les collisions, conserve toutes les trajectoires et affiche la référence de la balise.
- Au largage, la trajectoire se dessine progressivement en SVG; les points de
  proximité et d'impact apparaissent à la fin du tracé.
- Le champ est dessiné en SVG plutôt qu'en `<canvas>`, pour que le même code
  fonctionne tel quel sur web (wasm) et sur Android (rendu natif Dioxus).

## CI
Le job Rust exécute `cargo test --workspace`, puis vérifie la cible WASM.
Le build web et le build Android sont produits par Dioxus.
