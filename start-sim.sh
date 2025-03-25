#!/bin/bash

p=5200

until ! nc -z 0.0.0.0 $p
do
	echo Not available: $p
	((p=p+1))
done

echo Available: $p

./SimElevatorServer --port $p
