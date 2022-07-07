#!/bin/bash

# Node vars.
TARGET_PORT=8001
HTTP_PORT=8000
BS_PATH='./data/offline-bootstrap.bin'
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



get_db_folder


# Launch node.
if [ ! -f "./target/release/trinci-node" ]; then
    echo -e "${ERROR_CODE}Missing trinci executable. \n${CLEAN_CODE}"
    exit 1
fi

echo -e "${SUCCESS_CODE}Starting trinci node... \n ${CLEAN_CODE}"


# If all went good
cmd="./target/release/trinci-node --http-port $HTTP_PORT --bootstrap-path $BS_PATH --db-path $db_path --offline" 

$cmd
