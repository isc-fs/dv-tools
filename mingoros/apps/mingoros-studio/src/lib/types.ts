// Wire types shared between the Tauri Rust commands and the Svelte
// frontend. Each interface mirrors a `#[derive(Serialize)]` struct on
// the Rust side (see `src-tauri/src/*.rs`). Keeping the shapes in
// lockstep is enforced by hand for now.
//
// The data contract is fixed: the app polls `get_state` / `get_meta`
// every 250 ms and reconnects via `connect { domain }`.

/** Per-topic snapshot returned inside `get_state().topics`. */
export interface TopicSnapshot {
    /** Operator-facing short name (e.g. "AS state", "Safety"). */
    label: string;
    /** ROS topic path (e.g. "/assi/state"). */
    topic: string;
    /** Latest value string, or null when nothing has arrived. */
    value: string | null;
    /** Age of the last sample in ms, or null when never seen. */
    age_ms: number | null;
    /** Whether the last sample is within the watchdog window. */
    fresh: boolean;
    /** Whether the backend flagged this sample as a hard fault. */
    danger: boolean;
    /** Subscription lifecycle for this topic. */
    state: 'ok' | 'waiting' | 'unavailable';
}

/** Response shape of `get_state`. */
export interface StateResponse {
    topics: TopicSnapshot[];
}

/** Response shape of `get_meta`. Fields may be partially present. */
export interface Meta {
    backend?: string;
    domain?: number;
    /** Local interface DDS is bound to (direct-link Ethernet IP), or null. */
    iface?: string | null;
    /** Topics seen on the graph at connect — a "DV PC reachable" signal. */
    discovered?: number;
    connected?: boolean;
    error?: string | null;
    watchdog_s?: number;
}

/** One parsed `NAME:value` token out of the /debug firmware string. */
export interface ParsedSignal {
    name: string;
    val: string;
}

/** Good / bad / neutral tri-state a signal value classifies to. */
export type SignalClass = 'good' | 'bad' | 'neu';

/** PASS / FAIL / HOLD kind for a checklist row. */
export type RowKind = 'pass' | 'fail' | 'hold';

/** Overall board state (drives the ambient wash + banner + stamp). */
export type OverallState = 'fault' | 'hold' | 'go' | 'standby';

/** Verdict + human-readable reason spelled out on the stamp. */
export interface Verdict {
    state: OverallState;
    reason: string;
}

/** Result of the `/force_ebs` (std_srvs/SetBool) service call. */
export interface EbsResult {
    success: boolean;
    message: string;
}

/** A host network interface (from `list_interfaces`) — for the DDS iface picker. */
export interface NetInterface {
    name: string;
    ip: string;
    loopback: boolean;
}

/** A topic on the graph (from `list_topics`) — for the generic echo picker. */
export interface TopicInfo {
    name: string;
    type_name: string;
    qos?: string | null;
    note?: string | null;
}

/** One received message in the generic echo view (mirrors Rust `Sample`). */
export interface EchoSample {
    topic: string;
    type_name: string;
    seq: number;
    /** Milliseconds since the echo subscription started. */
    t_ms: number;
    /** Decoded value string, or "(live — payload not decoded)" for unknown types. */
    summary: string;
}

/** Response shape of `echo_tail`: the running session's recent samples. */
export interface EchoTail {
    /** Topic being echoed, or null when no session is running. */
    topic: string | null;
    /** Whether the background stream is still alive (false once it ends). */
    running: boolean;
    /** Total samples received this session (the ring buffer may hold fewer). */
    total: number;
    samples: EchoSample[];
}
