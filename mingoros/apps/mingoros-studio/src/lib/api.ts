// Typed wrappers around the Tauri commands the backend exposes. The
// whole frontend goes through these so the command names + payload
// shapes live in one place.
//
// CRUCIAL: when we are NOT running inside Tauri (i.e. `npm run dev` in
// a plain browser), we cannot call `invoke` — there is no backend. In
// that case these wrappers return a BAKED DEMO instead, so the app
// renders standalone. The demo alternates a full NOMINAL snapshot and
// a full FAULT snapshot every ~2.5 s. The demo path can NEVER run
// inside the bundled Tauri app.

import { invoke } from '@tauri-apps/api/core';

import type { Meta, StateResponse, TopicSnapshot } from './types';

/**
 * True only inside the bundled Tauri webview. We detect the *absence*
 * of Tauri internals rather than their presence so the demo lights up
 * in any plain browser (including SSR-free `npm run dev`).
 */
const IN_BROWSER =
    typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);

// ---- Baked demo samples (reproduced from the source dashboard) ----

const DEBUG_NOMINAL =
    'AS AS_READY || ASMS:on TS:on SDC:closed EBS:off ABS:ok || brakes:on mission:sel R2D:off motion:standstill finished:no || RES:GO || EBSinit:ok';
const DEBUG_FAULT =
    'AS AS_EMERGENCY || ASMS:on TS:off SDC:open EBS:on ABS:fail || brakes:on mission:sel R2D:off motion:standstill finished:no || RES:ESTOP || EBSinit:ok';

/** Wrap a value the way the ROS bridge does ("data: <v>"). */
function d(v: string): string {
    return 'data: ' + v;
}

/** Full NOMINAL / FAULT topic snapshot, exactly as the source baked it. */
function demoTopics(fault: boolean): TopicSnapshot[] {
    if (!fault) {
        return [
            { label: 'AS state', topic: '/assi/state', value: d('1 (AS_READY)'), age_ms: 180, fresh: true, danger: false, state: 'ok' },
            { label: 'Raw AS', topic: '/as_state', value: d('1 (READY)'), age_ms: 210, fresh: true, danger: false, state: 'ok' },
            { label: 'DV status', topic: '/dv/status', value: d('2 (Ready)'), age_ms: 240, fresh: true, danger: false, state: 'ok' },
            { label: 'RES', topic: '/res/status', value: d('3 (GO)'), age_ms: 160, fresh: true, danger: false, state: 'ok' },
            { label: 'RES go', topic: '/res/go', value: d('1 (GO)'), age_ms: 170, fresh: true, danger: false, state: 'ok' },
            { label: 'Mission', topic: '/ami/mission', value: d('1  (→ mission_id 2 acceleration)'), age_ms: 300, fresh: true, danger: false, state: 'ok' },
            { label: 'Safety', topic: '/debug', value: DEBUG_NOMINAL, age_ms: 120, fresh: true, danger: false, state: 'ok' },
        ];
    }
    return [
        { label: 'AS state', topic: '/assi/state', value: d('3 (AS_EMERGENCY)'), age_ms: 150, fresh: true, danger: true, state: 'ok' },
        { label: 'Raw AS', topic: '/as_state', value: d('3 (EMERGENCY)'), age_ms: 200, fresh: true, danger: true, state: 'ok' },
        { label: 'DV status', topic: '/dv/status', value: d('3 (Running)'), age_ms: 230, fresh: true, danger: false, state: 'ok' },
        { label: 'RES', topic: '/res/status', value: d('1 (ESTOP)'), age_ms: 140, fresh: true, danger: true, state: 'ok' },
        { label: 'RES go', topic: '/res/go', value: d('0 (NO-GO)'), age_ms: 150, fresh: true, danger: false, state: 'ok' },
        { label: 'Mission', topic: '/ami/mission', value: d('1  (→ mission_id 2 acceleration)'), age_ms: 2400, fresh: false, danger: false, state: 'ok' },
        { label: 'Safety', topic: '/debug', value: DEBUG_FAULT, age_ms: 110, fresh: true, danger: true, state: 'ok' },
    ];
}

/**
 * Demo alternates NOMINAL <-> FAULT every ~2.5 s. Stateless clock read
 * (no counter to fall out of sync); this path never runs inside Tauri.
 */
const DEMO_PERIOD_MS = 2500;

// ---- Tauri commands (with demo fallback) ----

/** Poll the current topic snapshot. */
export function getState(): Promise<StateResponse> {
    if (IN_BROWSER) {
        const fault = Math.floor(Date.now() / DEMO_PERIOD_MS) % 2 === 1;
        return Promise.resolve({ topics: demoTopics(fault) });
    }
    return invoke<StateResponse>('get_state');
}

/** Poll backend/connection metadata. */
export function getMeta(): Promise<Meta> {
    if (IN_BROWSER) {
        return Promise.resolve({
            backend: 'fake (demo)',
            domain: 0,
            connected: true,
            error: null,
            watchdog_s: 1.5,
        });
    }
    return invoke<Meta>('get_meta');
}

/** Reconnect the ROS bridge on a new domain id. No-op in the demo. */
export function connect(domain: number): Promise<void> {
    if (IN_BROWSER) return Promise.resolve();
    return invoke<void>('connect', { domain });
}

/** Whether the Connect control is wired to a real backend. */
export function isTauri(): boolean {
    return !IN_BROWSER;
}
