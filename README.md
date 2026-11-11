# Paf le Bengal

Deux petits jeux d'arcade, **Baseball** et **Altitude** (gore désactivable),
pour Packarch, écrits en Rust
avec [macroquad](https://macroquad.rs). Les protagonistes : **Baby Kart**, un
grand chat tigré gris humanoïde, et le **Gremlin du Bengal**.

Tout est dessiné par le code : aucune image ni aucun son à charger.

## Paf le Bengal : Baseball (`packarch-paf-le-bengal`)

Le Gremlin du Bengal saute de son arbre à chat ; Baby Kart le frappe avec une
batte entourée de fil barbelé pour l'envoyer le plus loin possible.

- **Clic / Espace** : faire sauter le bengal, puis **frapper** au bon moment
  (le timing décide de la puissance et de l'angle).
- Des **mines** sont posées sur le terrain : BOUM, le bengal repart en l'air.
- À l'arrivée : gros plan, le bengal part en morceaux.

## Paf le Bengal : Altitude (`packarch-paf-le-bengal-altitude`)

Baby Kart fait tournoyer le Gremlin du Bengal par la queue et le lâche vers le
ciel. Il faut monter le plus haut possible en rebondissant sur des pelotes de
laine géantes, jusque dans l'espace.

- **Maintenir clic / Espace** : faire tourner le bengal (l'élan s'accélère) ;
  **relâcher** quand il passe dans l'arc vert pour l'envoyer vers le haut.
  Trop longtemps à pleine vitesse… la queue lâche.
- **Souris ou flèches** (ou A/Q et D) : diriger le bengal en l'air.
- **Pelotes** : on les traverse en montant, on rebondit dessus en descendant
  (rebond centré : plus haut ; rebonds enchaînés : combo).
- **Pigeons** (coup de pouce), **drones-aspirateurs** (cassent l'élan),
  **sardine dorée** (turbo).
- Au-delà de 140 m : l'**espace** (station spatiale, Lune, la Terre qui
  rétrécit à mesure qu'on monte). En retombant… le bengal finit en crêpe.

## Commandes communes

- **G** : activer / désactiver le gore
- **Échap** : quitter

Les records sont enregistrés dans `~/.local/share/packarch-paf-le-bengal/`.

## Compilation

```sh
cargo build --release
./target/release/packarch-paf-le-bengal
./target/release/packarch-paf-le-bengal-altitude
```

Dépendances à l'exécution (chargées dynamiquement) : OpenGL (libglvnd),
X11 (libx11, libxi) ou Wayland (wayland, libxkbcommon).

## Licence

GPL-3.0-or-later (voir `COPYING`).
