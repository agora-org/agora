#!/bin/bash


# if lnd enabled, attempt to connect
if [[ ! -z "${LND_RPC_AUTHORITY}" ]]; then
    exec agora \
	 --directory $FILES_DIR \
	 --http-port $AGORA_PORT \
	 --lnd-rpc-authority $LND_RPC_AUTHORITY \
	 --lnd-rpc-cert-path $TLS_CERT_PATH \
	 --lnd-rpc-macaroon-path $INVOICES_MACAROON_PATH
# else if core-lightning enabled, attempt to connect
elif [[ ! -z "${CORE_LIGHTNING_RPC_PATH}" ]]; then
    exec agora \
	 --directory $FILES_DIR \
	 --http-port $AGORA_PORT \
	 --core-lightning-rpc-file-path $CORE_LIGHTNING_RPC_PATH
# else run simple server
else
    exec agora \
	 --directory $FILES_DIR \
	 --http-port $AGORA_PORT
fi
