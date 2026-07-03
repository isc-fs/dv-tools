// Signed auto-update, the Tauri-native equivalent of WarioCharger's flow: on
// launch the app checks the iskApps `latest.json` (configured in tauri.conf's
// `plugins.updater.endpoints`), and on confirm downloads + verifies the
// minisign signature + installs the bundle, then relaunches.
//
// Everything here is a no-op outside the bundled Tauri app (the browser demo),
// so importing it standalone is safe.

import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

const IN_BROWSER =
    typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);

export interface UpdateAvailable {
    update: Update;
    version: string;
    notes: string | null;
}

/**
 * Check the update endpoint. Returns the pending update (handle + info) if a
 * newer signed version is published, or null if up-to-date / not in Tauri.
 * Never throws — a failed/offline check just yields null.
 */
export async function checkForUpdate(): Promise<UpdateAvailable | null> {
    if (IN_BROWSER) return null;
    try {
        const update = await check();
        if (update) {
            return {
                update,
                version: update.version,
                notes: update.body ?? null,
            };
        }
    } catch (e) {
        console.warn('update check failed:', e);
    }
    return null;
}

/**
 * Download + verify + install the update (reporting % via `onProgress`, or null
 * when the total size is unknown), then relaunch into the new version.
 */
export async function installUpdate(
    update: Update,
    onProgress?: (pct: number | null) => void,
): Promise<void> {
    let total = 0;
    let downloaded = 0;
    await update.downloadAndInstall((event) => {
        switch (event.event) {
            case 'Started':
                total = event.data.contentLength ?? 0;
                onProgress?.(total ? 0 : null);
                break;
            case 'Progress':
                downloaded += event.data.chunkLength;
                onProgress?.(
                    total ? Math.min(100, Math.round((downloaded / total) * 100)) : null,
                );
                break;
            case 'Finished':
                onProgress?.(100);
                break;
        }
    });
    await relaunch();
}
