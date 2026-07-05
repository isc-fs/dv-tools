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

import type {
    EbsResult,
    EchoTail,
    Meta,
    NetInterface,
    StateResponse,
    TopicInfo,
    TopicSnapshot,
} from './types';

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
            iface: null,
            discovered: 7,
            connected: true,
            error: null,
            link_lost: false,
            watchdog_s: 1.5,
        });
    }
    return invoke<Meta>('get_meta');
}

/**
 * Reconnect the ROS bridge on a new domain id, optionally binding DDS to a
 * local interface IP (the direct-link Ethernet, for a point-to-point DV PC
 * link). Empty `iface` means "all interfaces". No-op in the demo.
 */
export function connect(domain: number, iface?: string): Promise<void> {
    if (IN_BROWSER) return Promise.resolve();
    return invoke<void>('connect', { domain, iface: iface && iface.trim() ? iface.trim() : null });
}

/**
 * Trigger the uDV's `/force_ebs` service (std_srvs/SetBool). `engage=true`
 * fires the Emergency Brake System for a car-on-stands checkup; `false` returns
 * it to normal. Gate this behind an explicit confirmation in the UI. In the
 * browser demo there is no backend, so it simulates a successful call.
 */
export function forceEbs(engage: boolean): Promise<EbsResult> {
    if (IN_BROWSER) {
        return Promise.resolve({
            success: true,
            message: engage
                ? 'EBS forced open (demo — no backend)'
                : 'EBS returned to normal (demo — no backend)',
        });
    }
    return invoke<EbsResult>('force_ebs', { engage });
}

/**
 * Steering self-test (#92) — call the uDV's `/activate_steering` (std_srvs/
 * SetBool). `engage=true` drives the steering actuator for a car-on-stands
 * checkup; watch `/steering/feedback` + `/steering_angle` in the echo tab to
 * confirm it moved. Gated by the stands interlock + a confirmation.
 */
export function activateSteering(engage: boolean): Promise<EbsResult> {
    if (IN_BROWSER) {
        return Promise.resolve({
            success: true,
            message: engage
                ? 'Steering activated (demo — no backend)'
                : 'Steering deactivated (demo — no backend)',
        });
    }
    return invoke<EbsResult>('activate_steering', { engage });
}

/**
 * List the host's network interfaces (for the DDS interface picker — choose the
 * direct-link Ethernet instead of typing its IP). The browser demo returns a
 * couple of plausible entries so the dropdown is populated standalone.
 */
export function listInterfaces(): Promise<NetInterface[]> {
    if (IN_BROWSER) {
        return Promise.resolve([
            { name: 'en7', ip: '10.42.0.2', loopback: false },
            { name: 'en0', ip: '192.168.1.50', loopback: false },
            { name: 'lo0', ip: '127.0.0.1', loopback: true },
        ]);
    }
    return invoke<NetInterface[]>('list_interfaces');
}

// ---- Generic echo tab ----

/** Topics visible on the graph, for the echo picker. Demo returns a sample set. */
export function listTopics(): Promise<TopicInfo[]> {
    if (IN_BROWSER) {
        return Promise.resolve([
            { name: '/debug', type_name: 'std_msgs/msg/String' },
            { name: '/assi/state', type_name: 'std_msgs/msg/UInt8' },
            { name: '/ctrl/cmd', type_name: 'geometry_msgs/msg/Twist' },
            { name: '/slam/pose', type_name: 'nav_msgs/msg/Odometry' },
            { name: '/imu', type_name: 'sensor_msgs/msg/Imu' },
            { name: '/perception/cones', type_name: 'sensor_msgs/msg/PointCloud2' },
        ]);
    }
    return invoke<TopicInfo[]>('list_topics');
}

// Demo echo: a module-local set of synthetic streams (topic → start time) so
// the multi-topic tab works standalone.
const demoEcho = new Map<string, number>();

/** Add a topic to the echo view (idempotent). */
export function echoAdd(topic: string): Promise<void> {
    if (IN_BROWSER) {
        if (!demoEcho.has(topic)) demoEcho.set(topic, Date.now());
        return Promise.resolve();
    }
    return invoke<void>('echo_add', { topic });
}

/** Remove one topic from the echo view. */
export function echoRemove(topic: string): Promise<void> {
    if (IN_BROWSER) {
        demoEcho.delete(topic);
        return Promise.resolve();
    }
    return invoke<void>('echo_remove', { topic });
}

/** Stop every topic and clear the buffer. */
export function echoClear(): Promise<void> {
    if (IN_BROWSER) {
        demoEcho.clear();
        return Promise.resolve();
    }
    return invoke<void>('echo_clear');
}

/** The merged tail across all active echo topics. */
export function echoTail(limit: number): Promise<EchoTail> {
    if (IN_BROWSER) {
        const topics = [...demoEcho.keys()].map((topic) => ({ topic, running: true }));
        const all: { arr: number; s: EchoTail['samples'][number] }[] = [];
        for (const [topic, start] of demoEcho) {
            const decoded = topic !== '/perception/cones';
            const n = Math.floor((Date.now() - start) / 500); // ~2 Hz each
            for (let i = 0; i < n; i++) {
                all.push({
                    arr: start + i * 500,
                    s: {
                        topic,
                        type_name: 'demo',
                        seq: i,
                        t_ms: i * 500,
                        summary: decoded ? `data: ${topic} #${i}` : '(live — payload not decoded)',
                    },
                });
            }
        }
        all.sort((a, b) => a.arr - b.arr); // interleave by arrival, like the backend
        return Promise.resolve({ topics, total: all.length, samples: all.slice(-limit).map((x) => x.s) });
    }
    return invoke<EchoTail>('echo_tail', { limit });
}

/** Whether the Connect control is wired to a real backend. */
export function isTauri(): boolean {
    return !IN_BROWSER;
}
