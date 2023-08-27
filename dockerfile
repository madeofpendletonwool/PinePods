FROM python:3.12.0a7-slim

LABEL maintainer="Collin Pendleton <collinp@collinpendleton.com>"

ARG DEBIAN_FRONTEND=noninteractive

# Create location where pinepods code is stored
# RUN mkdir /pinepods
# Make sure the package repository is up to date. Also install needed packages via apt
RUN apt update && \
    apt -qy upgrade && \
    apt install -qy git software-properties-common curl cron supervisor gcc libffi-dev zlib1g-dev libjpeg-dev mariadb-client libpq-dev openssl && \
    rm -rf /var/lib/apt/lists/*

# Install needed python packages via pip
ADD ./requirements.txt /
RUN pip install -r ./requirements.txt

COPY wait-for-it/wait-for-it.sh /wait-for-it.sh
RUN chmod +x /wait-for-it.sh

# Add a cache-busting build argument
ARG CACHEBUST=1

# Put pinepods Files in place
# Create structure for pinepods
RUN git clone https://github.com/madeofpendletonwool/pypods.git /pinepods && \
    chmod -R 755 /pinepods

# Begin pinepods Setup
ADD startup/startup.sh /
RUN ls -al /
ENTRYPOINT ["/startup.sh"]
