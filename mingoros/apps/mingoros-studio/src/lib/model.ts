// Pure, DOM-free, Svelte-free derivations for the Go / No-Go board.
//
// Everything here is a plain function so it can be unit-tested in
// isolation and reused by both App.svelte and the child components.
// The classification word-sets, the EBS:on special case, the
// off/no -> neutral (HOLD) rule, the danger keywords, the signal
// grouping and the verdict/overall state machine are reproduced
// EXACTLY from the original self-contained dashboard.

import type {
    ParsedSignal,
    RowKind,
    SignalClass,
    TopicSnapshot,
    Verdict,
} from './types';

// ---- Classification word-sets (case-insensitive on the value) ----
// Returns "good" | "bad" | "neu". EBS:on is the special bad case.
// Anything not matched (notably off / no) falls through to "neu",
// which reads as HOLD — expected-off on a stopped car.
const GOOD =
    /^(on|ok|closed|go|ready|yes|set|sel|standstill|armed|active|driving|engaged|true)$/;
const BAD = /^(open|fail|fault|estop|emergency|timeout|false)$/;

/**
 * Classify a single signal value. `name` matters only for the EBS
 * special case (EBS activated = emergency), everything else keys off
 * the lower-cased value.
 */
export function classifySignal(name: string, val: string): SignalClass {
    const v = (val || '').trim().toLowerCase();
    if (name.toUpperCase() === 'EBS' && v === 'on') return 'bad'; // EBS activated = emergency
    if (BAD.test(v)) return 'bad';
    if (GOOD.test(v)) return 'good';
    return 'neu';
}

/**
 * Parse the /debug firmware string into ordered NAME:value tokens.
 * Generic: any whitespace/pipe-separated token containing ':' counts,
 * so unknown signals still render. Strips the leading "AS <STATE>" and
 * any ROS "data:" wrapper token.
 */
export function parseDebug(dbg: string | null | undefined): ParsedSignal[] {
    if (!dbg) return [];
    return String(dbg)
        .split(/[\s|]+/)
        .filter(Boolean)
        .filter((t) => t.includes(':'))
        .map((t) => {
            const i = t.indexOf(':');
            return { name: t.slice(0, i), val: t.slice(i + 1) };
        })
        .filter((s) => s.name.length > 0 && s.val.length > 0)
        // drop a leading ROS "data: N" wrapper token if one slipped through
        .filter(
            (s) => !(s.name.toLowerCase() === 'data' && /^-?\d/.test(s.val)),
        );
}

// ---- Checklist grouping ----
// Which parsed signals belong to which checklist group. Anything not
// named here (unknown firmware token) falls into the safety group so
// it is never silently dropped. RES is pulled out for its own banner.
export const SAFETY_ORDER = ['ASMS', 'TS', 'SDC', 'EBS', 'ABS', 'EBSinit'];
export const DRIVE_ORDER = ['brakes', 'mission', 'R2D', 'motion', 'finished'];

/** Long human-readable name for each known signal. */
export const NICE: Record<string, string> = {
    ASMS: 'Autonomous Mission Select switch',
    TS: 'Tractive system',
    SDC: 'Shutdown circuit',
    EBS: 'Emergency brake system',
    ABS: 'Autonomous System Brake self-check',
    EBSinit: 'EBS init / self-test',
    brakes: 'Service-brake pressure',
    mission: 'Mission selected',
    R2D: 'Ready-to-drive',
    motion: 'Vehicle motion',
    finished: 'Mission finished flag',
};

/**
 * Wire token -> the label shown to a human. The uDV firmware prints the
 * autonomous-system-brake check as `ABS:` on the wire, but the system is the
 * **ASB** (Autonomous System Brake) — its firmware method is `ASBChecksOK()`,
 * and it is NOT automotive anti-lock braking. We parse the wire token verbatim
 * (so the raw /debug dump stays honest) and relabel it for display everywhere a
 * signal name is surfaced to the operator.
 */
const DISPLAY: Record<string, string> = { ABS: 'ASB' };

/** The operator-facing label for a wire signal name (identity if unmapped). */
export function displayName(name: string): string {
    return DISPLAY[name] ?? name;
}

/** ASSI colour + flashing, matching the car's Autonomous System Status
 *  Indicator light. */
export interface AssiLook {
    color: 'yellow' | 'blue' | 'grey';
    blink: boolean;
}

/**
 * Map the `/assi/state` word to the car's ASSI light (FS-Rules Autonomous
 * System Status Indicator), so the app shows the SAME thing as the car:
 * AS_READY = yellow, AS_DRIVING = yellow flashing, AS_FINISHED = blue solid,
 * AS_EMERGENCY = blue flashing, AS_OFF (or unknown) = off/grey.
 */
export function assiLook(asWord: string | null | undefined): AssiLook {
    const w = (asWord || '').toUpperCase();
    if (w.includes('DRIVING')) return { color: 'yellow', blink: true };
    if (w.includes('READY')) return { color: 'yellow', blink: false };
    if (w.includes('EMERGENCY')) return { color: 'blue', blink: true };
    if (w.includes('FINISHED')) return { color: 'blue', blink: false };
    return { color: 'grey', blink: false };
}

/** Format a millisecond age as "N.Ns" (null -> "0.0s"). */
export function fmtAge(ms: number | null | undefined): string {
    return (ms == null ? 0 : ms / 1000).toFixed(1) + 's';
}

/**
 * Extract the compact display word from a topic value.
 * "data: 1 (AS_READY)" -> "AS_READY"; falls back to the raw value.
 */
export function extractWord(value: string | null | undefined): string | null {
    if (!value) return null;
    const m = value.match(/\(([^)]*)\)/);
    if (m) return m[1].trim();
    const m2 = value.match(/data:\s*(.+)$/i);
    return (m2 ? m2[1] : value).trim();
}

/**
 * Derive the good/bad/neutral *tone* of a fact/RES cell straight from
 * its value word. Danger short-circuits to "no". Mirrors the source's
 * `classifyTopicWord`. Returns the CSS tone token used on fact cards.
 */
export function classifyTopicWord(
    value: string | null,
    danger: boolean,
): 'go' | 'no' | 'idle' {
    if (danger) return 'no';
    const w = (extractWord(value) || '').toLowerCase();
    if (/estop|emergency|fail|open|timeout|no-?go|none|unavailable/.test(w)) {
        return w.includes('no-go') || w === 'none' ? 'idle' : 'no';
    }
    if (
        /ready|go|ok|driving|running|finished|acceleration|skidpad|autocross|trackdrive|ebs_test|inspection|sel/.test(
            w,
        )
    ) {
        return 'go';
    }
    return 'idle';
}

/** Marker glyph for a checklist row kind. */
export function markerGlyph(kind: RowKind): string {
    return kind === 'pass' ? '✓' : kind === 'fail' ? '✗' : '–';
}

/** Status word for a checklist row kind. */
export function statusWord(kind: RowKind): string {
    return kind === 'pass' ? 'PASS' : kind === 'fail' ? 'FAIL' : 'HOLD';
}

/** Map a signal class to the PASS/FAIL/HOLD row kind. */
export function classToKind(c: SignalClass): RowKind {
    return c === 'good' ? 'pass' : c === 'bad' ? 'fail' : 'hold';
}

// ---- Banner aggregation across the 7 priority topics ----

export interface BannerAgg {
    anyDanger: boolean;
    anyStale: boolean;
    anyOk: boolean;
    okCount: number;
}

/** Fold the topic list into the four aggregate flags the banners need. */
export function aggregate(topics: TopicSnapshot[]): BannerAgg {
    let anyDanger = false,
        anyStale = false,
        anyOk = false,
        okCount = 0;
    for (const r of topics) {
        if (r.state === 'ok') {
            anyOk = true;
            okCount++;
            if (r.danger) anyDanger = true;
            if (!r.fresh) anyStale = true;
        }
    }
    return { anyDanger, anyStale, anyOk, okCount };
}

/**
 * Overall board state machine. Contract precedence:
 *   danger (or a BAD safety signal) > stale > ok > waiting.
 */
export function overallStatus(
    topics: TopicSnapshot[],
    signals: ParsedSignal[],
): 'fault' | 'hold' | 'go' | 'standby' {
    const { anyDanger, anyStale, anyOk } = aggregate(topics);
    const badSignal = signals.some(
        (s) => classifySignal(s.name, s.val) === 'bad',
    );
    if (anyDanger || badSignal) return 'fault';
    if (anyStale) return 'hold';
    if (anyOk) return 'go';
    return 'standby';
}

/** Build the {topic -> snapshot} lookup used all over the render path. */
export function indexByTopic(
    topics: TopicSnapshot[],
): Record<string, TopicSnapshot> {
    const byTopic: Record<string, TopicSnapshot> = {};
    for (const r of topics) byTopic[r.topic] = r;
    return byTopic;
}

/**
 * Readiness verdict + stamp reason. Fault spells out blockers WITH
 * values; hold spells out the pending interlocks; go is the all-clear.
 * `watchdogS` feeds the stale-heartbeat message. Reproduced exactly
 * from the source's readiness block.
 */
export function deriveVerdict(
    topics: TopicSnapshot[],
    signals: ParsedSignal[],
    byTopic: Record<string, TopicSnapshot>,
    watchdogS: number,
): Verdict {
    const { anyDanger, anyStale, anyOk } = aggregate(topics);
    const badSignals = signals.filter(
        (s) => classifySignal(s.name, s.val) === 'bad',
    );

    if (anyDanger || badSignals.length > 0) {
        let reason: string;
        if (badSignals.length) {
            const parts = badSignals.map((s) => displayName(s.name) + ':' + s.val);
            reason =
                'faults: ' +
                parts.slice(0, 4).join(' · ') +
                (parts.length > 4 ? ' …' : '');
        } else {
            reason = 'safety-critical fault on a priority topic';
        }
        return { state: 'fault', reason };
    }

    if (anyStale) {
        return {
            state: 'hold',
            reason: 'heartbeat dropped past ' + watchdogS + ' s watchdog',
        };
    }

    if (anyOk) {
        // pending list: which arming interlocks are not yet satisfied
        const need = ['ASMS', 'TS', 'SDC', 'EBSinit', 'brakes', 'mission'];
        const smap: Record<string, ParsedSignal> = {};
        for (const s of signals) smap[s.name.toLowerCase()] = s;
        const pending: string[] = [];
        for (const nm of need) {
            const s = smap[nm.toLowerCase()];
            if (!s) {
                pending.push(displayName(nm) + ':?');
            } else if (classifySignal(s.name, s.val) !== 'good') {
                pending.push(displayName(s.name) + ':' + s.val);
            }
        }
        // RES must read GO
        const resRow = byTopic['/res/status'];
        const resWord =
            resRow && resRow.state === 'ok'
                ? extractWord(resRow.value) || ''
                : '';
        if (classifySignal('RES', resWord) !== 'good') {
            pending.push('RES:' + (resWord || '?'));
        }

        if (pending.length === 0) {
            return { state: 'go', reason: 'all interlocks pass · safe to arm' };
        }
        return {
            state: 'hold',
            reason:
                'pending: ' +
                pending.slice(0, 4).join(' · ') +
                (pending.length > 4 ? ' …' : ''),
        };
    }

    return {
        state: 'standby',
        reason: 'no topics yet — check connection / pipeline',
    };
}
