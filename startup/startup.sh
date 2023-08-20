#!/bin/bash

export DB_USER=$DB_USER
export DB_PASSWORD=$DB_PASSWORD
export DB_HOST=$DB_HOST
export DB_NAME=$DB_NAME
export DB_PORT=$DB_PORT
export FULLNAME=${FULLNAME:-'Pinepods Admin'}
export USERNAME=${USERNAME:-'pine-admin'}
export EMAIL=${EMAIL:-'admin@pinepods.online'}
export PASSWORD=${PASSWORD:-$(head /dev/urandom | tr -dc A-Za-z0-9 | head -c14 ; echo '')}
export REVERSE_PROXY=$REVERSE_PROXY
export API_URL=$API_URL

if [[ $FULLNAME == 'Pinepods Admin' ]]; then
  echo "Admin User Information:"
  echo "FULLNAME: $FULLNAME"
  echo "USERNAME: $USERNAME"
  echo "EMAIL: $EMAIL"
  echo "PASSWORD: $PASSWORD"
fi

cat << "EOF"
         A
        d$b
      .d\$$b.
    .d$i$$\$$b.      _______   __                                                __           
       d$$@b        /       \ /  |                                              /  |          
      d\$$$ib       $$$$$$$  |$$/  _______    ______    ______    ______    ____$$ |  _______ 
    .d$$$\$$$b      $$ |__$$ |/  |/       \  /      \  /      \  /      \  /    $$ | /       |
  .d$$@$$$$\$$ib.   $$    $$/ $$ |$$$$$$$  |/$$$$$$  |/$$$$$$  |/$$$$$$  |/$$$$$$$ |/$$$$$$$/ 
      d$$i$$b       $$$$$$$/  $$ |$$ |  $$ |$$    $$ |$$ |  $$ |$$ |  $$ |$$ |  $$ |$$      \ 
     d\$$$$@$b.     $$ |      $$ |$$ |  $$ |$$$$$$$$/ $$ |__$$ |$$ \__$$ |$$ \__$$ | $$$$$$  |
  .d$@$$\$$$$$@b.   $$ |      $$ |$$ |  $$ |$$       |$$    $$/ $$    $$/ $$    $$ |/     $$/ 
.d$$$$i$$$\$$$$$$b. $$/       $$/ $$/   $$/  $$$$$$$/ $$$$$$$/   $$$$$$/   $$$$$$$/ $$$$$$$/  
        ###                                           $$ |                                    
        ###                                           $$ |                                    
        ###                                           $$/                                     

A project created and written by Collin Pendleton
collinp@gooseberrydevelopment.com


EOF
# Creating cache directory
mkdir -p /pinepods/cache
mkdir -p /opt/pinepods/backups
mkdir -p /opt/pinepods/downloads
# Database Setup
if [[ $DB_TYPE == "postgresql" ]]; then
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setuppostgresdatabase.py
else
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setupdatabase.py
fi


# Start all services with supervisord
supervisord -c /pinepods/startup/supervisord.conf
# Create Admin User
# python3 /pinepods/create_user.py $DB_USER $DB_PASSWORD $DB_HOST $DB_NAME $DB_PORT "$FULLNAME" "$USERNAME" $EMAIL $PASSWORD

# Set up and start cron tasks
service cron start
chmod +x /pinepods/startup/call_refresh_endpoint.sh
echo "Starting a Podcast Refresh"
./pinepods/startup/call_refresh_endpoint.sh
echo "*/30 * * * * /pinepods/startup/call_refresh_endpoint.sh" | crontab -

# Start Pinepods Reverse Proxy last
python3 -u /pinepods/startup/fastapirouter.py