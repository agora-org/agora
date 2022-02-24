#!/usr/bin/env bash

# exit from script if error was raised.
set -e

# error function is used within a bash function in order to send the error
# message directly to the stderr output and exit.
error() {
    echo "$1" > /dev/stderr
    exit 0
}

# return is used within bash function in order to return the value.
return() {
    echo "$1"
}

# set_default function gives the ability to move the setting of default
# env variable from docker file to the script thereby giving the ability to the
# user override it durin container start.
set_default() {
    # docker initialized env variables with blank string and we can't just
    # use -z flag as usually.
    BLANK_STRING='""'

    VARIABLE="$1"
    DEFAULT="$2"

    if [[ -z "$VARIABLE" || "$VARIABLE" == "$BLANK_STRING" ]]; then

        if [ -z "$DEFAULT" ]; then
            error "You should specify default variable"
        else
            VARIABLE="$DEFAULT"
        fi
    fi

   return "$VARIABLE"
}

# Set default variables if needed.
TARGETHOST=$(set_default "$TARGETHOST" "localhost")
RPCHOST=$(set_default "$RPCHOST" "localhost")
RPCUSER=$(set_default "$RPCUSER" "devuser")
RPCPASS=$(set_default "$RPCPASS" "devpass")
DEBUG=$(set_default "$DEBUG" "info")
NETWORK=$(set_default "$NETWORK" "testnet")
CHAIN=$(set_default "$CHAIN" "bitcoin")
BACKEND=$(set_default "$BACKEND" "btcd")

if [[ "$BACKEND" == "bitcoind" ]]; then
    cmd="lnd \
    	 --noseedbackup \
	 --logdir=/data \
	 --$CHAIN.active \
	 --$CHAIN.$NETWORK \
	 --$CHAIN.node=$BACKEND \
	 --$BACKEND.rpchost=$RPCHOST \
	 --$BACKEND.rpcuser=$RPCUSER \
	 --$BACKEND.rpcpass=$RPCPASS \
	 --$BACKEND.zmqpubrawblock=tcp://${RPCHOST}:28332 \
	 --$BACKEND.zmqpubrawtx=tcp://${RPCHOST}:28333 \
	 --rpclisten=0.0.0.0:10009 \
	 --debuglevel=$DEBUG \
	 --tlsextradomain=lnd \
	 --tor.active \
	 --tor.control=tor-node:9051 \
	 --tor.socks=tor-node:9050 \
	 --tor.v3 \
	 --tor.targetipaddress=$TARGETHOST \
	 --listen=0.0.0.0:9735 \
	 $@"
    echo $cmd
    sh ./wait-for-block-index.sh "$cmd"
else
    exec lnd \
	 --noseedbackup \
	 --logdir=/data \
	 --$CHAIN.active \
	 --$CHAIN.$NETWORK \
	 --$CHAIN.node=$BACKEND \
	 --$BACKEND.rpccert=/rpc/rpc.cert \
	 --$BACKEND.rpchost=$RPCHOST \
	 --$BACKEND.rpcuser=$RPCUSER \
	 --$BACKEND.rpcpass=$RPCPASS \
	 --rpclisten=0.0.0.0:10009 \
	 --debuglevel=$DEBUG \
	 --tlsextradomain=lnd \
	 --tor.active \
	 --tor.control=tor-node:9051 \
	 --tor.socks=tor-node:9050 \
	 --tor.v3 \
	 --tor.targetipaddress=$TARGETHOST \
	 --listen=0.0.0.0:9735 \
	 $@
fi
