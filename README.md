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

## Pédagogie
Huit chapitres sont enseignés dans cet ordre, chacun avec son tutoriel généré puis ses vraies zones :

1. **Suivre** — le champ est fixe, la sonde ne fait que le suivre (tutoriel de démonstration).
2. **Position** — l'intensité est fixée, le largage est libre.
3. **Prédire** — la trajectoire est masquée : il faut deviner où elle mène.
4. **Influencer** — seule l'intensité est libre.
5. **Fil** — deux fils voisins, qui échouent tous les deux, encadrent la bonne intensité.
6. **Cadence** — la phase du champ est dialable : l'instant du largage compte autant que l'intensité.
7. **Composer** — un champ composé de plusieurs composantes pondérées.
8. **Coordonner** — plusieurs balises à toucher dans un même vol.

Chaque tutoriel est généré à partir du `TutorialSpec` de son chapitre, indépendamment du vrai
programme : le jeu réel peut être redessiné sans casser la leçon. Un tutoriel n'est jamais une porte
dûre — l'ouvrir suffit à débloquer le chapitre, sans avoir à le réussir — et il porte un badge une fois
terminé. Les chapitres sont générés à la demande : un tutoriel coûte quelques millisecondes, pas un
cursus complet.

## Ce qui diffère de la version JS
- Moteur de champs vectoriels 2D : chaque zone utilise des vitesses `(vx, vy)` et une intégration RK4 temporelle.
- Les huit mécaniques ci-dessus remplacent les six anciennes. Le tutoriel d'un chapitre est le premier
  emplacement de jeu du chapitre ; le vrai jeu suit derrière, et il compte 31 zones au total.
- Le réglage de phase est continu : il fige le champ à l'instant choisi (`Level::resolved`) au lieu
  d'animer un champ vivant, ce qui garde la déduction lisible. Un chapitre qui n'a qu'un seul instant
  n'affiche pas de curseur, et un réglage épinglé par le chapitre est annoncé (« fixé : phase »).
- La progression, la variante du tutoriel et la zone en cours sont sauvegardées dans `localStorage`
  (`serde_json`) et restaurées au chargement, une fois la lecture terminée — jamais écrasées par un état vide.
- La progression derrière les tutoriels reste séquentielle : une zone se débloque en réussissant la
  précédente. Le sélecteur « Toutes les zones » permet de parcourir le curriculum complet par groupe.
- La zone de sensibilité construit sa balise à partir de la marge de réglages gagnante mesurée par le
  solveur (`k_window`), et l'interface affiche cette marge en indice pendant le Laboratoire.
- Chaque session génère une variation de chaque thème : sonde, balises, obstacles et paramètres du courant restent dans les bornes du terrain.
- Le générateur rejette les candidates invalides ou sans réglage gagnant, avec un niveau de repli déterministe; aucune variation jouable n'est livrée sans solution.
- Chaque partie part d'une graine aléatoire; « Nouvelle zone » en tire une nouvelle, « Copier la graine » la recopie pour retrouver exactement le même set de niveaux.
- Le réglage propose des pas précis, les trois dernières trajectoires restent visibles et les échecs indiquent le passage le plus proche ou le point d'impact. Les parcours multi-balises rappellent le nombre de balises touchées.
- L'Exploration est le mode par défaut : les astéroïdes bloquent la sonde dans les deux modes, chaque balise touchée compte pour débloquer la zone suivante, toutes les trajectoires sont conservées et les balises ne s'affichent qu'en référence. La flèche de lancement suit le courant et pivote avec l'intensité. Le chapitre Suivre garde les vecteurs comme tutoriel, puis ils sont masqués.
- Le mode Laboratoire ajoute les collisions d'astéroïdes et les vecteurs du champ en permanence.
- Au largage, la trajectoire se dessine progressivement en SVG; les points de
  proximité et d'impact apparaissent à la fin du tracé.
- Le champ est dessiné en SVG plutôt qu'en `<canvas>`, pour que le même code
  fonctionne tel quel sur web (wasm) et sur Android (rendu natif Dioxus).

## CI
Le job Rust exécute `cargo test --workspace`, puis vérifie la cible WASM.
Le build web et le build Android sont produits par Dioxus.
