FROM ubuntu:latest

LABEL maintainer="Collin Pendleton <collinp@collinpendleton.com>"

ARG DEBIAN_FRONTEND=noninteractive

# Create location where pypods code is stored
# RUN mkdir /cecil
# Make sure the package repository is up to date. Also install needed packages via apt
RUN apt update && \
    apt -qy upgrade && \
    apt -qy upgrade && \
    apt install -qy python3 git software-properties-common python3-pip curl cron supervisor
# Install needed python packages via pip
ADD ./requirements.txt /
RUN pip install -r ./requirements.txt
# Put pypods Files in place
# Create structure for pypods
RUN git clone https://github.com/madeofpendletonwool/pypods.git /opt/ceciltemp && \
    mkdir -p /opt/pypods/ && \
    chmod -R 755 /opt
# Begin cecil Setup
ADD startup.sh /
RUN ls -al /
ENTRYPOINT ["/startup/startup.sh"]
 
