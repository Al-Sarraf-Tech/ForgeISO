# Observability

ForgeISO emits two parallel observability channels:

1. **Structured JSON logs** (always on). Daily-rolling files written to
   `<FORGEISO_LOG_DIR or $XDG_STATE_HOME/forgeiso>/forgeiso.log.YYYY-MM-DD`
   for the CLI, `forgeiso-tui.log.<date>` for the TUI, and
   `forgeiso-gui.log.<date>` for the GUI. Filter with `jq`:

   ```bash
   jq 'select(.level=="WARN" or .level=="ERROR")' \
     "$HOME/.local/state/forgeiso/forgeiso.log.$(date -I)"
   ```

2. **OpenTelemetry traces** (feature-gated, off by default). Spans
   wrap each top-level engine operation and ship to either an OTLP
   collector or stdout for local debugging.

Both channels share the same `tracing` event source — the OTLP layer is
attached when the `otel` feature is enabled at compile time, and adds
no runtime cost in default builds.

## Enabling OpenTelemetry

Build any of the three frontends with the `otel` cargo feature:

```bash
cargo build --release --features otel               # all crates in workspace
cargo build --release -p forgeiso-cli --features otel
cargo build --release -p forgeiso-tui --features otel
cargo build --release -p forge-slint --features otel
```

At runtime, point the binary at an OTLP HTTP endpoint with the
`FORGEISO_OTEL_ENDPOINT` environment variable. If unset, traces fall
back to a stdout exporter (useful for local debugging without a
collector).

```bash
# Ship to a Tempo / Jaeger / OTel Collector OTLP HTTP endpoint
FORGEISO_OTEL_ENDPOINT=http://localhost:4318/v1/traces forgeiso build ...

# No endpoint set — stdout exporter prints spans as JSON
forgeiso build ...
```

The endpoint must accept the OTLP HTTP/protobuf protocol on
`/v1/traces`. The default OTel Collector receiver, Tempo's distributor,
and recent Jaeger releases all satisfy this.

## Spans Emitted

The engine wraps each top-level orchestrator phase in a span so the
trace tree mirrors the operation. Sub-spans nest naturally under the
parent and inherit any caller-side context.

| Span name        | Phase     | Sub-spans (`phase` field)              |
|------------------|-----------|----------------------------------------|
| `inject_phase`   | Inject    | `inject_autoinstall`, `setup`, `extract`, `place`, `repack` |
| `build_phase`    | Build     | (top-level only)                       |
| `scan_phase`     | Scan      | (top-level only)                       |
| `verify_phase`   | Verify    | (top-level only)                       |

Each span carries `service.name=forgeiso` and `service.version=<crate
version>` resource attributes. Custom event fields (`source`, `name`,
`artifact`) appear on the appropriate span.

## Local Tempo + Grafana Setup

The simplest way to see traces locally is a Tempo container fronted by
Grafana. Save as `docker-compose.tempo.yml`:

```yaml
version: "3.9"
services:
  tempo:
    image: grafana/tempo:latest
    command: ["-config.file=/etc/tempo.yml"]
    ports:
      - "4318:4318"   # OTLP HTTP
      - "3200:3200"   # Tempo HTTP
    volumes:
      - ./tempo.yml:/etc/tempo.yml:ro
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
```

`tempo.yml`:

```yaml
server:
  http_listen_port: 3200
distributor:
  receivers:
    otlp:
      protocols:
        http:
          endpoint: 0.0.0.0:4318
storage:
  trace:
    backend: local
    local:
      path: /tmp/tempo
```

Run it:

```bash
docker compose -f docker-compose.tempo.yml up -d
FORGEISO_OTEL_ENDPOINT=http://localhost:4318/v1/traces \
  ./target/release/forgeiso build --config example.yml --out /tmp/iso
```

Open Grafana at <http://localhost:3000>, add Tempo as a data source
pointing at `http://tempo:3200`, and explore the trace.

## Local Jaeger Setup

Jaeger's all-in-one image accepts OTLP directly:

```bash
docker run --rm -p 4318:4318 -p 16686:16686 \
  -e COLLECTOR_OTLP_ENABLED=true \
  jaegertracing/all-in-one:latest

FORGEISO_OTEL_ENDPOINT=http://localhost:4318/v1/traces \
  ./target/release/forgeiso build ...
```

Open <http://localhost:16686>, pick the `forgeiso` service, and inspect
spans.

## Stdout Exporter (No Collector)

When `FORGEISO_OTEL_ENDPOINT` is unset, the stdout exporter prints
spans as JSON to stdout on shutdown — useful for one-off debugging
when running an OTLP collector is overkill:

```bash
cargo run --release --features otel -p forgeiso-cli -- doctor
# ... normal CLI output ...
# {"resourceSpans":[{"resource":{"attributes":[{"key":"service.name", ...}]}, ...}]}
```

## Failure Modes

OpenTelemetry init is fail-open: if the exporter cannot connect, the
binary keeps running with file logging and prints a single
`forgeiso: OpenTelemetry init failed: ...` line on stderr. Existing
event channels (in-app log pane for GUI/TUI, stderr for CLI) are
unaffected.
