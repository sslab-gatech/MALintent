#!/bin/bash

#set -e

usage() {
  echo "Usage: $(basename "$0") <app_name> [configs_dir] [coverage_socket_address] [fuzzer_arg_1 ...]"
  echo "       $(basename "$0") -h, --help"
  echo
}

if [[ $1 == "-h" || $1 == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -lt 1 ]]; then
  echo "Invalid number of arguments."
  usage
  exit 1
fi

app_name=${1}
configs_dir=${2:-.}
coverage_socket_address=${3:-localhost:6249}
args=${@:4}


COVERAGE_AGENT="/data/local/tmp/libcoverage_instrumenting_agent.so"


run_command() {
  local -a cmd=("$@")
  echo "Executing command: ${cmd[*]}"
  "${cmd[@]}"
}

# Log the entire output
if [[ "${NO_COV}" == "1" ]]; then
  logfile="${configs_dir}/log_nocov.txt"
else
  logfile="${configs_dir}/log_cov.txt"
fi
exec > >(tee ${logfile}) 2>&1

# Check 'RERUN_CORPUS' to determine if we just run the existing corpus
if [[ "${RERUN_CORPUS}" == "1" ]]; then
  echo "Re-running corpus instead of fuzzing"
  flag_rerun="--run-corpus"
fi

# Check 'NO_COV'
if [[ "${NO_COV}" == "1" ]]; then
  echo "Not using coverage feedback for fuzzing"
  nocov="-nocov"
  flag_nocov="--no-coverage"
else
  nocov="-cov"
fi


activity=$(basename "${config%.json}")

echo "Fuzzing app: ${app_name}"
date

# Bail out if the coverage agent does not exist.
if ! adb shell ls "${COVERAGE_AGENT}" &>/dev/null; then
  echo "Coverage agent not found in /data/local/tmp. Aborting."
  exit 1
fi

# Copy the coverage agent from /data/local/tmp to the app's data directory.
echo "Copying coverage agent to app's data directory."
run_command adb shell mkdir -p "/data/data/${app_name}/code_cache/startup_agents/"
run_command adb shell cp "${COVERAGE_AGENT}" "/data/data/${app_name}/code_cache/startup_agents/"

echo "Force-stopping all active apps"
running_apps=$(adb shell dumpsys activity recents | grep RecentTaskInfo -A 20 | grep baseActivity={ | sed -e 's:^.*{::' -e 's:/.*$::' )

for app in ${running_apps}; do
  run_command adb shell am force-stop "${app}"
done

sleep 2

# Check 'NATIVE_TRACE' environment variable to determine whether to enable native tracing.
# If 'NATIVE_TRACE' is set to 'true', enable native tracing.
if [[ "${NATIVE_TRACE}" == "1" ]]; then
  echo "Enabling native tracing"
  flag_tracing="-t --traces-dir ${configs_dir}/traces${nocov}/"
else
  TIMEOUT="timeout $((3600*24))"
fi

echo "Starting fuzzer"
run_command $TIMEOUT cargo run -- -i ${configs_dir}/configs/ --corpus-dir ${configs_dir}/corpus${nocov}/ --crashes-dir ${configs_dir}/crashes${nocov}/ --overall-coverage-file ${configs_dir}/edgecount${nocov}.txt --coverage-socket-address=${coverage_socket_address} ${flag_nocov} ${flag_rerun} ${flag_tracing} ${args}
