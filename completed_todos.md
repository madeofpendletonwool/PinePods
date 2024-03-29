# Completed todos

This is the list of previous todos that are now completed

Version 0.4.1

- [x] Fixed issue where get_user_episode_count wasn't displaying episode numbers. There was a syntax error in the api call
- [x] Added /api/data/podcast_episodes and /api/data/get_podcast_id api calls. These are needed for Pinepods Firewood


Version 0.4

- [x] Unlock api creation for standard users - The API has been completely re-written to follow along the permissions that users actually have. Meaning users can easily request their own api keys and sign into the client with admin consent
- [x] Signing into the client edition is now possible with either an API key or username and password sign in. It gives the option to choose which you would prefer. 
- [x] Email resets currently broken for non-admins due to lockdown on encryption key. Need to handle encryption server-side
- [x] Client version images load a lot faster now
- [x] Fixed issue with audio container not reappearing after entering playing fullscreen
- [x] Fixed Issue with Queue Bump Not working right
- [x] Added verification when deleting user

Version 0.3.1

- [x] Finalize reverse proxy processes and web playing

Version 0.3

- [x] Export and import of following podcasts (basically user backups)
- [x] Entire Server Backup and Import. This allows you to export and import your entire database for complete backups
- [x] New refresh system added to automatically update podcasts in database with no user input.
- [x] Reworked the controls displayed on the page to be components of a class. This should improve performance.
- [x] fixed issues with logging in on small screens. (a big step for mobile version) 
- [x] Bug fixing such as fixing queue bump, and fixing an audio changing issue - Along with quite a few random UI bug fixing throughout

Version 0.2

- [x] Implement custom urls for feeds
- [x] Organize folder layout in the same way as the client when server downloading

Version 0.1

- [X] Create Code that can pull Podcasts
- [X] Integrate Podcast Index
- [X] Play Audio Files using Python - Flet's Audio library is used
- [X] Record listen history and display user history on specific page
- [X] Record accurate listen time. So if you stop listening part-way through you can resume from the same spot
- [X] Scrubbing playback from a progress bar - ft.slider()
- [X] Visual progress bar based on time listened to podcasts partly listened to
- [X] Download option for podcasts. In addition, display downloaded podcasts in downloads area. Allow for deletion of these after downloaded
- [X] Queue, and allow podcasts to be removed from queue once added (Queue is added but you can't remove them from it yet)
- [X] Login screen
- [X] Episode view (Also display html in descriptions via markdown)
- [X] Multiple Themes (like 10 I think?)
- [X] Add picture of current episode to soundbar
- [X] Complete user management with admin options
- [X] Ability to Delete Users
- [X] Allow guest user to be disabled (Is disabled by default)
- [X] Ensure changes cannot be made to guest user
- [X] Ensure Users cannot delete themselves
- [X] Guest sign in via button on login screen when enabled
- [X] Saved episodes view
- [X] Caching image server (proxy)
- [X] Optional user self service creation
- [X] User stats page
- [X] Implement sign in retention. (App retention now works. It creates session keys and stores them locally. Browser retention is next, this will need some kind of oauth)
- [X] Audio Volume adjustment options
- [X] Create Web App
  - [X] Responsive layout
  - [X] Security and Logins
  - [X] Database interaction for users and podcast data
- [x] Fully update Readme with updated info and docs including deployment guide
- [X] Bugs
  - [X] Links when searching an episode are blue (wrong color)
  - [X] When changing theme, then selecting 'podcasts' page, the navbar does not retain theme
  - [X] There's an issue with Queue not working properly. Sometimes it just plays instead of queues (Fixed when switching to flet audio control)
  - [X] Clicking podcast that's already been added displays add podcast view with no current way to play
  - [X] Clicking play buttons on a podcast while another is loading currently breaks things
  - [X] Pausing audio changes font color
  - [X] Login screen colors are wrong on first boot
  - [X] Themeing currently wrong on audio interaction control
  - [X] Starting a podcast results in audio bar being in phone mode on application version (This should be fixed. I load the check screensize method now further down the page. Which results in consistent width collection.)
  - [X] Starting a podcast results in audio bar being in phone mode on application version
  - [X] Adding a podcast with an emoji in the description currently appears to break it
  - [X] Layout breaks when pausing for podcast names
  - [X] The queue works but currently does not remove podcasts after switching to a new one
  - [X] Resume is currently broken (it now works but it double plays an episode before resuming for some reason. It still double plays and there's not a great way to fix it. Return later. Updates to flet are likely to help eventually)
  - [X] Double check 2 users adding the same podcast (There was an issue with checking playback status that is now fixed)
  - [X] After refresh auto update current route
  - [X] Double and triple check all interactions to verify functionality
  - [X] Fix any additional browser playback bugs (Audio now routes properly through the proxy)
- [x] Dockerize
  - [X] Package into Container/Dockerfile
  - [X] Pypods image in docker hub
  - [X] Create Docker-Compose Code
  - [X] Mixed content - Currently running http or https content can cause an error
  - [x] Option to run your own local podcast index api connection
- [x] Implement Gravitar API for profile picture
- [x] Make web version utilize API Routes instead of database connections directly
- [x] Update flet dependancy to v6 (This fixes audio routing)
- [x] Ability to disable downloads (for public servers)
- [x] One set of functions. Currently client and web app uses different function set. This is be changed for consistency. 
- [x] GUI Wrapper for App
  - [x] Server Hosting and client Interaction - Client interaction works via API with mariadb which is hosted on server side
  - [x] Options to create API keys on the web client as well as ability to remove them
  - [x] Linux App
    - [x] Install Script
    - [x] Packaging and automation
  - [X] Proper web layout
  - [x] Windows App
    - [x] Packaging and automation
  - [x] Mac App
    - [x] Packaging and automation
- [x] Self Service PW Resets
- [x] Add creator info to bottom of stats page
- [x] Default User Creation (Default User is now created if user vars aren't specified in compoose file)
- [x] Issue with web search bar may be due to appbar (This was a rabbit hole. Turns out this was due to the way the top bar was created prior to the routes. I needed to rebuild how searching is done, but this is now fixed)
- [x] Occasionally podcasts will put seconds value in mins (This was a bug due to duration parsing. Code fixed, everything now displays properly)
- [x] Fix client pooling issue (This is a tough issue. Pooling is occasionally a problem. I set the idle timeout to kill old connections and I also fixed a couple database connections that didn't run cnx.close) Edit: I actually think this is truly fixed now. I rebuilt the way this works using async, no problems so far
- [x] Rebuild image Pulling process. The current one is just unworkable (It runs a lot better now. It spawns 4 workers to handle image gathering. Though it still isn't perfect, it hangs a bit occationally but for the time being it's totally usable)
- [x] Layout Settings page better
- [x] MFA Logins
- [x] Allow local downloads to just download the mp3 files direct (Likely only possible on app version)
- [x] Add Itunes podcast API
- [x] MFA Logins on web version
- [x] Do something when search results aren't found - Currently Blank screen
- [x] Implement smoother scrolling with big list loading (I've started a fix for this. ListViews are now active and working right on home and podview)
- [x] Option to remove from history
- [x] Reload not needed to add and remove episodes from pages
- [x] Add mfa to dynamic settings class
- [x] Add new users to dynamic settings class
- [x] Add Email settings to dynamic users class
- [x] logout on client remove saved app cache (Implemented button in settings to clear cache)
- [x] On top bar cutoff add a search button that opens a search prompt (There's a small version of the search button now)
- [x] custom timezone entry
- [x] MFA Display totp secret
- [x] Fix guest with timezone stuff
- [x] 2.0 description features 
- [x] Mass downloading episodes. Entire podcast at once (Implemented but I'm working on getting it to display on download page to see status)
- [x] Remove local podcasts if podcast is no longer in database - Handle this somehow - Mass delete feature added
- [x] Speed up database queries (Indexing added to episodes and podcasts)
- [x] Check local downloads if already downloaded
- [x] Allow description view on podcasts not added
- [x] Configure some kind of auto-refresh feature - Refreshes now on first boot and once every hour
- [x] Mass download options not working on web
- [x] Issue with loading poddisplay on web
- [x] Search options missing from web (Restored - Entirely due to flet jank from app to web)
- [x] Small layout Improvements (Try, complete layout overhaul actually)
- [x] Apparently I broke itunes searching (description addition was causing a problem)
- [x] Internal Episode Search
- [x] Refresh causes episode to restart
- [x] Fix logout - It's shows navbar still
- [x] Refresh with nothing in database breaks things
- [x] Revamp queue - It should just save to the database
- [x] Refresh changes on readme
- [x] API documentation (Site Built with Docusaurus)