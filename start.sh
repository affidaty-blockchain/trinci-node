#!/bin/bash

# Node vars.
TARGET_PORT=8001
HTTP_PORT=8000
BS_PATH_OFFLINE="./data/testnet-bootstrap.bin"
TESTNET_URL="https://testnet.trinci.net"
MAINNET_URL="https://relay.mainnet.trinci.net"
DB_PATH="./db/"
AUTOREPLICANT_NODE="n"

# Output settings.
STEP_CODE="\033[0;33m"
CLEAN_CODE="\033[0m"
SUCCESS_CODE="\033[0;32m"
ERROR_CODE="\033[0;31m"

chose_network() {
    echo "Choose an environment: offline, testnet, mainnet, custom.  Type 'q' to exit"
    read NETWORK
    case "${NETWORK}" in
        offline*)   offline_start;;
        testnet*)   start "$NETWORK";;
        mainnet*)   start "$NETWORK";;
        custom*)    custom_start;;
        q*)         exit;;
        *)          chose_network;;
    esac
}

chose_autorepl() {
    echo "Do you want to use auto replicant feature? [ y | n ]"
    read AUTOREPLICANT_NODE
    case "${AUTOREPLICANT_NODE}" in
        y*)         AUTOREPLICANT_NODE='y';;
        n*)         AUTOREPLICANT_NODE='n';;
        *)          chose_autorepl;;
    esac
}

find_remote_ip_address() {
    if command -v dig &> /dev/null
    then
        public_ip=`dig TXT +short o-o.myaddr.l.google.com @ns1.google.com`
        public_ip=`echo $public_ip | tr -d '"'`
    else
        echo -e "${ERROR_CODE}Dig command is missing, please install dnsutils to continue. \n${CLEAN_CODE}"
        exit 1
    fi
    echo -e "${STEP_CODE}Public IPs: $public_ip ${CLEAN_CODE}"
}

negotiate_upnp_port() {
    echo -e "${SUCCESS_CODE}\nHandshaking port for P2P ${CLEAN_CODE}" 
    endpoint_ip=`./tools/upnp_negotiator/target/release/upnp_negotiator $target_ip $TARGET_PORT`
    arrEndpointIp=(${endpoint_ip//:/ })

    if [ -z "${endpoint_ip}" ]; then
        echo -e "${ERROR_CODE}Handshaking went wrong, running node w/o P2P port ${CLEAN_CODE}"
        exit 1
    else	
        echo -e "${STEP_CODE}Endpoint IP: $public_ip:${arrEndpointIp[1]} \n ${CLEAN_CODE}"
    fi
}

check_trinci_exec() {
    if [ ! -f "./target/release/trinci-node" ]; then
        echo -e "${ERROR_CODE}Missing trinci executable. \n${CLEAN_CODE}"
        exit 1
    fi
}

find_local_ip() {
    if [ $ENVIRONMENT == "Linux" ]; then
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

        # uPnP operation to deal externa ip.
        target_ip=`hostname -I | awk '{print $1}'`
    else
        local_ip=`ifconfig | pcregrep -M -o '^[^\t:]+:([^\n]|\n\t)*status: active' | grep -w inet | awk '{print $2}'`
        if [ $? -ne 0 ]; then
            echo -e "${ERROR_CODE}Something went wrong checking local ip addresses. \n${CLEAN_CODE}"
            exit 1
        fi
        # uPnP operation to deal externa ip.
        target_ip=$local_ip
    fi

    echo -e "${STEP_CODE}Local IPs: $local_ip ${CLEAN_CODE}"
}

offline_start () {
    check_trinci_exec
    ./target/release/trinci-node --http-port $HTTP_PORT --bootstrap-path $BS_PATH_OFFLINE --db-path $DB_PATH"offline"
}

custom_start () {
    chose_autorepl
    find_remote_ip_address
    find_local_ip
    negotiate_upnp_port

    if [ $AUTOREPLICANT_NODE == "y" ] 
    then
        echo "Insert remote bootstrap node ip/url. Es: https://192.168.0.1 or https://some-dns.org"
        read AUTOREPLICANT_NODE_IP
        check_trinci_exec
        ./target/release/trinci-node --local-ip $local_ip --public-ip $public_ip:${arrEndpointIp[1]} --http-port $HTTP_PORT --p2p-port ${arrEndpointIp[1]} --autoreplicant-procedure $AUTOREPLICANT_NODE_IP
    else
        echo "Insert p2p bootstrap address. es: '12D3KooWMFJAmuKyrAXjcGv8bhD8yRw2hNYx4CtBPj2cqQD83btd@/ip4/10.0.0.1/tcp/9006'"
        read CUSTOM_P2P_BOOTSTRAP_ADDRESS
        echo "Insert database path"
        read CUSTOM_DB_PATH
        echo "Insert boostrap file path"
        read CUSTOM_BS_PATH
        check_trinci_exec
        ./target/release/trinci-node --local-ip $local_ip --public-ip $public_ip:${arrEndpointIp[1]} --http-port $HTTP_PORT --p2p-port ${arrEndpointIp[1]} --bootstrap-path $CUSTOM_BS_PATH --p2p-bootstrap-addr $CUSTOM_P2P_BOOTSTRAP_ADDRESS --db-path $CUSTOM_DB_PATH
    fi
}

start () {
    find_remote_ip_address
    find_local_ip
    negotiate_upnp_port

    network=$1
    check_trinci_exec
    [[ $network = "testnet" ]] && AUTOREPLICANT_NODE_IP=$TESTNET_URL || AUTOREPLICANT_NODE_IP=$MAINNET_URL
    ./target/release/trinci-node --local-ip $local_ip --public-ip $public_ip:${arrEndpointIp[1]} --http-port $HTTP_PORT --p2p-port ${arrEndpointIp[1]} --autoreplicant-procedure $AUTOREPLICANT_NODE_IP
}

#MAIN
echo -e "\nTrinci node start script v0.1.0 \n"

unameOut="$(uname -s)"
case "${unameOut}" in
    Linux*)     ENVIRONMENT="Linux";;
    Darwin*)    ENVIRONMENT="Mac";;
esac

chose_network
