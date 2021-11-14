#!/bin/bash

source .env
# if lnd enabled, attempt to connect
if [ "${LND_ENABLED}" = "1" ]
then ./bin/agora --directory files --http-port $AGORA_PORT --lnd-rpc-authority $LND_RPC_AUTHORITY --lnd-rpc-cert-path .lnd/tls.cert --lnd-rpc-macaroon-path .lnd/invoices.macaroon
# else run simple server
else ./bin/agora --directory files --http-port $AGORA_PORT
fi