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
export PINEPODS_PORT=$PINEPODS_PORT
export PROXY_PROTOCOL=$PROXY_PROTOCOL
export PINEPODS_PORT=$PINEPODS_PORT

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
mkdir -p /opt/pinepods/certs

openssl req -x509 -nodes -newkey rsa:4096 -keyout /opt/pinepods/certs/key.pem -out /opt/pinepods/certs/cert.pem -days 365 -subj "/C=US/ST=NY/L=NewYork/O=PinePods/CN=$HOSTNAME"
echo "127.0.0.1 $HOSTNAME" >> /etc/hosts
echo "Hosts file written and can be seen below:"
cat /etc/hosts

# Database Setup
if [[ $DB_TYPE == "postgresql" ]]; then
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setuppostgresdatabase.py
else
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setupdatabase.py
fi

echo "*/30 * * * * /pinepods/startup/call_refresh_endpoint.sh" | crontab -

# Start all services with supervisord
if [[ $PROXY_PROTOCOL == "http" ]]; then
supervisord -c /pinepods/startup/supervisord.conf
else
supervisord -c /pinepods/startup/supervisord.conf
fi
# Create Admin User
# python3 /pinepods/create_user.py $DB_USER $DB_PASSWORD $DB_HOST $DB_NAME $DB_PORT "$FULLNAME" "$USERNAME" $EMAIL $PASSWORD
