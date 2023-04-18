<p align="center">
  <img width="500" height="500" src="./images/pinepods-logo.jpeg">
</p>

# PinePods

- [PinePods](#pinepods)
  - [Features](#features)
  - [Testing](#Testing)
  - [Installing](#installing)
  - [ToDo](#todo)
    - [Needed pre-beta release](#needed-pre-beta-release)
    - [To be added after beta version](#to-be-added-after-beta-version)
  - [Platform Availability](#platform-availability)
  - [API Notes](#api-notes)
  - [Screenshots](#screenshots)
      
PinePods is a Python based app that can sync podcasts for individual accounts that relies on a central database with a web frontend and apps available on multiple platforms

## Features
Pinepods is a complete podcasts management system and allows you to play, download, and keep track of podcasts you enjoy. It allows for searching new podcasts using The Podcast Index and provides a modern looking UI to browse through shows and episodes. In addition, Pinepods provides simple user managment and can be used by multiple users at once using a browser or app version. Everything is saved into a Mysql database including user settings, podcasts and episodes. It's fully self-hosted, and I provide an option to use a hosted API or you can also get one from the podcast API and use your own. There's even many different themes to choose from! Everything is fully dockerized and I provide a simple guide found below explaining how to install Pinepods on your own system. 

## Testing

I try and maintain an instance of Pinepods that's publicly accessible for testing over at [pinepods.online](https://pinepods.online). Feel free to make an account there and try it out before making your own server instance. This is not intended as a permanant method of using Pinepods and it's expected you run your own server and so accounts will often be deleted from here.  

## Installing
There's potentially a few steps to getting Pinepods fully installed as after you get your server up and running fully. You can also install the client editions of your choice. The server install of Pinepods runs a server and a browser client over a port of your choice in order to be accessible on the web. With the client installs you simply give your install a specific url to connect to the database and then sign in. 

### Server Installation

First, the server. It's hightly recommended you run the server using docker compose. Here's the docker compose yaml needed.
#### Compose File
```
version: '3'
services:
  db:
    image: mariadb:latest
    environment:
      MYSQL_TCP_PORT: 3306
      MYSQL_ROOT_PASSWORD: password
      MYSQL_DATABASE: pypods_database
      MYSQL_COLLATION_SERVER: utf8mb4_unicode_ci
      MYSQL_CHARACTER_SET_SERVER: utf8mb4
      MYSQL_INIT_CONNECT: 'SET @@GLOBAL.max_allowed_packet=64*1024*1024;'
    ports:
      - "3306:3306"
    restart: always
  pinepods-proxy:
    image: madeofpendletonwool/pinepods-proxy:latest
    ports:
      - "8033:8000"
    restart: always
  pinepods:
    image: madeofpendletonwool/pinepods:latest
    ports:
      - "8034:8034"
    environment:
      # Default Admin User Information
      USERNAME: pinepods
      PASSWORD: password
      FULLNAME: John Pinepods
      EMAIL: john@pinepods.com
      # Database Vars
      DB_HOST: db
      DB_PORT: 3306
      DB_USER: root
      DB_PASSWORD: password
      DB_NAME: pypods_database
      # Image/Audio Proxy Vars
      PROXY_HOST: proxy.pinepods.online
      PROXY_PORT: 8033
      PROXY_PROTOCOL: https
      REVERSE_PROXY: "True"

    depends_on:
      - db
      - pinepods-proxy

```
Make sure you change these variables to variables specific to yourself. 
```
      MYSQL_ROOT_PASSWORD: password
      USERNAME: pinepods
      PASSWORD: password
      FULLNAME: John Pinepods
      EMAIL: john@pinepods.com
      DB_PASSWORD: password # This should match the MSQL_ROOT_PASSWORD
      PROXY_HOST: proxy.pinepods.online
      PROXY_PORT: 8033
      PROXY_PROTOCOL: http
      REVERSE_PROXY: "True"
```

Most of those are pretty obvious, but let's break a couple of them down. 

#### Admin User Info
First of all, the USERNAME, PASSWORD, FULLNAME, and EMAIL vars are your details for your default admin account. This account will have admin credentails and will be able to log in right when you start up the app. Once started you'll be able to create more users and even more admins but you need an account to kick things off on. If you don't specify credentials in the compose file it will create an account with a random password for you but I would recommend just creating one for yourself. 

#### Proxy Info
Second, the PROXY_HOST, PROXY_PORT, PROXY_PROTOCOL, and REVERSE_PROXY vars. Pinepods uses a proxy to route both images and audio files in order to prevent CORs issues in the app (Essentially so podcast images and audio displays correctly and securely). It uses a second container to accomplish this. That's the pinepods-proxy portion of the compose file. The application itself will then use this proxy to route media though. This proxy also be ran over a reverse proxy. Here's few examples

**Recommended**
Routed through proxy, secure, with reverse proxy
```
      PROXY_HOST: proxy.pinepods.online
      PROXY_PORT: 8033
      PROXY_PROTOCOL: https
      REVERSE_PROXY: "True"
```
Note: With reverse proxies you create a second proxy host. So for example my Pinepods instance itself runs at port 8034 at pinpods.online so my reverse proxy reflects that and I have a dns record for the domain created for pinepods.online to point to my public ip. In addition, my proxy is ran at port 8033 over domain proxy.pinepods.online. I created a seperate dns record for this pointed to my public ip.

Also Note: If you run pinepods over reverse proxy to secure it you **must** run the proxy server over reverse proxy as well to prevent mixed content in the browser 

Direct to ip, insecure, and no reverse proxy
```
      PROXY_HOST: 192.168.0.30
      PROXY_PORT: 8033
      PROXY_PROTOCOL: http
      REVERSE_PROXY: "False"
```

Hostname, secure, and no reverse proxy
```
      PROXY_HOST: proxy.pinepods.online
      PROXY_PORT: 8033
      PROXY_PROTOCOL: https
      REVERSE_PROXY: "False"
```

Note: Changing REVERSE_PROXY to False adjusts what the application uses for the reverse proxy. In short it removed the port from the url it uses for routing since the reverse proxy will add the port for you. 

So REVERSE_PROXY "True"
https://proxy.pinepods.online
REVERSE_PROXY "False"
https://proxy.pinepods.online:8033

#### API Notes
Coming Soon

#### Start it up!

Either way, once you have everything all setup and your compose file created go ahead and run your 
```
sudo docker-compose up
```
command to pull the container images and get started. Once fully started up you'll be able to access pinepods on the url you configured and you'll be able to start connecting clients as well.

### Linux Client Install
Coming Soon

### Windows Client Install
Coming Soon

### Mac Client Install
Coming Soon

### Android Install
Coming Soon

### ios Install
Coming Soon

## Platform Availability

The Intention is for this app to become available on Windows, Linux, Mac, Android, and IOS. The server will be run from docker and connect to the clients on these platforms

## ToDo
 - [x] Create Code that can pull Podcasts
 - [x] Integrate Podcast Index
 - [x] Play Audio Files using Python - Flet's Audio library is used
 - [x] Record listen history and display user history on specific page
 - [x] Record accurate listen time. So if you stop listening part-way through you can resume from the same spot
 - [x] Scrubbing playback from a progress bar - ft.slider()
 - [x] Visual progress bar based on time listened to podcasts partly listened to
 - [x] Download option for podcasts. In addition, display downloaded podcasts in downloads area. Allow for deletion of these after downloaded
 - [x] Queue, and allow podcasts to be removed from queue once added (Queue is added but you can't remove them from it yet)
 - [x] Login screen
 - [x] Episode view (Also display html in descriptions via markdown)
 - [x] Multiple Themes (like 10 I think?)
 - [x] Add picture of current episode to soundbar
 - [x] Complete user management with admin options
 - [x] Ability to Delete Users
 - [x] Allow guest user to be disabled (Is disabled by default)
 - [x] Ensure changes cannot be made to guest user
 - [x] Ensure Users cannot delete themselves
 - [x] Guest sign in via button on login screen when enabled
 - [x] Saved episodes view
 - [x] Caching image server (proxy)
 - [x] Optional user self service creation
 - [x] User stats page
 - [x] Implement sign in retention. (App retention now works. It creates session keys and stores them locally. Browser retention is next, this will need some kind of oauth)
 - [x] Audio Volume adjustment options 
 - [x] Create Web App
     - [x] Responsive layout 
     - [x] Security and Logins
     - [x] Database interaction for users and podcast data

 ### Needed pre-beta release
 - [ ] Fully update Readme with updated info and docs including deployment guide
 - [x] Bugs
    - [x] Links when searching an episode are blue (wrong color)
    - [x] When changing theme, then selecting 'podcasts' page, the navbar does not retain theme
    - [x] There's an issue with Queue not working properly. Sometimes it just plays instead of queues (Fixed when switching to flet audio control)
    - [x] Clicking podcast that's already been added displays add podcast view with no current way to play
    - [x] Clicking play buttons on a podcast while another is loading currently breaks things
    - [x] Pausing audio changes font color
    - [x] Login screen colors are wrong on first boot
    - [x] Themeing currently wrong on audio interaction control
    - [x] Starting a podcast results in audio bar being in phone mode on application version (This should be fixed. I load the check screensize method now further down the page. Which results in consistent width collection.)
    - [x] Starting a podcast results in audio bar being in phone mode on application version
    - [x] Adding a podcast with an emoji in the description currently appears to break it
    - [x] Layout breaks when pausing for podcast names
    - [x] The queue works but currently does not remove podcasts after switching to a new one
    - [x] Resume is currently broken (it now works but it double plays an episode before resuming for some reason. It still double plays and there's not a great way to fix it. Return later. Updates to flet are likely to help eventually)
    - [x] Double check 2 users adding the same podcast (There was an issue with checking playback status that is now fixed)
    - [x] After refresh auto update current route
    - [x] Double and triple check all interactions to verify functionality
    - [x] Fix any additional browser playback bugs (Audio now routes properly through the proxy)
 - [ ] Dockerize
     - [x] Package into Container/Dockerfile
     - [x] Pypods image in docker hub
     - [x] Create Docker-Compose Code
     - [x] Mixed content - Currently running http or https content can cause an error
     - [ ] Option to run your own local podcast index api connection


 ### To be added after beta version

 - [ ] Implement Gravitar API for profile picture
 - [ ] Rotating currently playing
 - [ ] Implement smoother scrolling with big list loading
 - [ ] Suggestions page - Create podcasts you might like based on the ones you already added
 - [ ] Allow local downloads to just download the mp3 files direct
 - [ ] Page refreshing to handle adding and removing of things better
 - [ ] Handle Images better. Currently it takes a long time to parse through many images (Needs to not load all images. Only ones on screen)
 - [ ] Reload not needed to add and remove episodes from pages
 - [ ] Customizable login screen
 - [ ] Add highlight to indicate which page you're on
 - [ ] Add Itunes podcast API
 - [ ] Allow for episodes to be played without being added
 - [ ] Better queue interaction. There should be a way to drop down current queue and view without changing route
 - [ ] MFA Logins - Github integration and local MFA (OAuth)
 - [ ] Implement Browser edition sign in retention (This will require some kind of OAuth provider. Part of OAuth and MFA)
 - [ ] GUI Wrapper for App
     - [ ] Server Hosting and client Interaction - Client interaction works via API with mariadb which is hosted on server side
     - [ ] Linux App
     - [x] Proper web layout
     - [ ] Windows App
     - [ ] Mac App
     - [ ] Mobile Apps
       - [ ] Sign in retention for moble editions
       - [ ] Android App
       - [ ] IOS App
  - [ ] Add verification before deleting user
  - [ ] Rating System
  - [ ] Sharing System

## Screenshots

<p align="center">
  <img src="./images/podlist.png">
</p>