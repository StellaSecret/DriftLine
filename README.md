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
- Moteur de champs vectoriels 2D : les trente zones utilisent des vitesses `(vx, vy)` et une intégration RK4 temporelle.
- Six mécaniques sont enseignées dans l'ordre, cinq zones chacune : courant constant, tourbillons, zones
  (courant constant puis bandes horizontales), zones opposées, sensibilité à `k`, et parcours à plusieurs
  balises à toucher dans un même vol (un seul réglage, ordre libre).
- La progression est séquentielle : une zone se débloque en réussissant la précédente, et les groupes sont
  générés à la demande (`generate_level_group`) au lieu des trente zones d'un coup. Le sélecteur « Toutes les
  zones » permet de parcourir le curriculum complet par groupe.
- La zone de sensibilité construit sa balise à partir de la marge de réglages gagnante mesurée par le
  solveur (`k_window`), et l'interface affiche cette marge en indice pendant le Laboratoire.
- Chaque session génère une variation de chaque thème : sonde, balises, obstacles et paramètres du courant restent dans les bornes du terrain.
- Le générateur rejette les candidates invalides ou sans réglage gagnant, avec un niveau de repli déterministe; aucune variation jouable n'est livrée sans solution.
- Chaque partie part d'une graine aléatoire; « Nouvelle zone » en tire une nouvelle, « Copier la graine » la recopie pour retrouver exactement le même set de niveaux.
- Le réglage propose des pas précis, les trois dernières trajectoires restent visibles et les échecs indiquent le passage le plus proche ou le point d'impact. Les parcours multi-balises rappellent le nombre de balises touchées.
- L'Exploration est le mode par défaut : les astéroïdes bloquent la sonde dans les deux modes, chaque balise touchée compte pour débloquer la zone suivante, toutes les trajectoires sont conservées et les balises ne s'affichent qu'en référence. La flèche de lancement suit le courant et pivote avec l'intensité. Le premier groupe de cinq zones garde les vecteurs comme tutoriel, puis ils sont masqués.
- Le mode Laboratoire ajoute les collisions d'astéroïdes et les vecteurs du champ en permanence.
- Au largage, la trajectoire se dessine progressivement en SVG; les points de
  proximité et d'impact apparaissent à la fin du tracé.
- Le champ est dessiné en SVG plutôt qu'en `<canvas>`, pour que le même code
  fonctionne tel quel sur web (wasm) et sur Android (rendu natif Dioxus).

## CI
Le job Rust exécute `cargo test --workspace`, puis vérifie la cible WASM.
Le build web et le build Android sont produits par Dioxus.
