# Courants — Rust + Dioxus

Portage du prototype HTML en Rust/Dioxus, structuré pour coller à ta CI existante.

## Structure
- `core/` — logique pure (niveaux, champs, intégration RK4, tests). C'est ce
  que `cargo test` (working-directory: core) exécute dans le job `rust-core`.
- `app/` — crate `peoplemodeler-app` (Dioxus). Rendu du champ en SVG (portable
  web + mobile, pas de canvas/web-sys). C'est la cible de
  `cargo check --target wasm32-unknown-unknown -p peoplemodeler-app` et de
  `dx build --release --package peoplemodeler-app`.

## Lancer en local
```bash
cd core && cargo test          # logique
dx serve --package peoplemodeler-app   # web, http://localhost:8080
dx build --package peoplemodeler-app --platform android  # APK/AAB (job build-android)
```

## Ce qui diffère de la version JS
- Même moteur (mêmes 4 champs, mêmes niveaux, même intégration RK4).
- Pas d'animation image par image au largage : la trajectoire s'affiche en
  entier immédiatement (au lieu de se dessiner progressivement). Facile à
  ajouter ensuite avec un `use_future` + minuteur si tu veux la retrouver.
- Le champ est dessiné en SVG plutôt qu'en `<canvas>`, pour que le même code
  fonctionne tel quel sur web (wasm) et sur Android (rendu natif Dioxus).

## À adapter dans ta CI
Rien de spécial : les noms de package (`peoplemodeler-app`, workspace avec
`core` comme sous-dossier testé isolément) correspondent déjà à ton
`build.yml`. Il faudra juste un `package.json` minimal si `npm ci` /
Playwright doivent tourner contre ce nouveau `app/`.
