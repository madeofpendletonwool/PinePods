<p align="center">
  <img width="300" height="300" src="./images/Pypods-logos_blue.jpeg">
</p>

# PyPods

- [PyPods](#PyPods)
  - [Features](#Features)
  - [Hosting](#Hosting)
  - [Installing/Running](#Installing/Running)
  - [ToDo](#ToDo)
  - [Platform Availability](#Platform-Availability)
  - [API Notes](#API-Notes)
  - [Screenshots](#Screenshots)
      
PyPods will be a Python based app that can sync podcasts for individual accounts that relies on a central database with a web frontend and apps available on multiple platforms

## Features
N/A

## Hosting
N/A

## Installing/Running
N/A

## ToDo

 - [x] Create Code that can pull Podcasts
 - [x] Integrate Podcast Index
 - [x] Play Audio Files using Python - The python vlc package is used for this
 - [x] Record listen history and display user history on specific page
 - [x] Record accurate listen time. So if you stop listening part-way through you can resume from the same spot
 - [x] Scrubbing playback from a progress bar - ft.slider()
 - [x] Add visual progress bar based on time listened to podcasts partly listened to
 - [x] Add Download option for podcasts. In addition, display downloaded podcasts in downloads area. Allow for deletion of these after downloaded
 - [x] Add Queue, and allow podcasts to be removed from queue once added (Queue is added but you can't remove them from it yet)
 - [x] Create login screen
 - [x] Check for and remove podcasts no longer available (This will be handled from scheduled cron job that queues)
 - [x] Check user values when adding new user
 - [x] Prevent user from being added without required info 
 - [x] Prevent submit for user from being hit without populated values
 - [x] Figure out why some podcasts don't appear in search (This was because of the old podcast index python package. Rebuilt using requests and now it works great)
 - [x] Implement resume playback throughout all areas of the app
 - [x] Implement Episode view (Should be able to display html via markdown)
 - [x] Theme settings
 - [x] Fix issues with episodes playing not in database (Sorta fixed. For now episodes played are always in database. External to database episodes coming soon)
 - [x] Add picture of current episode to soundbar
 - [ ] Fix issue with podcasts sometimes not registering time when played
 - [ ] Second bar can sometimes lag a bit. Need to optimize
 - [ ] Implement smoother scrolling with big list loading
 - [ ] Episode Streaming via external web client doesn't currently work
 - [ ] Implement download episode checking throughout
 - [ ] Implement saved episodes view
 - [ ] Allow local downloads, to just download the mp3 files direct
 - [ ] Customize login screen
 - [ ] Admin area for User management
 - [ ] Remove Podcasts from search or just don't allow adding a second time
 - [ ] Add Itunes podcast API
 - [ ] Dockerize
     - [ ] Package into Container/Dockerfile
     - [ ] Pypods image in docker hub
     - [ ] Create Docker-Compose Code
 - [ ] Create Web App
     - [ ] More responsive layout 
     - [x] Security and Logins
     - [ ] Database interaction for users and podcast data
     - [ ] MFA Logins - Github integration and local MFA
 - [ ] GUI Wrapper for App
     - [ ] Server Hosting and client Interaction - Client interaction works via API with mariadb which is hosted on server side
     - [ ] Linux App
     - [x] Proper web layout
     - [ ] Windows App
     - [ ] Mac App
     - [ ] Android App
     - [ ] IOS App
  - [ ] Add caching to image server
  - [ ] Add loading wheels throughout
  - [ ] Layout soundbar properly (it adjusts for screensize but can overlap at times with the episode title)
  - [ ] Fix local images
  - [ ] The math is currently wrong on the queued time


## Platform Availability

The Intention is for this app to become available on Windows, Linux, Mac, Android, and IOS. The server will be run from docker and connect to the clients on these platforms

## API Notes

Coming soon

## Screenshots

<p align="center">
  <img src="./images/podlist.png">
</p>