#!/bin/bash
TARGET_PORT=8000
BS_PATH='./data/bootstrap.bin'

STEP_CODE="\033[0;33m"
CLEAN_CODE="\033[0m"
SUCCESS_CODE="\033[0;32m"
ERROR_CODE="\033[0;31m"

echo -e "\nTrinci node start script v0.0.1 \n"

echo -e "${SUCCESS_CODE}Gatering network informations... \n${CLEAN_CODE}"
# Recover local ip.
# Check which command use
if command -v ip &> /dev/null
then
    local_ip=`ip addr | sed -n -e '/state UP/,/[0-9]: / p' | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | tr '\n' '|'`
    local_ip=${local_ip::-1}
fi

if command -v ifconfig &> /dev/null
then
    local_ip=`ifconfig | sed -n -e '/UP/,/[0-9]: / p' | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | tr '\n' '|'`
    local_ip=${local_ip::-1}
fi

if [ $? -ne 0 ]; then
    echo -e "${ERROR_CODE}Something went wrong checking local ip addresses. \n${CLEAN_CODE}"
    exit 1
fi

echo -e "${STEP_CODE}Local IPs: $local_ip ${CLEAN_CODE}"

# uPnP operation to deal externa ip.
target_ip=`hostname -I | awk '{print $1}'`
public_ip=`dig +short myip.opendns.com @resolver1.opendns.com`

echo -e "${STEP_CODE}Public IPs: $public_ip ${CLEAN_CODE}"

endpoint_ip=`./tools/upnp_negotiator/target/release/upnp_negotiator $target_ip $TARGET_PORT`
arrEndpointIp=(${endpoint_ip//:/ })

echo -e "${STEP_CODE}Endpoint IP: $public_ip:${arrEndpointIp[1]} \n ${CLEAN_CODE}"

# Launch node.
if [ ! -f "./target/release/trinci-node" ]; then
    echo -e "${ERROR_CODE}Missing trinci executable. \n${CLEAN_CODE}"
    exit 1
fi
echo -e "${SUCCESS_CODE}Starting trinci node... \n ${CLEAN_CODE}"

./target/release/trinci-node --local-ip $local_ip --public-ip $public_ip:${arrEndpointIp[1]} --http-port $TARGET_PORT --bootstrap-path $BS_PATH
