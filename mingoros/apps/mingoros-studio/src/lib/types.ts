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
