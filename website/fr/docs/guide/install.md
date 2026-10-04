---
description: "Construire et installer Sioul à partir de ses sources, sur Linux d’abord ; notes pour Windows et macOS."
---

# Installer {#install}

## Télécharger {#download}

Les paquets de chaque version sont sur [la page des versions](https://github.com/aurelienpierre/sioul/releases/latest), construits par GitHub à partir des sources de cette version :

| Système | Paquet | Comment |
|---|---|---|
| Windows 10 et 11 (64 bits) | `sioul-<version>-setup.exe` | Lancez-le. Windows peut dire qu’il ne connaît pas l’éditeur, car le paquet n’est pas encore signé : **Informations complémentaires**, puis **Exécuter quand même**. |
| macOS 13 et plus récent, puces Apple et Intel | `Sioul-<version>-macos-universal.dmg` | Ouvrez-le et glissez Sioul dans Applications. Il n’est pas encore notarié : la première fois, ouvrez-le depuis Applications, puis **Réglages Système ▸ Confidentialité et sécurité ▸ Ouvrir quand même**. |
| Linux, toute distribution (64 bits) | `Sioul-<version>-x86_64.AppImage` | Rendez-le exécutable (`chmod +x Sioul-*.AppImage`), puis lancez-le. Sur Ubuntu 24.04 et plus récent, ses sites tournent sans le bac à sable de Chromium : préférez-y le Flatpak. |
| Linux, avec Flatpak | `Sioul-<version>.flatpak` | `flatpak install --user Sioul-<version>.flatpak` : l’environnement KDE et Qt WebEngine viennent de Flathub. |

Chacun contient aussi la ligne de commande, `sioul`. Ces premiers paquets ont été construits et testés par GitHub, mais peu de personnes les ont encore lancés : un mot dans les [tickets GitHub](https://github.com/aurelienpierre/sioul/issues) aide.

## Ou le construire à partir de ses sources {#or-build-it-from-its-sources}

La suite de cette page construit Sioul à partir de ses sources : pour les distributions Linux qui n’ont pas encore de paquet, et pour suivre les derniers changements.

## Ce qu’il faut {#what-it-needs}

- **Rust** 1.89 ou plus récent, avec Cargo.
- **Un compilateur C++** : la fenêtre est en partie écrite en C++.
- **Les fichiers de développement de D-Bus**, sur Linux : pour le trousseau du système, où vont les mots de passe, et pour les notifications.
- **Qt 6.9 ou plus récent** (Sioul est construit et utilisé avec Qt 6.11), avec ces modules : Qt Declarative (QML et Qt Quick), Qt WebEngine avec Qt WebChannel et Qt PDF, Qt Multimedia, Qt Positioning, Qt Location et Qt Image Formats.

En option, et installables plus tard :

- **ClamAV**, pour faire vérifier les pièces jointes avant qu’elles s’ouvrent. Sans lui, Sioul vous dit qu’un fichier ne sera pas vérifié, et demande avant de l’ouvrir.
- **Tesseract et Poppler**, pour lire le courrier papier que vous scannez. Sans eux, les scans attendent, non lus. Voir [Les papiers et les lettres](papers.md#paper-letters).

## Sur Linux {#on-linux}

### Les paquets {#the-packages}

=== "Fedora"

    ```
    sudo dnf install git rust cargo gcc-c++ dbus-devel \
        qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtwebengine-devel \
        qt6-qtwebchannel-devel qt6-qtpdf-devel qt6-qtpositioning-devel \
        qt6-qtmultimedia qt6-qtlocation qt6-qtimageformats
    ```

    En option : l’antivirus, puis ses signatures, tenues à jour :

    ```
    sudo dnf install clamav clamav-update
    sudo freshclam
    sudo systemctl enable --now clamav-freshclam
    ```

    En option : la lecture du courrier papier, en français et en anglais :

    ```
    sudo dnf install tesseract tesseract-langpack-fra poppler-utils
    ```

=== "Debian, Ubuntu, Arch"

    Les fichiers de développement de D-Bus, un compilateur C++, et Qt 6.9 ou plus récent avec les modules ci-dessus, et leurs modules QML. Les distributions les nomment différemment : Debian et Ubuntu `qt6-…-dev` et `qml6-module-…`, Arch `qt6-…`. Votre version de la distribution doit fournir Qt 6.9 ou plus récent.

    Rust depuis [rustup.rs](https://rustup.rs) quand celui de votre version est plus ancien que 1.89.

Sioul est développé sur Fedora : les autres systèmes n’ont pas été essayés. S’il manque un module, la construction s’arrête, ou la fenêtre ne s’ouvre pas et Qt nomme le module manquant dans ses messages (`QT_FORCE_STDERR_LOGGING=1 sioul-app` les affiche dans le terminal). Un mot dans les [tickets GitHub](https://github.com/aurelienpierre/sioul/issues) aide alors la personne suivante.

### Construire {#building}

```
git clone https://github.com/aurelienpierre/sioul.git
cd sioul
cargo build --release
cargo build --release -p sioul-app
```

La première ligne `cargo build` construit `sioul`, la ligne de commande ; la seconde construit `sioul-app`, la fenêtre. La première construction télécharge et compile quelques centaines de bibliothèques : elle prend un moment, et quelques gigaoctets de disque.

### Dans votre menu d’applications {#into-your-application-menu}

```
install -Dm755 target/release/sioul-app ~/.local/bin/sioul-app
install -Dm755 target/release/sioul ~/.local/bin/sioul
install -Dm644 data/com.aurelienpierre.Sioul.desktop ~/.local/share/applications/com.aurelienpierre.Sioul.desktop
install -Dm644 data/com.aurelienpierre.Sioul.metainfo.xml ~/.local/share/metainfo/com.aurelienpierre.Sioul.metainfo.xml
mkdir -p ~/.local/share/icons && cp -r data/icons/hicolor ~/.local/share/icons/
```

Sioul apparaît alors dans votre menu d’applications sous le nom « Sioul ». La ligne de commande `sioul` vaut la peine d’être installée aussi : les rappels s’en servent quand la fenêtre est fermée ([Paramètres](settings.md#reminders-and-notifications)).

### Mettre à jour {#updating}

Dans le dossier `sioul` : `git pull`, puis à nouveau les deux lignes `cargo build` et les trois lignes `install`. Vos réglages et vos données sont gardés à part du programme, dans vos propres dossiers : ils restent tels quels.

## L’essayer d’abord sur du courrier inventé {#trying-it-on-invented-mail-first}

Depuis le dossier `sioul`, la ligne de commande peut montrer le Porche sur quelques messages inventés, sans aucun compte :

```
cargo run -q -- --config examples/demo.toml porch
```

## Sur Windows {#on-windows}

Pas encore essayé. Les étapes, pour qui veut :

1. Rust, par [rustup](https://rustup.rs), avec les outils de construction de Microsoft (MSVC).
2. Qt 6.9 ou plus récent depuis l’installateur en ligne de Qt, pour MSVC 2022 64 bits, avec les modules ci-dessus, et le dossier `bin` de Qt dans votre `PATH`.
3. `cargo build --release -p sioul-app`.
4. `windeployqt --release --qmldir crates\sioul-app\qml target\release\sioul-app.exe` rassemble Qt à côté du programme. `packaging\windows\sioul.iss` fait un installateur avec Inno Setup.

Sur Windows, les pièces jointes sont vérifiées par Microsoft Defender, par son interface d’analyse (Antimalware Scan Interface). Les rappels ne viennent que tant que la fenêtre est ouverte.

## Sur macOS {#on-macos}

Pas encore essayé. Les étapes sont dans [packaging/macos/README.md (en anglais)](https://github.com/aurelienpierre/sioul/blob/main/packaging/macos/README.md) : Qt 6.9 ou plus récent depuis l’installateur en ligne de Qt, Rust depuis rustup, puis `cargo build --release -p sioul-app -p sioul-cli` et un paquet fait avec `macdeployqt` ; la construction automatique sur GitHub peut faire le même `.dmg` quand on la lance à la main. ClamAV depuis Homebrew (`brew install clamav`) vérifie les pièces jointes quand il est là. Le paquet n’est pas encore signé : macOS vous demande de confirmer la première ouverture (clic droit, Ouvrir).

## Ensuite {#next}

[Premiers pas](first-steps.md) : ajouter votre courrier, vos agendas et contacts, Google, et les sites que vous consultez.
