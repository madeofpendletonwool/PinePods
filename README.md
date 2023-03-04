<p align="center">
  <img width="300" height="300" src="./images/logo.png">
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

 - [x] Start
 - [x] Create Code that can pull Podcasts
 - [x] Integrate Podcast Index
 - [x] Play Audio Files using Python - The python vlc package is used for this
 - [ ] Record listen history and display user history on specific page
 - Allow Download of Podcasts in Structured Form into specific location on Computer
 - Create Users/User Functionality - Users can now be added via settings page. Currently, there's no login options. Coming soon
 - Allow for Saving when app is closed - probably from config file that gets saved into config foler that holds data
 - [ ] Implement saving listen time when stopping playback in order to resume later
 - Dockerize
     - Docker Networking
     - Server Hosting and client Interaction - Client interaction works via API with mariadb which is hosted on server side
     - Package into Container/Dockerfile
     - Create Docker-Compose Code
 - Create Web App
     - This will be broken into it's own instance
     - Security and Logins
     - Database interaction for users and podcast data
 - GUI Wrapper for App
     - Linux App
     - Windows App
     - Mac App
     - Android App
     - IOS App

## Platform Availability

The Intention is for this app to become available on Windows, Linux, Mac, Android, and IOS. The server will be run from docker and connect to the clients on these platforms

## API Notes

Coming soon

## Screenshots

<p align="center">
  <img src="./images/podlist.png">
</p>