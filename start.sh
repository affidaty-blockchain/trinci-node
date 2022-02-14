#!/bin/bash

# Node vars.
TARGET_PORT=8001
HTTP_PORT=8000
BS_PATH='./data/bootstrap.bin'
BS_IP_ADDR='15.161.71.249' 
BS_ADDR='@/ip4/15.161.71.249/tcp/9006'
DB_PATH='./db/'

# Output settings.
STEP_CODE="\033[0;33m"
CLEAN_CODE="\033[0m"
SUCCESS_CODE="\033[0;32m"
ERROR_CODE="\033[0;31m"

echo -e "\nTrinci node start script v0.0.1 \n"

echo -e "${SUCCESS_CODE}Gatering network informations... \n${CLEAN_CODE}"
# Recover local ip.
# Check which command use.
if command -v ip &> /dev/null
then
    local_ip=`ip addr | sed -n -e '/state UP/,/[0-9]: / p' | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | tr '\n' '|'`
    local_ip=${local_ip%?}
fi

if command -v ifconfig &> /dev/null
then
    local_ip=`ifconfig | sed -n -e '/UP/,/[0-9]: / p' | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | tr '\n' '|'`
    local_ip=${local_ip%?}
fi

if [ $? -ne 0 ]; then
    echo -e "${ERROR_CODE}Something went wrong checking local ip addresses. \n${CLEAN_CODE}"
    exit 1
fi

echo -e "${STEP_CODE}Local IPs: $local_ip ${CLEAN_CODE}"

# uPnP operation to deal externa ip.
target_ip=`hostname -I | awk '{print $1}'`

if command -v dig &> /dev/null
then
    public_ip=`dig +short myip.opendns.com @resolver1.opendns.com`
else
    echo -e "${ERROR_CODE}Dig command is missing, please install dnsutils to continue. \n${CLEAN_CODE}"
    exit 1
fi

echo -e "${STEP_CODE}Public IPs: $public_ip ${CLEAN_CODE}"
echo -e "${SUCCESS_CODE}\nHandshaking port for P2P ${CLEAN_CODE}" 
endpoint_ip=`./tools/upnp_negotiator/target/release/upnp_negotiator $target_ip $TARGET_PORT`
arrEndpointIp=(${endpoint_ip//:/ })

if [ -z "${endpoint_ip}" ]; then
	echo -e "${ERROR_CODE}Handshaking went wrong, running node w/o P2P port ${CLEAN_CODE}" 
	echo -e "${STEP_CODE}Endpoint IP: $public_ip \n ${CLEAN_CODE}"
else	
	echo -e "${STEP_CODE}Endpoint IP: $public_ip:${arrEndpointIp[1]} \n ${CLEAN_CODE}"
fi

# Retrieve BS addr ID
echo -e "${SUCCESS_CODE}Retrieving P2P bootstrap ID... \n ${CLEAN_CODE}"
bs_id=`curl http://testnet.trinci.net/api/v1/p2p/id` 
bs_addr="${bs_id}${BS_ADDR}"
echo -e "${STEP_CODE}Bootstrap address: $bs_addr\n ${CLEAN_CODE}"


# Calculating DB path
if [ ! -f "bootstrap.bin" ]; then
    echo -e "${ERROR_CODE}Missing bootstrap file. \n${CLEAN_CODE}"
    exit 1
fi

echo -e "${SUCCESS_CODE}Calculating DB path... \n ${CLEAN_CODE}"
bootstrap_hash=`shasum -a 256 bootstrap.bin | cut -f1 -d' '`
db_path="${DB_PATH}${bootstrap_hash}/"
echo -e "${STEP_CODE}DB path: $db_path ${CLEAN_CODE}"


# Launch node.
if [ ! -f "./target/release/trinci-node" ]; then
    echo -e "${ERROR_CODE}Missing trinci executable. \n${CLEAN_CODE}"
    exit 1
fi

echo -e "${SUCCESS_CODE}Starting trinci node... \n ${CLEAN_CODE}"

if [ -z "${endpoint_ip}" ]; then
	# If uPnP went wrong.
	./target/release/trinci-node --local-ip $local_ip --public-ip $public_ip --http-port $HTTP_PORT --bootstrap-path $BS_PATH --p2p-bootstrap-addr $bs_addr --db-path $db_path
elif [ -z "${public_ip}" ]; then
	# If no public IP.
	./target/release/trinci-node --local-ip $local_ip --http-port $HTTP_PORT --p2p-port ${arrEndpointIp[1]} --bootstrap-path $BS_PATH --p2p-bootstrap-addr $bs_addr --db-path $db_path 
else
	# If all went good
	./target/release/trinci-node --local-ip $local_ip --public-ip $public_ip:${arrEndpointIp[1]} --http-port $HTTP_PORT --p2p-port ${arrEndpointIp[1]} --bootstrap-path $BS_PATH --p2p-bootstrap-addr $bs_addr --db-path $db_path  
fi
