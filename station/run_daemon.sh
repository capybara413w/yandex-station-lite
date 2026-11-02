#!/bin/sh
# Launcher script for station_daemon to capture logs and run detached
exec /data/station_daemon </dev/null >/tmp/station_daemon.log 2>&1
