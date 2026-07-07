// Live startup-sequence derivation for the Startup tab (#feat-59).
//
// Ports the uDV startup-trainer's checklist + "what's next" engine, but reads
// the LIVE /debug-parsed signals + the AS-state word instead of simulator
// toggles. Pure + read-only: it tells the operator which step they're on and
// what to do next; it never commands anything. Mirrors the firmware AS gates
// (as_transition.hpp) and the EBS init FSM (ebs_manager.cpp) at the level the
// wire exposes (state names, not raw hydraulics).

import type { ParsedSignal } from './types';
import { assiLook, type AssiLook } from './model';

export type StepState = 'done' | 'current' | 'pending' | 'err';
export type Phase =
    | 'unpowered'
    | 'normal'
    | 'ready'
    | 'driving'
    | 'finished'
    | 'emergency'
    | 'failed';

export interface StartupStep {
    id: string;
    label: string;
    state: StepState;
    detail: string;
}

export interface PowerRailItem {
    id: string;
    label: string;
    on: boolean;
    tone: 'on' | 'off' | 'warn' | 'bad';
    val: string;
}

export type RailNodeState = 'done' | 'active' | 'err' | 'pending';
export interface EbsRailNode {
    key: string;
    label: string;
    state: RailNodeState;
}
export interface EbsView {
    /** raw EBS-init token from /debug, e.g. "ok" / "Done" / "CheckActuator1" / "Failed" */
    raw: string;
    label: string;
    tone: 'idle' | 'run' | 'done' | 'fail';
    /** index into EBS_RAIL, -1 when unknown/idle */
    railIdx: number;
    /** the self-check sub-state progression, one node per firmware sub-state */
    rail: EbsRailNode[];
    /** true when the token isn't a named sub-state (wire only emitted ok/fail) */
    coarse: boolean;
}

export interface PneumaticView {
    /** AS SDC relay (D4) closed — live from /debug SDC, falls back to sub-state */
    sdcClosed: boolean;
    /** storage-tank pressure state (no numeric bar on the wire — state only) */
    tanks: 'good' | 'low' | 'charging' | 'unknown';
    /** EBS actuator 1 (D1) firing now — from the EBS-init sub-state */
    firingD1: boolean;
    /** EBS actuator 2 (D2) firing now */
    firingD2: boolean;
    /** brake line pressurised / caliper clamping */
    braking: boolean;
    /** EBS armed (self-check passed, holding the brakes) */
    armed: boolean;
}

export interface StartupView {
    phase: Phase;
    asWord: string | null;
    assi: AssiLook;
    pneumatic: PneumaticView;
    steps: StartupStep[];
    currentId: string | null;
    hint: string;
    hintTone: 'info' | 'success' | 'alert';
    power: PowerRailItem[];
    ebs: EbsView;
    dwellRemaining: number; // s until the 5 s READY dwell elapses (0 once past)
    receiving: boolean;
}

export interface StartupInputs {
    signalMap: Record<string, ParsedSignal>;
    asWord: string | null;
    /** live data actually arriving (honest-connection `receiving`) = uDV powered */
    receiving: boolean;
    /** decoded /ami/mission word, e.g. "trackdrive" / "inspection" / null */
    missionWord: string | null;
    /** /dv/status fresh + healthy = DV pipeline up */
    dvpcUp: boolean;
    /** seconds AS has been in READY (client-timed), 0 if not READY */
    readyElapsedS: number;
}

export const DWELL_S = 5;

// The EBS init sub-states the firmware can name on /debug (best-effort; the
// wire may only emit ok/fail, which we still handle).
const EBS_RAIL = ['Start', 'WaitLow', 'CheckPressure', 'WaitTS', 'CheckActuator1', 'WaitInterActuatorCheck', 'CheckActuator2', 'Done'];
const EBS_LABEL: Record<string, string> = {
    start: 'SDC check',
    waitlow: 'settling',
    checkpressure: 'pressure',
    waitts: 'waiting for TS',
    checkactuator1: 'firing D1',
    waitinteractuatorcheck: 'settling',
    checkactuator2: 'firing D2',
    done: 'armed',
    ok: 'armed',
    armed: 'armed',
    failed: 'FAILED',
    fail: 'FAILED',
};

// Short node captions, aligned to EBS_RAIL indices.
const EBS_SHORT = ['SDC', 'LOW', 'PRESS', 'TS', 'ACT 1', 'WAIT', 'ACT 2', 'ARMED'];
const CP_IDX = 2; // CheckPressure — where a storage-pressure failure surfaces

function ebsView(raw0: string): EbsView {
    const raw = raw0 || '';
    const k = raw.toLowerCase();
    const done = ['ok', 'done', 'armed', 'pass', 'passed', 'ready'].includes(k);
    const failed = k.includes('fail');
    const tone: EbsView['tone'] = failed ? 'fail' : done ? 'done' : k ? 'run' : 'idle';
    // map the token to a rail index when it's a named sub-state
    const idx = EBS_RAIL.findIndex((s) => s.toLowerCase() === k);
    const railIdx = done ? EBS_RAIL.length - 1 : failed ? CP_IDX : idx;
    // the wire only named a sub-state when idx>=0; otherwise it's coarse (ok/fail)
    const coarse = idx < 0 && !failed;

    const rail: EbsRailNode[] = EBS_RAIL.map((key, i) => {
        let state: RailNodeState;
        if (failed) state = i < CP_IDX ? 'done' : i === CP_IDX ? 'err' : 'pending';
        else if (done) state = 'done';
        else if (idx < 0) state = 'pending'; // unknown / not yet reporting a sub-state
        else if (i < idx) state = 'done';
        else if (i === idx) state = 'active';
        else state = 'pending';
        return { key, label: EBS_SHORT[i] ?? key, state };
    });

    return { raw, label: EBS_LABEL[k] ?? (raw || '—'), tone, railIdx, rail, coarse };
}

const HINTS: Record<string, string> = {
    lv: 'Power the low-voltage system (LVMS) — no uDV heartbeat yet.',
    asms: 'Turn on the ASMS (autonomous system master).',
    ts: 'Turn on the TSMS to activate the Tractive System.',
    ebs: 'EBS self-check running — wait for it to arm the brakes.',
    res: 'Clear the RES e-stop.',
    mission: 'Select a mission on the AMI.',
    ready: 'Car should reach AS READY now.',
    dwell: 'Hold — the mandatory 5 s READY dwell is counting down.',
    go: 'Give GO (RES) to enter AS DRIVING.',
};

/** Derive the whole live startup view from the current signals. Pure. */
export function deriveStartup(i: StartupInputs): StartupView {
    const v = (name: string): string =>
        (i.signalMap[name.toLowerCase()]?.val ?? '').toLowerCase();

    const asU = (i.asWord || '').toUpperCase();
    const emergency = asU.includes('EMERGENCY');
    const driving = asU.includes('DRIVING');
    const finished = asU.includes('FINISHED');
    const ready = asU.includes('READY');
    const readyReached = ready || driving || finished;

    const lv = i.receiving; // a live uDV heartbeat means LV is powered
    const asms = v('asms') === 'on';
    const ts = v('ts') === 'on';
    const ebs = ebsView(i.signalMap['ebsinit']?.val ?? '');
    const ebsDone = ebs.tone === 'done';
    const ebsFailed = ebs.tone === 'fail';
    const resWord = v('res');
    const estop = emergency || resWord.includes('estop') || resWord.includes('emergency');
    const resOk = lv && !estop;
    const missionSel =
        v('mission') === 'sel' ||
        (i.missionWord != null &&
            i.missionWord !== '' &&
            !/^none$|^-1$|^0$/.test(i.missionWord.toLowerCase()));

    const dwellRemaining = ready && !driving ? Math.max(0, DWELL_S - i.readyElapsedS) : 0;

    // step done/err predicates, in operator order
    type Def = { id: string; label: string; done: boolean; err?: boolean };
    const defs: Def[] = [
        { id: 'lv', label: 'LV power — uDV heartbeat', done: lv },
        { id: 'asms', label: 'ASMS on — autonomous master', done: lv && asms },
        { id: 'ts', label: 'TSMS on — Tractive System active', done: lv && ts },
        { id: 'ebs', label: 'EBS self-check passed — brakes armed', done: ebsDone, err: ebsFailed },
        { id: 'res', label: 'RES healthy — no e-stop', done: resOk, err: estop },
        { id: 'mission', label: 'Mission selected (AMI)', done: missionSel },
        { id: 'ready', label: 'AS READY reached', done: readyReached },
        { id: 'dwell', label: '5 s READY dwell elapsed', done: readyReached && dwellRemaining <= 0 },
        { id: 'go', label: 'GO → AS DRIVING', done: driving || finished },
    ];

    // current = first step that is neither done nor errored
    const currentDef = defs.find((d) => !d.done && !d.err);
    const currentId = currentDef?.id ?? null;

    const steps: StartupStep[] = defs.map((d) => {
        let state: StepState = d.err ? 'err' : d.done ? 'done' : 'pending';
        if (state === 'pending' && d.id === currentId) state = 'current';
        const detail =
            d.id === 'ebs' && !d.done && !d.err ? ebs.label : d.id === 'dwell' && dwellRemaining > 0 ? dwellRemaining.toFixed(1) + ' s' : '';
        return { id: d.id, label: d.label, state, detail };
    });

    // phase + hint
    let phase: Phase = 'normal';
    let hint = '';
    let hintTone: StartupView['hintTone'] = 'info';
    if (!lv) {
        phase = 'unpowered';
        hint = 'No uDV heartbeat — power the car (LVMS) and check the link.';
        hintTone = 'info';
    } else if (emergency) {
        phase = 'emergency';
        hint = 'AS EMERGENCY. Turn the ASMS off (then on) to reset — nothing else clears it.';
        hintTone = 'alert';
    } else if (ebsFailed) {
        phase = 'failed';
        hint = 'EBS self-check FAILED — storage pressure too low, the car cannot arm. Reset to retry.';
        hintTone = 'alert';
    } else if (driving) {
        phase = 'driving';
        hint = 'AS DRIVING — brakes released, ASSI yellow-flashing. Startup complete.';
        hintTone = 'success';
    } else if (finished) {
        phase = 'finished';
        hint = 'AS FINISHED — mission complete, ASSI blue.';
        hintTone = 'success';
    } else if (ready) {
        phase = 'ready';
        hint =
            dwellRemaining > 0
                ? `AS READY — hold ${dwellRemaining.toFixed(1)} s more, then GO.`
                : 'AS READY and dwell elapsed — give GO (RES) to drive.';
        hintTone = dwellRemaining > 0 ? 'info' : 'success';
    } else {
        hint = currentId ? HINTS[currentId] ?? (currentDef?.label ?? '') : 'Ready — give GO.';
        hintTone = 'info';
    }

    // power-up rail (read-only indicators)
    const power: PowerRailItem[] = [
        { id: 'lv', label: 'LV', on: lv, tone: lv ? 'on' : 'off', val: lv ? 'heartbeat' : 'no data' },
        { id: 'asms', label: 'ASMS', on: lv && asms, tone: lv && asms ? 'on' : 'off', val: asms ? 'on' : 'off' },
        { id: 'ts', label: 'TS', on: lv && ts, tone: lv && ts ? 'on' : 'off', val: ts ? 'active' : 'off' },
        {
            id: 'dvpc',
            label: 'DVPC',
            on: i.dvpcUp,
            tone: i.dvpcUp ? 'on' : /track/i.test(i.missionWord ?? '') ? 'warn' : 'off',
            val: i.dvpcUp ? 'up' : 'down',
        },
    ];

    // Pneumatic schematic state — what activates. The firing actuator comes
    // from the EBS-init sub-state; the SDC relay is live from /debug; tank
    // pressure is state-only (no bar value on the wire).
    const ebsKey = (i.signalMap['ebsinit']?.val ?? '').toLowerCase();
    const firingD1 = ebsKey === 'checkactuator1';
    const firingD2 = ebsKey === 'checkactuator2';
    const sdcTok = v('sdc');
    const pneumatic: PneumaticView = {
        firingD1,
        firingD2,
        armed: ebsDone,
        sdcClosed: sdcTok ? sdcTok === 'closed' : ebs.railIdx > CP_IDX || ebsDone,
        tanks: ebsFailed
            ? 'low'
            : ebsDone || ebs.railIdx > CP_IDX
              ? 'good'
              : lv && ebs.tone === 'run'
                ? 'charging'
                : 'unknown',
        braking: firingD1 || firingD2 || (ebsDone && !driving),
    };

    return {
        phase,
        asWord: i.asWord,
        assi: assiLook(i.asWord),
        pneumatic,
        steps,
        currentId,
        hint,
        hintTone,
        power,
        ebs,
        dwellRemaining,
        receiving: i.receiving,
    };
}
