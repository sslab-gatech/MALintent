#!/bin/bash

set -e

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd "$SCRIPT_DIR"

# This scripts copies the edge count data files from the servers to this machine.
# The file structure is the same as the one on the servers.
# The edge count data is stored in directories named './<target_app>/edgecount*'.

servers=${SERVERS:-rodimus nickel.gtisc.gatech.edu zinc.gtisc.gatech.edu goldbug.gtisc.gatech.edu grimlock.gtisc.gatech.edu computron.gtisc.gatech.edu iron.gtisc.gatech.edu}
#servers=${SERVERS:-nickel.gtisc.gatech.edu}

for server in $servers; do
	# Find all the relevant files on the server.
#	ssh "$server" find src/jni-fuzz/IntentFuzzerLibAFL/eval/ -maxdepth 2 -name 'edgecount\*' -type d
#	rsync -avz --include '*/' --include '/*/edgecount*/*' --exclude '*' --prune-empty-dirs "$server:src/jni-fuzz/IntentFuzzerLibAFL/eval/" "./tmp/"


	ssh "$server" find src/jni-fuzz/IntentFuzzerLibAFL/eval/ -maxdepth 2 -name 'edgecount\*' -type d -printf '%P\\n' | while read -r line; do
		# Copy the files to the local machine.
		echo "Copying $line from $server"
		rsync -avz "$server:src/jni-fuzz/IntentFuzzerLibAFL/eval/$line" "$(dirname $line)"
	done
done
