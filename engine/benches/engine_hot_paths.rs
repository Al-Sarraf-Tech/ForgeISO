//! Criterion benchmarks for hot paths in the ForgeISO engine.
//!
//! These are guard-rails against perf regressions on the synchronous critical
//! path that runs on every build/inject/scan cycle. Inputs are synthetic and
//! deterministic so the benches are CI-safe and don't touch the network or
//! shell out to xorriso/squashfs-tools.
//!
//! Coverage:
//!   1. `sha256_file` — ISO content hashing on a 4 MiB buffer (proxy for the
//!      sustained per-MB throughput seen during real ISO verify/inspect).
//!   2. `generate_autoinstall_yaml` — Ubuntu autoinstall serialization.
//!   3. `generate_kickstart_cfg`    — Fedora/RHEL kickstart serialization.
//!   4. `generate_mint_preseed`     — Mint/Debian preseed serialization.
//!   5. `EngineEvent::with_bytes`   — fluent progress builder + percent calc
//!      (called per emit during downloads — runs hundreds of times/sec).
//!
//! Running:
//!     cargo bench -p forgeiso-engine
//!     cargo bench -p forgeiso-engine -- sha256
//!
//! Output: HTML report at `target/criterion/report/index.html`.

use std::io::Write;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use forgeiso_engine::config::{InjectConfig, IsoSource};
use forgeiso_engine::events::{EngineEvent, EventPhase};
use forgeiso_engine::{
    generate_autoinstall_yaml, generate_kickstart_cfg, generate_mint_preseed,
    orchestrator::sha256_file,
};

/// Build a populated InjectConfig representative of a real CLI invocation —
/// same shape used by the e2e regression tests. Heavier than `default()` so
/// generators exercise their full code paths (groups, late commands, sshkeys,
/// firewall, sysctl).
fn populated_inject_config() -> InjectConfig {
    let mut cfg = InjectConfig {
        source: IsoSource::from_raw("/tmp/forgeiso-bench.iso"),
        out_name: "bench-output.iso".to_string(),
        hostname: Some("forge-bench".to_string()),
        username: Some("operator".to_string()),
        password: Some("placeholder-password-not-a-secret".to_string()),
        realname: Some("Bench Operator".to_string()),
        timezone: Some("UTC".to_string()),
        locale: Some("en_US.UTF-8".to_string()),
        keyboard_layout: Some("us".to_string()),
        storage_layout: Some("lvm".to_string()),
        apt_mirror: Some("http://archive.ubuntu.com/ubuntu".to_string()),
        extra_packages: vec!["vim".into(), "htop".into(), "git".into(), "curl".into()],
        extra_late_commands: vec![
            "echo bench >> /etc/motd".into(),
            "systemctl enable cron".into(),
        ],
        no_user_interaction: true,
        ..Default::default()
    };
    cfg.user.groups = vec!["sudo".into(), "docker".into()];
    cfg.user.shell = Some("/bin/bash".into());
    cfg.user.sudo_nopasswd = true;
    cfg.ssh.authorized_keys = vec![
        "ssh-ed25519 AAAA0000aaaaPLACEHOLDERkey1 user@bench".into(),
        "ssh-ed25519 AAAA0000aaaaPLACEHOLDERkey2 user@bench".into(),
    ];
    cfg.ssh.allow_password_auth = Some(false);
    cfg.ssh.install_server = Some(true);
    cfg.firewall.enabled = true;
    cfg.firewall.allow_ports = vec!["22/tcp".into(), "443/tcp".into()];
    cfg
}

/// Write `size` bytes of pseudo-random-looking data into a temp file and
/// return the path. The pattern is deterministic across runs so hash output
/// is stable but non-trivial enough that compilers can't fold it.
fn make_temp_blob(size: usize) -> tempfile::NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    let mut buf = vec![0u8; 4096];
    for (i, byte) in buf.iter_mut().enumerate() {
        *byte = ((i * 131 + 17) & 0xff) as u8;
    }
    let mut written = 0;
    while written < size {
        let chunk = (size - written).min(buf.len());
        tmp.write_all(&buf[..chunk]).expect("write");
        written += chunk;
    }
    tmp.flush().expect("flush");
    tmp
}

fn bench_sha256_file(c: &mut Criterion) {
    // 4 MiB is large enough to exercise the read loop / hasher pipelines but
    // small enough to bench in milliseconds. Real ISOs are 1-5 GB; the
    // throughput here scales near-linearly to that.
    let size = 4 * 1024 * 1024;
    let tmp = make_temp_blob(size);

    let mut group = c.benchmark_group("sha256_file");
    group.throughput(Throughput::Bytes(size as u64));
    group.bench_function(BenchmarkId::new("4MiB_blob", size), |b| {
        b.iter(|| {
            let h = sha256_file(black_box(tmp.path())).expect("hash");
            black_box(h);
        });
    });
    group.finish();
}

fn bench_generate_autoinstall_yaml(c: &mut Criterion) {
    let cfg = populated_inject_config();
    c.bench_function("generate_autoinstall_yaml/populated", |b| {
        b.iter(|| {
            let yaml = generate_autoinstall_yaml(black_box(&cfg)).expect("yaml");
            black_box(yaml);
        });
    });
}

fn bench_generate_kickstart_cfg(c: &mut Criterion) {
    let cfg = populated_inject_config();
    c.bench_function("generate_kickstart_cfg/populated", |b| {
        b.iter(|| {
            let ks = generate_kickstart_cfg(black_box(&cfg)).expect("ks");
            black_box(ks);
        });
    });
}

fn bench_generate_mint_preseed(c: &mut Criterion) {
    let cfg = populated_inject_config();
    c.bench_function("generate_mint_preseed/populated", |b| {
        b.iter(|| {
            let pre = generate_mint_preseed(black_box(&cfg)).expect("preseed");
            black_box(pre);
        });
    });
}

fn bench_event_with_bytes(c: &mut Criterion) {
    // Called every 512 KB during downloads — dozens to hundreds of times
    // per ISO. Cheap, but a regression here would show up in CLI latency.
    c.bench_function("EngineEvent::with_bytes", |b| {
        b.iter(|| {
            let ev = EngineEvent::info(EventPhase::Download, "downloading")
                .with_bytes(black_box(500_000_000), black_box(1_000_000_000))
                .with_substage("download");
            black_box(ev);
        });
    });
}

criterion_group!(
    benches,
    bench_sha256_file,
    bench_generate_autoinstall_yaml,
    bench_generate_kickstart_cfg,
    bench_generate_mint_preseed,
    bench_event_with_bytes,
);
criterion_main!(benches);
