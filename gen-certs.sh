#!/bin/bash -e

# Generates all required TLS certificates and keys

cd $(dirname $0)
project_dir=$(realpath .)

# Give "regen" argument to regenerate

if [ "$1" == "regen" ]
then
    echo "Cleaning old certs"
    rm -r certs/
fi

# Give "check" to stop succesfully if the folder exists

if [ "$1" == "check" ]
then
    [ -d "certs/" ] && echo "Output directory certs/ already exists" && exit 0
fi

# Create directories

mkdir certs
cd certs

mkdir ca_server ca_client
mkdir server
for d in ../gen-cert-conf/client* ; do # All clients specified in the config
    b=$(basename $d)
    mkdir "${b%%.*}"
done

# Generate server root CA

cd ca_server
openssl genrsa -out root-ca.key 4096
openssl req -x509 -new -nodes -subj '/CN=ServerRootCA/O=Server root CA./C=US' \
    -key root-ca.key -sha256 -days 1024 -out root-ca.crt
openssl x509 -outform der -in root-ca.crt -out root-ca.der
cd ..

# Generate client root CA

cd ca_client
openssl genrsa -out root-ca.key 4096
openssl req -x509 -new -nodes -subj '/CN=ClientRootCA/O=Client root CA./C=US' \
    -key root-ca.key -sha256 -days 1024 -out root-ca.crt
openssl x509 -outform der -in root-ca.crt -out root-ca.der
cd ..


# Generate server certificate

cd server
openssl genrsa -out server.key 4096
# openssl req -new -key server.key -subj '/CN=localhost/O=Server/C=US' -out server.csr
openssl req -new -key server.key -out server.csr \
    -reqexts san -config $project_dir/gen-cert-conf/server.conf -extensions san
openssl x509 -req -in server.csr -CA ../ca_server/root-ca.crt -CAkey ../ca_server/root-ca.key \
    -CAcreateserial -out server.crt -days 1024 -sha256 \
    -extfile $project_dir/gen-cert-conf/server.conf -extensions san
openssl pkcs12 -export -inkey server.key -in server.crt -out server.p12 -password pass:
cd ..

# Generate client certificate

for d in client* ; do
    cd $d
    openssl genrsa -out client.key 4096
    openssl req -new -key client.key -out client.csr \
        -reqexts san -config $project_dir/gen-cert-conf/$d.conf -extensions san
    openssl x509 -req -in client.csr -CA ../ca_client/root-ca.crt -CAkey ../ca_client/root-ca.key \
        -CAcreateserial -out client.crt -days 1024 -sha256 \
        -extfile $project_dir/gen-cert-conf/server.conf -extensions san
    openssl pkcs12 -export -inkey client.key -in client.crt -out client.p12 -password pass:
    cd ..
done


# Cleanup

rm */*.csr */*.srl

echo "Done"
