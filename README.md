<p align="center">
  <img width="500" height="500" src="./images/pinepods-logo.jpeg">
</p>

# PinePods :evergreen_tree:
[![](https://dcbadge.vercel.app/api/server/bKzHRa4GNc)](https://discord.gg/bKzHRa4GNc)
[![Chat on Matrix](https://matrix.to/img/matrix-badge.svg)](https://matrix.to/#/#pinepods:matrix.org)
![Docker Container Build](https://github.com/madeofpendletonwool/PinePods/actions/workflows/docker-publish.yml/badge.svg)
[![GitHub Release](https://img.shields.io/github/v/release/madeofpendletonwool/pinepods)](https://github.com/madeofpendletonwool/PinePods/releases)

- [PinePods :evergreen\_tree:](#pinepods-evergreen_tree)
- [Getting Started](#getting-started)
  - [Features](#features)
  - [Try it out! :zap:](#try-it-out-zap)
  - [Installing :runner:](#installing-runner)
    - [Server Installation :floppy\_disk:](#server-installation-floppy_disk)
      - [Compose File](#compose-file)
      - [Admin User Info](#admin-user-info)
      - [Proxy Info](#proxy-info)
      - [Note on the Search API](#note-on-the-search-api)
      - [Client API Vars](#client-api-vars)
      - [Start it up!](#start-it-up)
    - [Linux Client Install :computer:](#linux-client-install-computer)
    - [Windows Client Install :computer:](#windows-client-install-computer)
    - [Mac Client Install :computer:](#mac-client-install-computer)
    - [Android Install :iphone:](#android-install-iphone)
    - [ios Install :iphone:](#ios-install-iphone)
  - [Pinepods Firewood](#pinepods-firewood)
  - [Platform Availability](#platform-availability)
  - [ToDo](#todo)
  - [Screenshots :camera:](#screenshots-camera)

# Getting Started

PinePods is a Rust based podcast management system that manages podcasts with multi-user support and relies on a central database with clients to connect to it. It's browser based and your podcasts and settings follow you from device to device due to everything being stored on the server. It works on mobile devices and can also sync with a Nextcloud server so you can use external apps like Antennapod as well!

For more information than what's provided in this repo visit the [documentation site](https://www.pinepods.online/).

<p align="center">
  <img src="./images/screenshots/homethemed.png">
</p>

## Features

Pinepods is a complete podcast management system and allows you to play, download, and keep track of podcasts you (or any of your users) enjoy. It allows for searching new podcasts using The Podcast Index or Itunes and provides a modern looking UI to browse through shows and episodes. In addition, Pinepods provides simple user managment and can be used by multiple users at once using a browser or app version. Everything is saved into a MySQL or Postgres database (alternative database support is on the roadmap) including user settings, podcasts and episodes. It's fully self-hosted, open-sourced, and I provide an option to use a hosted search API or you can also get one from the Podcast Index and use your own. There's even many different themes to choose from! Everything is fully dockerized and I provide a simple guide found below explaining how to install and run Pinepods on your own system.

## Try it out! :zap:

I try and maintain an instance of Pinepods that's publicly accessible for testing over at [try.pinepods.online](https://try.pinepods.online). Feel free to make an account there and try it out before making your own server instance. This is not intended as a permanent method of using Pinepods and it's expected you run your own server; accounts will often be deleted from there.

## Installing :runner:

There's potentially a few steps to getting Pinepods fully installed. After you get your server up and running fully you can also install the client editions of your choice. The server install of Pinepods runs a server and a browser client over a port of your choice in order to be accessible on the web. With the client installs you simply give the client your server url to connect to the database and then sign in.

### Server Installation :floppy_disk:

First, the server. You have multiple options for deploying Pinepods:

  - [Using Docker Compose :whale:](#docker-compose)
  - [Using Helm for Kubernetes :anchor:](#helm-deployment)

You can also choose to use MySQL/MariaDB or Postgres as your database. Examples for both are provided below.

### Docker Compose

#### Compose File - MariaDB

```
version: '3'
services:
  db:
    image: mariadb:latest
    command: --wait_timeout=1800
    environment:
      MYSQL_TCP_PORT: 3306
      MYSQL_ROOT_PASSWORD: myS3curepass
      MYSQL_DATABASE: pinepods_database
      MYSQL_COLLATION_SERVER: utf8mb4_unicode_ci
      MYSQL_CHARACTER_SET_SERVER: utf8mb4
      MYSQL_INIT_CONNECT: 'SET @@GLOBAL.max_allowed_packet=64*1024*1024;'
    volumes:
      - /home/user/pinepods/sql:/var/lib/mysql
    ports:
      - "3306:3306"
    restart: always
  pinepods:
    image: madeofpendletonwool/pinepods:latest
    ports:
    # Pinepods Main Port
      - "8040:8040"
    environment:
      # Basic Server Info
      SEARCH_API_URL: 'https://search.pinepods.online/api/search'
      # Default Admin User Information
      USERNAME: myadminuser01
      PASSWORD: myS3curepass
      FULLNAME: Pinepods Admin
      EMAIL: user@pinepods.online
      # Database Vars
      DB_TYPE: mariadb
      DB_HOST: db
      DB_PORT: 3306
      DB_USER: root
      DB_PASSWORD: myS3curepass
      DB_NAME: pinepods_database
      # Enable or Disable Debug Mode for additional Printing
      DEBUG_MODE: False
    volumes:
    # Mount the download and the backup location on the server if you want to. You could mount a nas to the downloads folder or something like that.
    # The backups directory is used if backups are made on the web version on pinepods. When taking backups on the client version it downloads them locally.
      - /home/user/pinepods/downloads:/opt/pinepods/downloads
      - /home/user/pinepods/backups:/opt/pinepods/backups
    depends_on:
      - db
```

#### Compose File - postgres

```

services:
  db:
    image: postgres:latest
    environment:
      POSTGRES_DB: pinepods_database
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: myS3curepass
      PGDATA: /var/lib/postgresql/data/pgdata
    volumes:
      - /home/user/pinepods/pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    restart: always
  pinepods:
    image: madeofpendletonwool/pinepods:latest
    ports:
    # Pinepods Main Port
      - "8040:8040"
    environment:
      # Basic Server Info
      SEARCH_API_URL: 'https://search.pinepods.online/api/search'
      # Default Admin User Information
      USERNAME: myadminuser01
      PASSWORD: myS3curepass
      FULLNAME: Pinepods Admin
      EMAIL: user@pinepods.online
      # Database Vars
      DB_TYPE: postgresql
      DB_HOST: db
      DB_PORT: 5432
      DB_USER: postgres
      DB_PASSWORD: myS3curepass
      DB_NAME: pinepods_database
      # Enable or Disable Debug Mode for additional Printing
      DEBUG_MODE: False
    volumes:
      # Mount the download location on the server if you want to. You could mount a NAS to this folder or something similar
      - /home/user/pinepods/downloads:/opt/pinepods/downloads
      - /home/user/pinepods/backups:/opt/pinepods/backups
    depends_on:
      - db

```

Make sure you change these variables to variables specific to yourself.

```
      MYSQL_ROOT_PASSWORD: password
      SEARCH_API_URL: 'https://search.pinepods.online/api/search'
      USERNAME: pinepods
      PASSWORD: password
      FULLNAME: John Pinepods
      EMAIL: john@pinepods.com
      DB_PASSWORD: password # This should match the MSQL_ROOT_PASSWORD
```

Most of those are pretty obvious, but let's break a couple of them down.

#### Admin User Info

First of all, the USERNAME, PASSWORD, FULLNAME, and EMAIL vars are your details for your default admin account. This account will have admin credentails and will be able to log in right when you start up the app. Once started you'll be able to create more users and even more admins but you need an account to kick things off on. If you don't specify credentials in the compose file it will create an account with a random password for you but I would recommend just creating one for yourself.


#### Note on the Search API

Let's talk quickly about the searching API. This allows you to search for new podcasts and it queries either itunes or the podcast index for new podcasts. The podcast index requires an api key while itunes does not. If you'd rather not mess with the api at all simply set the API_URL to the one below.

```
SEARCH_API_URL: 'https://search.pinepods.online/api/search'
```

Above is an api that I maintain. I do not guarantee 100% uptime on this api though, it should be up most of the time besides a random internet or power outage here or there. A better idea though, and what I would honestly recommend is to maintain your own api. It's super easy. Check out the API docs for more information on doing this. Link Below -

https://www.pinepods.online/docs/API/search_api


#### Start it up!

Either way, once you have everything all setup and your compose file created go ahead and run

```
sudo docker-compose up
```

To pull the container images and get started. Once fully started up you'll be able to access pinepods at the port you configured and you'll be able to start connecting clients as well.


### Helm Deployment

Alternatively, you can deploy Pinepods using Helm on a Kubernetes cluster. Helm is a package manager for Kubernetes that simplifies deployment.
Adding the Helm Repository

First, add the Pinepods Helm repository:

```
helm repo add pinepods http://helm.pinepods.online/PinePods
helm repo update
```
#### Installing the Chart

To install the Pinepods Helm chart, run:

```
helm install pinepods pinepods/pinepods -f my-values.yaml --namespace pinepods-namespace
```
#### Customizing Values

Create a my-values.yaml file to override default values - Leave DB_HOST as it is unless you package your own helm chart:

```
replicaCount: 2

image:
  repository: pinepods
  tag: latest
  pullPolicy: IfNotPresent

service:
  type: NodePort
  port: 8040
  nodePort: 30007

persistence:
  enabled: true
  accessMode: ReadWriteOnce
  size: 10Gi

postgresql:
  enabled: true
  auth:
    username: postgres
    password: "supersecretpassword"
    database: pinepods_database
  primary:
    persistence:
      enabled: true
      existingClaim: postgres-pvc

env:
  SEARCH_API_URL: "https://search.pinepods.online/api/search"
  USERNAME: "admin"
  PASSWORD: "password"
  FULLNAME: "Admin User"
  EMAIL: "admin@example.com"
  DB_TYPE: "postgresql"
  DB_HOST: "pinepods-postgresql.pinepods-namespace.svc.cluster.local"
  DB_PORT: "5432"
  DB_USER: "postgres"
  DB_NAME: "pinepods_database"
  DEBUG_MODE: "false"
```

#### Create a namespace for Pinepods:

Create a namespace to hold the deployment:
```
kubectl create namespace pinepods-namespace
```

#### Starting Helm

Once you have everything set up, install the Helm chart:
```
helm install pinepods pinepods/Pinepods -f my-values.yaml
```
This will deploy Pinepods on your Kubernetes cluster with a postgres database. MySQL/MariaDB is not supported with the kubernetes setup. The service will be accessible at the specified NodePort.

Check out the Tutorials on the documentation site for more information on how to do basic things.

https://pinepods.online/tutorial-basic/sign-in-homescreen.md

### Linux Client Install :computer:

Any of the client additions are super easy to get going. First head over to the releases page on Github

https://github.com/madeofpendletonwool/PinePods/releases

Grab the latest linux release. There's both an app image and a deb. Use the appimage of course if you aren't using a debian based distro. Change the permissions if using the appimage version to allow it to run.

```
sudo chmod +x pinepods.appimage
```

^ The name will vary slightly based on the name so be sure you change it or it won't work.

Once started you'll be able to sign in with your username and password. The server name is simply the url you browse to to access the server.

### Windows Client Install :computer:

Any of the client additions are super easy to get going. First head over to the releases page on Github

https://github.com/madeofpendletonwool/PinePods/releases

There's a exe and msi windows install file.

The exe will actually start an install window and allow you to properly install the program to your computer.

The msi will simply run a portable version of the app.

Either one does the same thing ultimately and will work just fine.

Once started you'll be able to sign in with your username and password. The server name is simply the url you browse to to access the server.

### Mac Client Install :computer:

Any of the client additions are super easy to get going. First head over to the releases page on Github

https://github.com/madeofpendletonwool/PinePods/releases

There's a dmg and pinepods_mac file.

Simply extract, and then go into Contents/MacOS. From there you can run the app.

The dmg file will prompt you to install the Pinepods client into your applications fileter while the _mac file will just run a portable version of the app.

Once started you'll be able to sign in with your username and password. The server name is simply the url you browse to to access the server.

### Android Install :iphone:

Coming Soon - The web app works great for phones. Otherwise, if you sync using Nextcloud you can use the AntennaPods app and your podcasts will sync between Antennapod and Pinepods.

### ios Install :iphone:

Coming Soon - The web app works great for phones.

## Pinepods Firewood

A CLI only client that can be used to remotely share your podcasts to is in the works! Check out [Pinepods Firewood!](https://github.com/madeofpendletonwool/pinepods-firewood)

## Platform Availability

The Intention is for this app to become available on Windows, Linux, Mac, Android, and IOS. Windows, Linux, Mac, and web are all currently available and working. For Android you can use AntennaPod and sync podcasts between AntennaPod and Pinepods using the Nextcloud sync App.

[Nextcloud Podcast Sync App](https://apps.nextcloud.com/apps/gpoddersync)

[AntennaPod F-Droid AppListing](https://f-droid.org/en/packages/de.danoeh.antennapod/)

ARM devices are also supported including raspberry pis. The app is shockingly performant on a raspberry pi as well. The only limitation is that a 64bit OS is required on an arm device. Setup is exactly the same, just use the latest tag and docker will auto pull the arm version.

## ToDo pre-1.0

- [x] Additional Downloads Page organization - Organize by Podcast
- [x] Download entire podcast button. For episode archival
- [x] Offline mode for playing locally downloaded episodes
- [x] Installable PWA
- [x] Custom Podcast Start and End Position
- [x] Jump to clicked timestamp
- [x] Timestamps in playing page
- [x] Chapter Image Support
- [x] Drag to reorganize queue
- [x] Allow for reading transcrips
- [x] Add Funding Links when offered
- [x] Chapter images
- [ ] Restore Server via GUI
- [ ] Login with Github integration and cloud logins (OAuth) Potentially utilize https://authjs.dev/ to make this process easy.
- [ ] Ensure descriptions appear when searching itunes podcasts. This will take some very fast external parsing.
- [ ] Add Fyyd as searching index
- [ ] Client sharing. Search network for other clients and play to them Lightweight client. I'm building a terminal based version called Pinepods Firewood, which will do this. Chromecast support will also be added.
- [ ] Subscription filtering (The ability to search within a given podcast for specific keywords. Give additional searching options, such as searching based on length of episodes)
- [ ] Youtube subscriptions. Subscribe to youtube channels to get subscriptions based on the videos. Audio only.
- [ ] How-to guides on doing things in the app
- [ ] *Pinepods Firewood*. A light, terminal based client used as a remote streaming device or to just listen to podcasts in your terminal! No GUI
- [ ] Add highlight to indicate which page you're on
- [ ] More useful Podcast saving. Perhaps implementing a tagging system for users to make tagged groups of podcasts.
- [ ] Suggestions page - Create podcasts you might like based on the ones you already added
- [ ] Playlist Priority - Similar to podcast republic
- [ ] Better queue interaction. There should be a way to drop down current queue and view without changing route
- [ ] Rating System
- [ ] Implement Podroll to Podcast page when offered
- [ ] Sharing System - Podcast Share implemented, episode coming in next minor update!
- [ ] Option to use login images as background throughout app.
- [ ] Guest Parsing and search page. Use people tags to show other podcasts a specific guest is part of
- [ ] Link Sharing to a podcast to share and allow people to listen to that episode on the server without logging in
- [ ] Side load audio from the web

### Clients to support

- [ ] Flatpak Client - https://www.reddit.com/r/flatpak/comments/xznfbu/how_to_build_the_tauri_app_into_flatpak/
- [ ] Nix Package
- [x] Helm Chart and repo for kubernetes deployment
- [ ] Mobile Apps
  - [ ] Android App
    - [ ] Android Auto support
  - [ ] IOS App
  - [ ] Packaging and automation

## Long term goals

- [ ] Podcast ad blocking. Either by parsing audio blocks with ai and filtering ads or by utilizing a centralized server to allow others to send their ad block info to after determining the timestamps for ads.


## Screenshots :camera:

Main Homepage with podcasts displayed
<p align="center">
  <img src="./images/screenshots/homethemed.png">
</p>

Loads of themes!
<p align="center">
  <img src="./images/screenshots/home.png">
</p>
<p align="center">
  <img src="./images/screenshots/homelight.png">
</p>

Full Podcast Management
<p align="center">
  <img src="./images/screenshots/podpage.png">
</p>

Browse through episodes
<p align="center">
  <img src="./images/screenshots/podview.png">
</p>

Markdown and HTML display compatible
<p align="center">
  <img src="./images/screenshots/markdownview.png">
</p>

Mobile support baked right in!
<p align="center">
  <img src="./images/screenshots/mobile.png">
</p>
