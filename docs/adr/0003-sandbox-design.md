# ADR-0003: Sandbox design — `SandboxRunner` trait, Docker default, OpenShell optional

* Status: Proposed
* Date: 2026-11-14

## Context
Fixer and Bug Fixer run untrusted code (builds, tests, package installs) from arbitrary repos, driven by an LLM that may be prompt-injected. Fixloop must be fully functional without OpenShell. OpenShell (Apache-2.0, v0.1.8 at time of writing) is an optional, stronger runner.

## Decision

### Trait
```rust
#[async_trait]
pub trait SandboxRunner: Send + Sync {
    fn kind(&self) -> RunnerKind;
    async fn preflight(&self) -> Result<RunnerInfo, SandboxError>; // is runtime present & healthy?
    async fn create(&self, spec: &SandboxSpec) -> Result<SandboxHandle, SandboxError>;
    async fn exec(&self, h: &SandboxHandle, cmd: &ExecSpec) -> Result<ExecOutput, SandboxError>;
    async fn write_files(&self, h: &SandboxHandle, files: &[FileWrite]) -> Result<(), SandboxError>;
    async fn read_file(&self, h: &SandboxHandle, path: &RelPath) -> Result<Vec<u8>, SandboxError>;
    async fn diff(&self, h: &SandboxHandle) -> Result<Patch, SandboxError>;
    async fn events(&self, h: &SandboxHandle) -> Result<Vec<SandboxEvent>, SandboxError>; // policy blocks etc.
    async fn destroy(&self, h: SandboxHandle) -> Result<(), SandboxError>;
}
```
`SandboxSpec` is runner-neutral: image, workspace source, cpu/mem/time limits, network allow list (hosts + ports), env (non-secret only), and a profile name. Each runner translates it into native controls.

### Key rule: the agent loop runs in the worker, not in the sandbox
The LLM loop lives in the trusted worker. The sandbox only executes tool actions (exec, read and write files in the workspace). So:
* LLM API keys never enter the sandbox. Inference does not need sandbox egress unless an OpenShell inference-routing deployment is chosen.
* Git credentials never enter the sandbox. The worker extracts the diff (`diff()`) and does commit/push itself with a short-lived installation token.
* Sandbox egress is limited to package registries (and the git host only if a dependency fetch needs it).

### Runners
| Runner | Isolation | Notes |
|---|---|---|
| Docker/Podman (default) | non-root UID, read-only rootfs + tmpfs workspace overlay, `--cap-drop=ALL`, `no-new-privileges`, seccomp profile shipped in `deploy/docker/`, pids/cpu/mem limits, wall-clock timeout | Egress allow list via a per-job internal network + egress proxy sidecar (HTTP CONNECT allow list). Rootless Podman supported. |
| Kubernetes Job | Pod Security `restricted`, NetworkPolicy (egress to proxy only), resource limits, `automountServiceAccountToken: false`, optional gVisor/Kata RuntimeClass | Workspace delivered via init container from a worker-produced tarball |
| Local | none | Dev only. Requires `--i-understand-local-is-unsafe` and logs a warning on every run |
| OpenShell (`feature = "openshell"`) | Landlock FS, seccomp, network policy, provider credential placeholders | See below |

### OpenShell integration (Phase 4b)
* **Before implementing, read the OpenShell docs for the pinned version (`/llms.txt` index) and pin a tested version range.** No CLI/API surface is assumed in this ADR.
* Policies ship in `deploy/openshell/`: FS = workspace + tmp only; network = git host + configured LLM endpoints + package registries for the detected ecosystem; process = no escalation.
* Inference routing: an `LlmProvider` variant that targets the OpenShell inference route, so admins can force all model traffic through approved backends. In that mode the agent loop may optionally run inside the sandbox.
* Credentials: use OpenShell provider credential placeholders where they fit. Otherwise, the worker-side push model above applies.
* Policy blocks surface as `SandboxEvent::PolicyBlock` → dashboard → proposed policy diff → human `approval`. OpenShell network policy is hot-reloadable, while FS/process policy is fixed at creation, so FS/process approvals take effect on the next run.
* Config `sandbox.runner: openshell` + missing runtime: hard error with a remediation hint, unless `sandbox.fallback: docker` is set explicitly.

## Consequences
* An egress proxy is needed for Docker/K8s allow lists, because Docker has no native per-host egress rules. It is a small Rust proxy (`fixloop-egress`) in the same image.
* Ecosystem-specific build images (Rust, Node, Python, Go, Java) are published under `ghcr.io/<org>/fixloop-sandbox-*`. Repos can override the image in `.fixloop.yaml`.
