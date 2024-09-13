#!/bin/bash

apps="$@"

eval_dir="eval-fdroid"

for app in $apps; do
	echo -e "\nRunning $app"
	echo "RUST_BACKTRACE=full ANDROID_SERIAL=localhost:55${PORT} ./run.sh ${app} ./${eval_dir}/${app} localhost:62${PORT}"
	RUST_BACKTRACE=full ANDROID_SERIAL=localhost:55${PORT} ./run.sh ${app} ./${eval_dir}/${app} localhost:62${PORT}
done
