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

echo -e "\nTrinci node start script v0.0.2 \n"

unameOut="$(uname -s)"
case "${unameOut}" in
    Linux*)     ENVIRONMENT=Linux;;
    Darwin*)    ENVIRONMENT=Mac;;
esac

echo -e "${SUCCESS_CODE}Gatering system environment: ${STEP_CODE}$ENVIRONMENT${CLEAN_CODE}"

find_osx_local_ip_address() {
    local_ip=`ifconfig | pcregrep -M -o '^[^\t:]+:([^\n]|\n\t)*status: active' | grep -w inet | awk '{print $2}'`
    if [ $? -ne 0 ]; then
        echo -e "${ERROR_CODE}Something went wrong checking local ip addresses. \n${CLEAN_CODE}"
        exit 1
    fi
    # uPnP operation to deal externa ip.
    target_ip=$local_ip

    echo -e "${STEP_CODE}Local IPs: $local_ip ${CLEAN_CODE}"
}

find_linux_local_ip_address() {
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

    # uPnP operation to deal externa ip.
    target_ip=`hostname -I | awk '{print $1}'`

    echo -e "${STEP_CODE}Local IPs: $local_ip ${CLEAN_CODE}"
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
        echo -e "${STEP_CODE}Endpoint IP: $public_ip \n ${CLEAN_CODE}"
    else	
        echo -e "${STEP_CODE}Endpoint IP: $public_ip:${arrEndpointIp[1]} \n ${CLEAN_CODE}"
    fi
}

get_bs_address() {
    # Retrieve BS addr ID
    echo -e "${SUCCESS_CODE}Retrieving P2P bootstrap ID... ${CLEAN_CODE}"
    bs_id=`curl -s http://testnet.trinci.net/api/v1/p2p/id` 
    bs_addr="${bs_id}${BS_ADDR}"
    echo -e "${STEP_CODE}Bootstrap address: $bs_addr\n ${CLEAN_CODE}"
}

base58()
if
    local -a base58_chars=(
    1 2 3 4 5 6 7 8 9
      A B C D E F G H   J K L M N   P Q R S T U V W X Y Z
      a b c d e f g h i j k   m n o p q r s t u v w x y z
    )
    local OPTIND OPTARG o
    getopts d o
then
    shift $((OPTIND - 1))
    case $o in
      d)
        local input
        read -r input < "${1:-/dev/stdin}"
        if [[ "$input" =~ ^1.+ ]]
        then printf "\x00"; ${FUNCNAME[0]} -d <<<"${input:1}"
        elif [[ "$input" =~ ^[$(printf %s ${base58_chars[@]})]+$ ]]
        then
      {
        printf "s%c\n" "${base58_chars[@]}" | nl -v 0
        sed -e i0 -e 's/./ 58*l&+/g' -e aP <<<"$input"
      } | dc
        elif [[ -n "$input" ]]
        then return 1
        fi |
        if [[ -t 1 ]]
        then cat -v
        else cat
        fi
        ;;
    esac
else
    xxd -p -u "${1:-/dev/stdin}" |
    tr -d '\n' |
    {
      read hex
      while [[ "$hex" =~ ^00 ]]
      do echo -n 1; hex="${hex:2}"
      done
      if test -n "$hex"
      then
    dc -e "16i0$hex Ai[58~rd0<x]dsxx+f" |
    while read -r
    do echo -n "${base58_chars[REPLY]}"
    done
      fi
      echo
    }
fi

if [ $ENVIRONMENT == "Linux" ]; then
    find_linux_local_ip_address
else
    find_osx_local_ip_address
fi

get_db_folder() {
    # Calculating DB path
    if [ ! -f $BS_PATH ]; then
        echo -e "${ERROR_CODE}Missing bootstrap file. \n${CLEAN_CODE}"
        exit 1
    fi

    echo -e "${SUCCESS_CODE}Calculating DB path...${CLEAN_CODE}"
    bootstrap_hash=$(echo -n "1220"$(shasum -a 256 $BS_PATH | cut -f1 -d' ') | xxd -r -p | base58)
    db_path="${DB_PATH}${bootstrap_hash}/"
    echo -e "${STEP_CODE}DB path: $db_path ${CLEAN_CODE}"
}


find_remote_ip_address
negotiate_upnp_port
get_bs_address
get_db_folder


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
