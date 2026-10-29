# PD status acceptance

HW2.1 with unloaded charger. A one-second 5 V sink session was allowed to expire;
[status.txt](status.txt) shows `last_session` as TimedOut with release confirmed,
and `last_command` as the subsequent Stop with its separate ID. The [recording](timeout.ndjson)
contains the corresponding events. [verified.json](verified.json) records the
application readback and unchanged loader. The custom image remains installed.

The host unit test checks snapshot matching when no topic event is available.
This run did not deliberately saturate the device event queue; that remains a
separate hardware test. Status stores the latest session and command in RAM,
not a durable history, and a later command can replace the last command result.
