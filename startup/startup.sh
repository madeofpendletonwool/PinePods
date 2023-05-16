#!/bin/bash

export DB_USER=$DB_USER
export DB_PASSWORD=$DB_PASSWORD
export DB_HOST=$DB_HOST
export DB_NAME=$DB_NAME
export DB_PORT=$DB_PORT
export FULLNAME=$FULLNAME
export USERNAME=$USERNAME
export EMAIL=$EMAIL
export PASSWORD=$PASSWORD
export REVERSE_PROXY=$REVERSE_PROXY
export API_URL=$API_URL

cat << "EOF"
         A
        d$b
      .d\$$b.
    .d$i$$\$$b.
       d$$@b
      d\$$$ib
    .d$$$\$$$b
  .d$$@$$$$\$$ib.
      d$$i$$b
     d\$$$$@$b
  .d$@$$\$$$$$@b.
.d$$$$i$$$\$$$$$$b.
        ###
        ###
        ###


 _______   __                                                __           
/       \ /  |                                              /  |          
$$$$$$$  |$$/  _______    ______    ______    ______    ____$$ |  _______ 
$$ |__$$ |/  |/       \  /      \  /      \  /      \  /    $$ | /       |
$$    $$/ $$ |$$$$$$$  |/$$$$$$  |/$$$$$$  |/$$$$$$  |/$$$$$$$ |/$$$$$$$/ 
$$$$$$$/  $$ |$$ |  $$ |$$    $$ |$$ |  $$ |$$ |  $$ |$$ |  $$ |$$      \ 
$$ |      $$ |$$ |  $$ |$$$$$$$$/ $$ |__$$ |$$ \__$$ |$$ \__$$ | $$$$$$  |
$$ |      $$ |$$ |  $$ |$$       |$$    $$/ $$    $$/ $$    $$ |/     $$/ 
$$/       $$/ $$/   $$/  $$$$$$$/ $$$$$$$/   $$$$$$/   $$$$$$$/ $$$$$$$/  
                                  $$ |                                    
                                  $$ |                                    
                                  $$/                                     
A project created and written by Collin Pendleton
collinp@gooseberrydevelopment.com


EOF
# Database Setup
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setupdatabase.py
# Create Admin User
# python3 /pinepods/create_user.py $DB_USER $DB_PASSWORD $DB_HOST $DB_NAME $DB_PORT "$FULLNAME" "$USERNAME" $EMAIL $PASSWORD
# Start the proxy server
nohup gunicorn --bind 0.0.0.0:8000 --timeout 120 pinepods.imageserver.wsgi:app &
# Start the FastAPI client api
python3 /pinepods/clients/clientapi.py &
# Start PinePods
python3 -u /pinepods/pypods.py