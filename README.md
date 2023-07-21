<p align="center">
  <img width="500" height="500" src="./images/pinepods-logo.jpeg">
</p>

# PinePods :evergreen_tree:

- [PinePods :evergreen\_tree:](#pinepods-evergreen_tree)
  - [Features](#features)
  - [Try it out! :zap:](#try-it-out-zap)
  - [Installing :runner:](#installing-runner)
    - [Server Installation :floppy\_disk:](#server-installation-floppy_disk)
      - [Compose File](#compose-file)
      - [Admin User Info](#admin-user-info)
      - [Proxy Info](#proxy-info)
      - [API Notes](#api-notes)
      - [Start it up!](#start-it-up)
    - [Linux Client Install :computer:](#linux-client-install-computer)
    - [Windows Client Install :computer:](#windows-client-install-computer)
    - [Mac Client Install :computer:](#mac-client-install-computer)
    - [Android Install :iphone:](#android-install-iphone)
    - [ios Install :iphone:](#ios-install-iphone)
  - [Platform Availability](#platform-availability)
  - [ToDo](#todo)
    - [Needed pre-beta release](#needed-pre-beta-release)
    - [To be added after beta version](#to-be-added-after-beta-version)
  - [Screenshots :camera:](#screenshots-camera)

PinePods is a Python based app that can sync podcasts for individual accounts that relies on a central database with a web frontend and apps available on multiple platforms

## Features

Pinepods is a complete podcasts management system and allows you to play, download, and keep track of podcasts you enjoy. It allows for searching new podcasts using The Podcast Index and provides a modern looking UI to browse through shows and episodes. In addition, Pinepods provides simple user managment and can be used by multiple users at once using a browser or app version. Everything is saved into a Mysql database including user settings, podcasts and episodes. It's fully self-hosted, and I provide an option to use a hosted API or you can also get one from the podcast API and use your own. There's even many different themes to choose from! Everything is fully dockerized and I provide a simple guide found below explaining how to install Pinepods on your own system.

## Try it out! :zap:

I try and maintain an instance of Pinepods that's publicly accessible for testing over at [pinepods.online](https://pinepods.online). Feel free to make an account there and try it out before making your own server instance. This is not intended as a permanant method of using Pinepods and it's expected you run your own server so accounts will often be deleted from there.

## Installing :runner:

There's potentially a few steps to getting Pinepods fully installed as after you get your server up and running fully. You can also install the client editions of your choice. The server install of Pinepods runs a server and a browser client over a port of your choice in order to be accessible on the web. With the client installs you simply give your install a specific url to connect to the database and then sign in.

### Server Installation :floppy_disk:

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
    volumes:
      - /home/user/pinepods/sql:/var/lib/mysql
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
      - "8032:8032"
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
      PROXY_HOST: pinepods-proxy
      PROXY_PORT: 8033
      PROXY_PROTOCOL: http
      REVERSE_PROXY: "True"
      #Podcast Index API
      API_URL: 'https://api.pinepods.online/api/search'


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
      API_URL: 'https://api.pinepods.online/api/search'
```

Most of those are pretty obvious, but let's break a couple of them down.

#### Admin User Info

First of all, the USERNAME, PASSWORD, FULLNAME, and EMAIL vars are your details for your default admin account. This account will have admin credentails and will be able to log in right when you start up the app. Once started you'll be able to create more users and even more admins but you need an account to kick things off on. If you don't specify credentials in the compose file it will create an account with a random password for you but I would recommend just creating one for yourself.

#### Proxy Info

Second, the PROXY_HOST, PROXY_PORT, PROXY_PROTOCOL, and REVERSE_PROXY vars. Pinepods uses a proxy to route both images and audio files in order to prevent CORs issues in the app (Essentially so podcast images and audio displays correctly and securely). It uses a second container to accomplish this. That's the pinepods-proxy portion of the compose file. The application itself will then use this proxy to route media though. This proxy also be ran over a reverse proxy. Here's few examples

**Recommended:**
Routed through proxy, secure, with reverse proxy

```
      PROXY_HOST: proxy.pinepods.online
      PROXY_PORT: 8033
      PROXY_PROTOCOL: https
      REVERSE_PROXY: "True"
```

*Note*: With reverse proxies you create a second proxy host. So for example my Pinepods instance itself runs at port 8034 at pinpods.online so my reverse proxy reflects that and I have a dns record for the domain created for pinepods.online to point to my public ip. In addition, my proxy is ran at port 8033 over domain proxy.pinepods.online. I created a seperate dns record for this pointed to my public ip.

*Also Note*: If you run pinepods over reverse proxy to secure it you **must** run the proxy server over reverse proxy as well to prevent mixed content in the browser

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

Let's talk about the API. The variable in the compose file

```
API_URL: 'https://api.pinepods.online/api/search'
```

This is an api that I maintain to forward search queries to the podcast index which returns results based on the search term you passed to it. You can leave this variable default, and if you do you'll be using the api that I maintain for this. I do not guarantee 100% uptime on this api though, it should be up most of the time bar a random internet or power outage here or there. A better idea though, and what I would honestly recommend is to maintain your own api. It's super easy

Head over to the podcast index API website and sign up to get your very own api and key. It's free and makes everything extra secure.
[Podcast Index API Website](https://api.podcastindex.org/)

Once you have it. Use this docker compose file

```
version: '3'
services:
    pypods-backend:
       image: madeofpendletonwool/pinepods_backend:latest
       container_name: pypods-be
       env_file: env_file
       ports:
            - 5000:5000
       restart: unless-stopped
```
You also need to create the env file. It should contain your api key and secret NOTE: You MUST use the env file. Docker compose will not interpret certain characters if not in an env file. Don't smash your face against that issue for hours like I did
env_file
```
API_KEY=your_api_key
API_SECRET=your_api_secret
```

Now go ahead and ```sudo docker-compose up``` your file. Then, in the pinepods compose file update the api_url.

```
API_URL: 'http://<YOUR_IP>/api/search'
```

Or, even better, stick this behind a reverse proxy with your own domain as well.

```
API_URL: 'https://<YOUR_DOMAIN>/api/search'
```

#### Start it up!

Either way, once you have everything all setup and your compose file created go ahead and run your

```
sudo docker-compose up
```

command on the main pinepods app to pull the container images and get started. Once fully started up you'll be able to access pinepods on the url you configured and you'll be able to start connecting clients as well.

### Linux Client Install :computer:

Coming Soon

### Windows Client Install :computer:

Coming Soon

### Mac Client Install :computer:

Coming Soon

### Android Install :iphone:

Coming Soon

### ios Install :iphone:

Coming Soon

## Platform Availability

The Intention is for this app to become available on Windows, Linux, Mac, Android, and IOS. The server will be run from docker and connect to the clients on these platforms

## ToDo
### Pre-beta version

- [ ] Refresh changes on readme
- [ ] Full Screen Currently Playing Page (Mostly implemented. There's a couple bugs on the web version to fix)
- [ ] Rework local images to run through the image proxy for web
- [ ] API documentation (Site Built with Docusaurus)
- [ ] Revamp queue - It should just save to the database
- [ ] Fix logout - It's shows navbar still
- [ ] playing page not currently removing playing bar on bottom in app version

### To be added after beta version (Listed in order they will be implemented)

- [ ] Internal Episode Search
- [ ] Mass delete options not appearing in web version. This seems to be a bug. It works totally fine in client app
- [ ] Implement page views for poddisplays that have over 30 episodes
- [ ] Jump to clicked timestamp
- [ ] Offline mode for playing locally downloaded episodes
- [ ] Allow for episodes to be played without being added
- [ ] Add highlight to indicate which page you're on
- [ ] Suggestions page - Create podcasts you might like based on the ones you already added
- [ ] Make scrolling screens roll up more. So that the currently playing episode doesn't get in the way of your view
- [ ] Rotating currently playing
- [ ] Customizable login screens
- [ ] Better queue interaction. There should be a way to drop down current queue and view without changing route
- [ ] MFA Logins - Github integration and cloud logins (OAuth)
- [ ] Implement Browser edition sign in retention (This will require some kind of OAuth provider. Part of OAuth and MFA)
- [ ] Linux App    
  - [ ] Flatpak
  - [ ] Snap
- [ ] Mobile Apps
  - [ ] Sign in retention for mobile editions
  - [ ] Android App
  - [ ] IOS App
  - [ ] Packaging and automation
- [ ] Add verification before deleting user
- [ ] Rating System
- [ ] Sharing System

## Screenshots :camera:

Main Homepage with podcasts displayed
<p align="center">
  <img src="./images/screenshots/homethemed.png">
</p>

Loads of themes!
<p align="center">
  <img src="./images/screenshots/home.png">
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