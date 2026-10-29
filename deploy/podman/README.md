# Containers' health for the admin dashboard

Every service of the server runs in a Podman container (Umami, PostgreSQL,
Redis, `pulsars-api`, `pulsars-web`). The admin dashboard shows their state
(`GET /v1/admin/containers`), but the API never gets the Podman socket,
which is root-equivalent. Instead, a root timer writes a snapshot every
minute, and the API reads it from a read-only bind mount.

| File | Role |
|---|---|
| `pulsars-podman-releve` | POSIX shell, no `jq`: writes `{"taken_at", "ps", "inspect", "stats"}` (the raw JSON of `podman ps --all`, `podman inspect` and `podman stats --all --no-stream`; `null` for a command that failed) atomically to `/var/lib/pulsars-podman/releve.json`, mode 0644, directory 0755 |
| `pulsars-podman-releve.service` | oneshot, as root |
| `pulsars-podman-releve.timer` | every minute (`AccuracySec=5s`) |

## Install

```sh
install -m 0755 deploy/podman/pulsars-podman-releve /usr/local/sbin/
install -m 0644 deploy/podman/pulsars-podman-releve.service \
    deploy/podman/pulsars-podman-releve.timer /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now pulsars-podman-releve.timer
# Check: a snapshot younger than a minute.
systemctl start pulsars-podman-releve.service
ls -l /var/lib/pulsars-podman/releve.json
```

`PULSARS_PODMAN_DIR` (in a drop-in) moves the directory. The containers
must be root's: rootless containers of another user are not seen by root's
`podman`.

## Give it to the API

Mount the directory read-only into the API's container, in its Quadlet
(`/etc/containers/systemd/pulsars-api.container`):

```ini
[Container]
Volume=/var/lib/pulsars-podman:/run/pulsars-podman:ro
```

and point the API at the file (`/etc/pulsars-api/pulsars-api.env`):

```sh
PODMAN_SNAPSHOT_PATH=/run/pulsars-podman/releve.json
```

Mount the directory, not the file: the script replaces the file with
`mv`, which a file bind mount would not follow.

What the dashboard then says:

- `reason: not_configured`: `PODMAN_SNAPSHOT_PATH` is not set;
- `reason: missing`: no file at that path (the timer never ran, or the
  mount is missing);
- `reason: unreadable`: not JSON, over 2 MB, or `podman ps` failed;
- `stale: true`: the snapshot is more than 3 minutes old (the timer
  stopped).
