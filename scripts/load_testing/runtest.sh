#!/bin/bash

# NOTE(!!): the script only works when all config folders are freshly created,
# i.e. no data from previous runs

# Initialize our own variables:
client_config=""
server_config=""
target=""

show_help()
{
    echo "runtest.sh -c <client_config_path> -s <server_config_path> -t <target: host where servers run>"
}

get_args()
{
    while getopts ":c:s:t:h" opt; do
        case "$opt" in
        c)  client_config=$OPTARG
            ;;
        s)  server_config=$OPTARG
            ;;
        t)  target=$OPTARG
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

server_launch()
{
    ssh -C "$target" "cd $server_config && ./warm-up.sh"
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
    ssh -C "$target" "pkill -f pelikan_twemcache"
}

get_args "${@}"
server_launch
client_run
wrap_up
