#!/bin/bash

# NOTE(!!): the script only works when all config folders are freshly created,
# i.e. no data from previous runs

instances=30

# Initialize our own variables:
client_config=""
server_config=""

show_help()
{
    echo "runtest.sh -c <client_config_path> -s <server_config_path>"
}

get_args()
{
    while getopts ":c:s:h" opt; do
        case "$opt" in
        c)  client_config=$OPTARG
            ;;
        s)  server_config=$OPTARG
            ;;
        h)
            show_help
            exit 0
            ;;
        \?)
            echo "unrecognized option $opt"
            show_help
            exit 1
            ;;
        esac
    done
}

server_warmup()
{
    cd "$server_config" || exit 1

    ./bring-up.sh

    local nready=0
    while [ $nready -lt $instances ]
    do
        nready=$(grep -l "prefilling slab" log/twemcache-*.log | wc -l)
        echo "$(date): $nready out of $instances servers are warmed up"
        sleep 10
    done

    cd - > /dev/null || exit 1
}


client_run()
{
    cd "$client_config" || exit 1

    ./test.sh

    local nrunning=1
    while [ $nrunning -gt 0 ]
    do
        nrunning=$(pgrep -c rpc-perf)
        echo "$(date): $nrunning clients are still running"
        sleep 10
    done

    cd - > /dev/null || exit 1
}

wrap_up()
{
    pkill -f pelikan_twemcache
}

get_args "${@}"
server_warmup
client_run
wrap_up
