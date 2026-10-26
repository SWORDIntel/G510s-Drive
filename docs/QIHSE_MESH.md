# QIHSE mesh telemetry

`examples/plugins/qihse_mesh.py` reads a scoped AIMEM count for the G510
status display. It connects to the standalone loopback AIMEM service, not the
production QIHSE cluster. The service is QKP-only: the plugin loads the
protected environment configuration, pins the service's QKP identity using
the matching native SDK/library, authenticates as `AIMEM_SERVICE`, and issues
`AIMEM COUNT-SCOPED` rather than `DBSIZE`. It keeps the session open across
polls. It does not use an operator credential or downgrade to cleartext.

When authentication or transport fails, the plugin waits 300 seconds before
retrying instead of generating a two-second AUTH storm. Check the standalone
service and its key/configuration before restarting the telemetry unit;
repeated bad-password probes can lock out legitimate users.

Run `python3 -m unittest discover -s tests -p test_qihse_mesh.py` from this
repository to verify the count and backoff behavior. A live probe should read
only through the restricted service API; never inspect AIMEM storage files.
