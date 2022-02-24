#!/usr/bin/env bash

# exit from script if error was raised.
set -e

CMD="$1"

while true
do
    echo "Trying to run cmd: $CMD" >&2
    eval $CMD &
    last_pid=$!
    echo "Starting lnd with pid: $last_pid" >&2
    echo "Sleeping for 10 seconds..."
    sleep 10
    if ps -p $last_pid; then
	echo "Found running process for pid: $last_pid" >&2
	wait $last_pid
    fi
done
