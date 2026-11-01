#!/bin/bash

# ビルド
docker build -t 192.168.1.92:5000/pdf-converter:0.3.0 .
docker tag 192.168.1.92:5000/pdf-converter:0.3.0 192.168.1.92:5000/pdf-converter:latest

# push
docker push 192.168.1.92:5000/pdf-converter:0.3.0
docker push 192.168.1.92:5000/pdf-converter:latest
