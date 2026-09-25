#!/usr/bin/env bash
# Plays a few visitors against the live share while a demo is recorded.
until url=$(bunflared ls | awk '$3 == "5174,3000" { print $2 }') && [ -n "$url" ]; do
  sleep 1
done
sleep "${TRAFFIC_DELAY:-4}"
for i in $(seq 1 45); do
  curl -s -o /dev/null "$url/" &
  curl -s -o /dev/null "$url/_port/3000/api/items?page=$i" &
  [ $((i % 6)) = 0 ] && curl -s -o /dev/null "$url/missing" &
  [ $((i % 11)) = 0 ] && curl -s -o /dev/null "$url/_port/3000/boom" &
  sleep "0.$((RANDOM % 6 + 2))"
done
