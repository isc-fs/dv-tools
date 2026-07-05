// Shared session-recorder store (#55, lifted for the single-viewport board).
//
// The board's decoded-state TRANSITIONS (AS state + Go/No-Go verdict) are
// captured here, in a singleton, so the record TOGGLE (on the board's gauge
// deck) and the DEBRIEF card (on the Details tab) read one source and capture
// keeps running no matter which tab is showing. App.svelte drives observe()
// from an always-mounted $effect; the toggle calls start()/stop().

import type { OverallState } from './types';

interface Ev {
    t: number; // ms since record start
    kind: 'AS' | 'verdict';
    to: string;
}

class Recorder {
    recording = $state<boolean>(false);
    events = $state<Ev[]>([]);
    now = $state<number>(0);

    #startT = 0;
    #lastAs = '';
    #lastVerdict = '';
    // Latest observed values, tracked even when not recording so start() can
    // seed the initial state without needing props threaded to the toggle.
    #curAs = '—';
    #curVerdict: OverallState = 'standby';

    /** Called every poll from App.svelte; records transitions while recording. */
    observe(asWord: string | null, verdictState: OverallState): void {
        const as = asWord ?? '—';
        this.#curAs = as;
        this.#curVerdict = verdictState;
        if (!this.recording) return;
        const t = Date.now() - this.#startT;
        this.now = t;
        if (as !== this.#lastAs) {
            this.#lastAs = as;
            this.events = [...this.events, { t, kind: 'AS', to: as }];
        }
        if (verdictState !== this.#lastVerdict) {
            this.#lastVerdict = verdictState;
            this.events = [...this.events, { t, kind: 'verdict', to: verdictState }];
        }
    }

    start(): void {
        this.#startT = Date.now();
        this.now = 0;
        this.#lastAs = this.#curAs;
        this.#lastVerdict = this.#curVerdict;
        this.events = [
            { t: 0, kind: 'AS', to: this.#lastAs },
            { t: 0, kind: 'verdict', to: this.#lastVerdict },
        ];
        this.recording = true;
    }

    stop(): void {
        this.recording = false;
    }

    /** Debrief summary — recomputes reactively from events/now/recording. */
    get debrief(): {
        dur: number;
        transitions: number;
        dwell: [string, number][];
        faultCount: number;
        firstFault: number | null;
    } {
        const events = this.events;
        const dur = this.recording
            ? this.now
            : events.length
              ? events[events.length - 1].t
              : 0;
        const asEvents = events.filter((e) => e.kind === 'AS');
        const dwell = new Map<string, number>();
        for (let i = 0; i < asEvents.length; i++) {
            const end = i + 1 < asEvents.length ? asEvents[i + 1].t : dur;
            dwell.set(
                asEvents[i].to,
                (dwell.get(asEvents[i].to) ?? 0) + (end - asEvents[i].t),
            );
        }
        const faults = events.filter((e) => e.kind === 'verdict' && e.to === 'fault');
        return {
            dur,
            transitions: events.length,
            dwell: [...dwell.entries()].sort((a, b) => b[1] - a[1]),
            faultCount: faults.length,
            firstFault: faults.length ? faults[0].t : null,
        };
    }
}

export const recorder = new Recorder();
export const fmtT = (ms: number): string => `${(ms / 1000).toFixed(1)}s`;
