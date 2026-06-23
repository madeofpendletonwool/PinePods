<p align="center">
  <img width="240" src="./images/logo1024square.png" alt="PinePods logo">
</p>

<h1 align="center">PinePods :evergreen_tree:</h1>

<p align="center">
  <em>A forest of podcasts, rooted in the spirit of self-hosting.</em>
</p>

<p align="center">
  <a href="https://discord.gg/bKzHRa4GNc"><img src="https://img.shields.io/badge/discord-join%20chat-5B5EA6" alt="Discord"></a>
  <a href="https://matrix.to/#/#pinepods:matrix.org"><img src="https://matrix.to/img/matrix-badge.svg" alt="Chat on Matrix"></a>
  <a href="https://github.com/madeofpendletonwool/PinePods/actions"><img src="https://github.com/madeofpendletonwool/PinePods/actions/workflows/docker-publish.yml/badge.svg" alt="Docker Container Build"></a>
  <a href="https://github.com/madeofpendletonwool/PinePods/releases"><img src="https://img.shields.io/github/v/release/madeofpendletonwool/pinepods" alt="GitHub Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPLv3-blue" alt="License"></a>
</p>

<p align="center">
  <a href="https://flathub.org/apps/com.gooseberrydevelopment.pinepods"><img src="https://flathub.org/api/badge?locale=en" alt="Get it on Flathub" height="50"></a>
  <a href="https://apps.apple.com/us/app/pinepods/id6751441116"><img src="./images/Download_on_the_App_Store_Badge_US-UK_RGB_blk_092917.svg" alt="Download on the App Store" height="50"></a>
  <a href="https://play.google.com/store/apps/details?id=com.gooseberrydevelopment.pinepods"><img src="https://play.google.com/intl/en_us/badges/static/images/badges/en_badge_web_generic.png" alt="Get it on Google Play" height="50"></a>
  <a href="https://apt.izzysoft.de/fdroid/index/apk/com.gooseberrydevelopment.pinepods"><img src="https://gitlab.com/IzzyOnDroid/repo/-/raw/master/assets/IzzyOnDroid.png" alt="Get it on IzzyOnDroid" height="50"></a>
  <a href="https://apps.obtainium.imranr.dev/redirect?r=obtainium://app/%7B%22id%22%3A%22com.gooseberrydevelopment.pinepods%22%2C%22url%22%3A%22https%3A//github.com/madeofpendletonwool/PinePods%22%2C%22author%22%3A%22madeofpendletonwool%22%2C%22name%22%3A%22PinePods%22%2C%22installerUrl%22%3A%22https%3A//github.com/madeofpendletonwool/PinePods/releases/latest%22%7D"><img src="./images/badge_obtainium.png" alt="Get it on Obtainium" height="50"></a>
</p>

<p align="center">
  <img src="./images/screenshots/homepage.png" alt="PinePods home screen" width="900">
</p>

---

## What is PinePods?

**PinePods is a complete, self-hosted podcast management system written in Rust.**

You run one server, your whole household connects to it, and your subscriptions,
history, queue, downloads, and settings follow you from device to device — because
everything lives in your own database. Listen in the browser, on the desktop, on
your phone, in the car, or even from the terminal.

- :house: **Self-hosted & open source** — your podcasts and listening data stay on
  your hardware (Postgres or MySQL/MariaDB).
- :busts_in_silhouette: **Multi-user** — one instance serves your whole family, each
  with their own library, stats, and settings.
- :iphone: **Native apps everywhere** — web, Linux, Windows, macOS, Android, and iOS,
  plus a CLI client.
- :twisted_rightwards_arrows: **Bring your own apps** — a built-in gpodder-compatible
  sync server lets you keep using AntennaPod and friends alongside PinePods.
- :globe_with_meridians: **Speaks your language** — translated into 36 languages by
  the community.

> :point_right: Want the full story, deep configuration, and tutorials? Head to the
> **[documentation site](https://www.pinepods.online/)**.

## Highlights

| | |
|---|---|
| :headphones: **Listen** | Audio **and video** podcasts, variable speed, chapters, transcripts, downloads, a persistent queue panel, **serial auto-play**, and **per-podcast auto-download**. |
| :card_index_dividers: **Organize** | Subscriptions, **smart playlists**, manual playlists, saved episodes, **favorites & favorite categories**, full history, and **local file-system podcasts**. |
| :mag: **Discover** | Search across **Podcast Index, iTunes, and YouTube**, search **inside your own library** for episodes, follow **podcast hosts** via [PodPeople DB](https://podpeopledb.com), and generate **shareable episode links**. |
| :twisted_rightwards_arrows: **Sync & apps** | Built-in **gpodder server** (AntennaPod, etc.), **OIDC / SSO**, **MFA / TOTP**, web + desktop + mobile clients with **CarPlay** and **Android Auto**, plus the **Firewood** CLI. |
| :art: **Make it yours** | Multiple built-in themes plus a **custom theme creator**, detailed **listening stats**, OPML import/export, push notifications (ntfy / webhook), and **36 languages**. |

See the [full feature catalog](https://www.pinepods.online/docs/intro) in the docs.

## Screenshots :camera:

<p align="center"><strong>A home dashboard that picks up where you left off — with dozens of built-in themes plus a custom theme creator</strong></p>
<p align="center">
  <img width="49%" src="./images/screenshots/homepage.png" alt="Home dashboard">
  <img width="49%" src="./images/screenshots/tonsofthemes.png" alt="Theme picker with many themes">
</p>

<p align="center"><strong>Browse your whole library and dig into any show</strong></p>
<p align="center">
  <img width="49%" src="./images/screenshots/podcastlayout.png" alt="Podcast library grid">
  <img width="49%" src="./images/screenshots/singlepodcastpage.png" alt="Single podcast page">
</p>

<p align="center"><strong>One unified feed of every new episode across your subscriptions</strong></p>
<p align="center">
  <img width="800" src="./images/screenshots/feedpage.png" alt="Unified episode feed">
</p>

<p align="center"><strong>Rich episode pages with show notes, chapters, and transcripts</strong></p>
<p align="center">
  <img width="800" src="./images/screenshots/singleepisodepage.png" alt="Episode page with notes, chapters, and transcript tabs">
</p>

<p align="center"><strong>Listen your way — full-screen audio and native video playback</strong></p>
<p align="center">
  <img width="32%" src="./images/screenshots/videoplayer.png" alt="Video player">
  <img width="32%" src="./images/screenshots/fullscreenplayer.png" alt="Full-screen audio player">
  <img width="32%" src="./images/screenshots/smallplayer.png" alt="Mini player">
</p>

<p align="center"><strong>A persistent queue, smart playlists, library search, and detailed stats</strong></p>
<p align="center">
  <img width="49%" src="./images/screenshots/queuepage.png" alt="Queue side panel">
  <img width="49%" src="./images/screenshots/playlistcreator.png" alt="Smart playlist creator">
</p>
<p align="center">
  <img width="49%" src="./images/screenshots/searchpage.png" alt="Search your library">
  <img width="49%" src="./images/screenshots/userstats.png" alt="User statistics dashboard">
</p>

<p align="center"><strong>Mobile apps for iOS & Android, with CarPlay and Android Auto</strong></p>
<p align="center">
  <img width="280" src="./images/screenshots/mobile.png" alt="Mobile home">
  <img width="280" src="./images/screenshots/mobileepisode.png" alt="Mobile episode">
</p>

## Try it out! :zap:

A public demo instance lives at **[try.pinepods.online](https://try.pinepods.online)** —
make an account and take a look before you self-host. It's for evaluation only;
accounts there are wiped periodically, so run your own server for real use.

## Quick Start :rocket:

The fastest way to run PinePods is Docker Compose with PostgreSQL. Create a
`docker-compose.yml`:

```yaml
services:
  db:
    container_name: db
    image: postgres:18
    environment:
      POSTGRES_DB: pinepods_database
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: myS3curepass
      PGDATA: /var/lib/pgdata/pgdata
    volumes:
      - /home/user/pinepods/pgdata:/var/lib/pgdata
    restart: always

  valkey:
    image: valkey/valkey:8-alpine
    restart: always

  pinepods:
    image: madeofpendletonwool/pinepods:latest
    ports:
      - "8040:8040"
    environment:
      # Basic server info
      SEARCH_API_URL: 'https://search.pinepods.online/api/search'
      PEOPLE_API_URL: 'https://people.pinepods.online'
      HOSTNAME: 'http://localhost:8040'
      # Database
      DB_TYPE: postgresql
      DB_HOST: db
      DB_PORT: 5432
      DB_USER: postgres
      DB_PASSWORD: myS3curepass
      DB_NAME: pinepods_database
      # Valkey cache
      VALKEY_HOST: valkey
      VALKEY_PORT: 6379
      DEBUG_MODE: false
      # Run as your host user so downloads stay accessible (optional)
      PUID: ${UID:-911}
      PGID: ${GID:-911}
      # Local timezone (optional, used for logs)
      TZ: "America/New_York"
    volumes:
      - /home/user/pinepods/downloads:/opt/pinepods/downloads
      - /home/user/pinepods/backups:/opt/pinepods/backups
    restart: always
    depends_on:
      - db
      - valkey
```

Then start it:

```bash
sudo docker compose up -d
```

Open `http://localhost:8040` and you'll be prompted to create your first admin
account. That's it — you're up.

> :information_source: **On PostgreSQL 18 / upgrading from 17.** New installs default to
> `postgres:18` and need no special steps. Two things to know when moving an existing
> install to 18:
>
> - **Data isn't auto-migrated across major versions.** `postgres:18` won't start
>   against a data directory created by 17 (`FATAL: database files are incompatible with
>   server`). Your data is safe — run `deployment/docker/upgrade-postgres.sh` (takes a
>   backup, then upgrades in place) or follow the
>   [Upgrading PostgreSQL](https://www.pinepods.online/docs/Troubleshooting/PostgresMajorUpgrade)
>   guide. Back up first — the upgrade is one-way.
> - **The `postgres:18` image moved its data dir and `VOLUME`.** Bind-mounting to the
>   old `/var/lib/postgresql/data` can fail on some Linux/overlay2 hosts with
>   `change mount propagation through procfd ... no such file or directory`. The compose
>   above avoids this by mounting at `/var/lib/pgdata`, outside the image's `VOLUME`;
>   use that same pattern when you upgrade. See
>   [docker-library/postgres#1363](https://github.com/docker-library/postgres/issues/1363).

**Need more?** Helm/Kubernetes, MySQL/MariaDB, admin bootstrap vars, the self-hosted
search API, timezone tuning, PUID/PGID, and OIDC are all covered in the docs:

- :whale: **[Full server install guide](https://www.pinepods.online/docs/intro)**
- :anchor: **Helm chart** — `helm repo add pinepods http://helm.pinepods.online`
- :mag: **[Self-hosting the search API](https://www.pinepods.online/docs/API/search_api)**
- :closed_lock_with_key: **[OIDC / SSO setup](https://www.pinepods.online/docs/tutorial-extras/OIDC-setup)**

## Clients

Run the server, then connect any client by pointing it at your server URL and signing
in. The web client is served by the server itself — the rest are optional native apps.

| Platform | How to get it | Notes |
|---|---|---|
| :globe_with_meridians: **Web** | Built in — browse to your server's port | No install needed |
| :penguin: **Linux** | [Flathub](https://flathub.org/apps/com.gooseberrydevelopment.pinepods), [AUR](https://aur.archlinux.org/packages/pinepods) (`paru -S pinepods`), or AppImage / `.deb` / `.rpm` on [Releases](https://github.com/madeofpendletonwool/PinePods/releases) | Flatpak recommended |
| :window: **Windows** | `.exe` (installer) or `.msi` (portable) on [Releases](https://github.com/madeofpendletonwool/PinePods/releases) | |
| :apple: **macOS** | `.dmg` (installer) or portable build on [Releases](https://github.com/madeofpendletonwool/PinePods/releases) | |
| :iphone: **iOS** | [App Store](https://apps.apple.com/us/app/pinepods/id6751441116) | CarPlay supported |
| :robot: **Android** | [Google Play](https://play.google.com/store/apps/details?id=com.gooseberrydevelopment.pinepods), [IzzyOnDroid](https://apt.izzysoft.de/fdroid/index/apk/com.gooseberrydevelopment.pinepods), or [Obtainium](https://github.com/madeofpendletonwool/PinePods/releases) | Android Auto supported |
| :computer: **Terminal** | [Pinepods Firewood](https://github.com/madeofpendletonwool/pinepods-firewood) (CLI) | |

ARM devices — including 64-bit Raspberry Pis — are fully supported; the `latest` tag
auto-pulls the right architecture. Client setup details live in the
[clients docs](https://www.pinepods.online/docs/tutorial-basics/clients).

## Ecosystem

**PodPeople DB** — A community database that supplements podcast *person* tags so you
can follow hosts across every show they appear on, even when a feed doesn't publish
host info. Use the hosted instance at [podpeopledb.com](https://podpeopledb.com) or
[self-host your own](https://podpeopledb.com/docs/self-host).
[Repo](https://github.com/madeofpendletonwool/podpeople-db) ·
[Why it exists](https://www.pinepods.online/blog).

**Pinepods Firewood** — A terminal-only client for enjoying your podcasts from the
comfort of the command line.
[Check it out](https://github.com/madeofpendletonwool/pinepods-firewood).

**Helm chart** — Deploy on Kubernetes with `helm repo add pinepods
http://helm.pinepods.online`. See the [install docs](https://www.pinepods.online/docs/intro)
for values.

## Docs & Community

- :books: **Documentation:** [pinepods.online](https://www.pinepods.online/)
- :speech_balloon: **Discord:** [join the chat](https://discord.gg/bKzHRa4GNc)
- :speech_balloon: **Matrix:** [#pinepods:matrix.org](https://matrix.to/#/#pinepods:matrix.org)
- :memo: **Blog:** [pinepods.online/blog](https://www.pinepods.online/blog)
- :earth_africa: **Translate PinePods:** contribute on
  [Weblate](https://hosted.weblate.org) — 36 languages and counting.
- :bug: **Issues & contributing:** see [CONTRIBUTING.md](CONTRIBUTING.md) and the
  [issue tracker](https://github.com/madeofpendletonwool/PinePods/issues).

## Credits & Licensing

PinePods is an open-source podcast player developed by **Gooseberry Development** and
licensed under the **GNU General Public License v3.0 (GPL-3.0)**.

The mobile app includes code adapted from the excellent
[Anytime Podcast Player](https://github.com/amugofjava/anytime_podcast_player) by Ben
Hills.

> **Anytime Podcast Player** — © 2020 Ben Hills and contributors, licensed under the
> BSD 3-Clause License. Affected files retain the original BSD license and attribution
> at the top; see `LICENSE.ben_hills` in the `mobile/` directory. Huge thanks to Ben
> Hills for open-sourcing Anytime — it accelerated PinePods' mobile development
> enormously.

ARM container images are made possible by [Runs-On](https://runs-on.com).
