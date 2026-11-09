"""bench_paths.py - the one place for machine-specific bench inputs.

The repository holds no absolute path and no service endpoint. The roots
and the service endpoints come from the environment, or from
~/.sheaf_bench.json. run_all.py writes its root flags into the environment
for every child process, so one command sets all of them.

    py run_all.py --data <root> --build <root> --datasets <root> \
                  --imdb <root> --pg-app <dir> --mariadb-app <dir> \
                  --redis-server <exe> --mongod <exe> --pgbench-data <dir>

The service names are pg, mariadb, mongo, scylla, neo4j, and redis. Each
one reads <name>_host, <name>_port, <name>_user, and <name>_password.
"""
from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESULTS = HERE / "results"
# HERE is sheaf-db/tests/e2e/realtime_bench.
SHEAF_DB = HERE.parents[2]
REPO = SHEAF_DB.parent


def _load_config() -> None:
    """Seed the environment from the machine config, when it exists."""
    p = Path(os.environ.get("SHEAF_BENCH_CONFIG") or Path.home() / ".sheaf_bench.json")
    if not p.exists():
        return
    try:
        cfg = json.loads(p.read_text(encoding="utf-8"))
    except Exception as e:  # a broken config must stop the run, not be ignored
        raise SystemExit(f"error: cannot read {p}: {e}")
    for key, value in cfg.items():
        os.environ.setdefault("SHEAF_" + key.upper(), str(value))


_load_config()


def _get(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        raise SystemExit(
            f"error: {name} is not set. Add it to {Path.home() / '.sheaf_bench.json'} "
            f"or pass the matching flag to run_all.py."
        )
    return Path(value)


def data(*parts: str) -> str:
    return str(_get("SHEAF_DATA").joinpath(*parts))


def build(*parts: str) -> str:
    return str(_get("SHEAF_BUILD").joinpath(*parts))


def datasets(*parts: str) -> str:
    return str(_get("SHEAF_DATASETS").joinpath(*parts))


def imdb(*parts: str) -> str:
    return str(_get("SHEAF_IMDB").joinpath(*parts))


def pg_app(*parts: str) -> str:
    return str(_get("SHEAF_PG_APP").joinpath(*parts))


def mariadb_app(*parts: str) -> str:
    return str(_get("SHEAF_MARIADB_APP").joinpath(*parts))


def redis_server() -> str:
    return str(_get("SHEAF_REDIS_SERVER"))


def mongod() -> str:
    return str(_get("SHEAF_MONGOD"))


def pgbench_data(*parts: str) -> str:
    return str(_get("SHEAF_PGBENCH_DATA").joinpath(*parts))


@dataclass(frozen=True)
class Service:
    """A service endpoint from the machine config."""

    host: str
    port: int
    user: str = ""
    password: str = ""

    @property
    def authority(self) -> str:
        return f"{self.host}:{self.port}"

    def uri(self, scheme: str) -> str:
        return f"{scheme}://{self.authority}"


_SERVICES: dict[str, Service] = {}


def service(name: str) -> Service:
    """Return the configured endpoint for a service.

    The host and port are required. The user and password are optional.
    """
    if name in _SERVICES:
        return _SERVICES[name]
    prefix = "SHEAF_" + name.upper() + "_"
    host = os.environ.get(prefix + "HOST")
    port = os.environ.get(prefix + "PORT")
    if not host or not port:
        raise SystemExit(
            f"error: {prefix}HOST and {prefix}PORT are not set. Add "
            f"{name}_host and {name}_port to {Path.home() / '.sheaf_bench.json'}."
        )
    try:
        port_number = int(port)
    except ValueError:
        raise SystemExit(f"error: {prefix}PORT must be an integer, got {port!r}")
    endpoint = Service(
        host=host,
        port=port_number,
        user=os.environ.get(prefix + "USER", ""),
        password=os.environ.get(prefix + "PASSWORD", ""),
    )
    _SERVICES[name] = endpoint
    return endpoint
