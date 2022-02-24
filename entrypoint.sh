#!/bin/bash

echo '$LND_RPC_AUTHORITY'
echo $LND_RPC_AUTHORITY

pwd
ls -l .

echo '$FILES_DIR'
echo $FILES_DIR

# if lnd enabled, attempt to connect
if [[ ! -z "${LND_RPC_AUTHORITY}" ]]
then
    exec agora --directory $FILES_DIR --http-port $AGORA_PORT --lnd-rpc-authority $LND_RPC_AUTHORITY --lnd-rpc-cert-path .lnd/tls.cert --lnd-rpc-macaroon-path .lnd/invoices.macaroon
# else run simple server
else
    exec agora --directory $FILES_DIR --http-port $AGORA_PORT
fi
