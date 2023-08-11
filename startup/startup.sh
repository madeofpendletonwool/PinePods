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
# Database Setup
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setupdatabase.py
# Create Admin User
# python3 /pinepods/create_user.py $DB_USER $DB_PASSWORD $DB_HOST $DB_NAME $DB_PORT "$FULLNAME" "$USERNAME" $EMAIL $PASSWORD
# Start the proxy server
nohup gunicorn --bind 0.0.0.0:${PROXY_PORT:-8000} --workers 4 --timeout 30 pinepods.imageserver.wsgi:app &
# Start the FastAPI client api
python3 /pinepods/clients/clientapi.py --port ${API_SERVER_PORT:-8032} &
# Start cron
service cron start
# Add to the cron job to call the script every hour
chmod +x /pinepods/startup/call_refresh_endpoint.sh
echo "Starting a Podcast Refresh"
./pinepods/startup/call_refresh_endpoint.sh
echo "0 * * * * /pinepods/startup/call_refresh_endpoint.sh" | crontab -
# Start PinePods
python3 -u /pinepods/pypods.py