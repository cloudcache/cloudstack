/**
 * SSE stream that provides live app deployment status to the browser.
 * Polls the Rust backend's /api/v1/monitoring/app-status every 5 s and
 * pushes changes as SSE events — same protocol as the old K8s-watch route
 * so the PodsStatusPollingService client code is unchanged.
 */

import { getServerSession } from "next-auth";
import { authOptions } from "@/server/utils/auth-options";

export const dynamic = "force-dynamic";

const BACKEND_URL = (process.env.BACKEND_URL?.trim() || 'http://localhost:3001').replace(/\/+$/, '');
const POLL_INTERVAL_MS = 5000;

export async function POST(request: Request) {
    const session = await getServerSession(authOptions);
    const token: string | undefined = (session as any)?.backendToken;
    if (!token) {
        return new Response('Unauthorized', { status: 401 });
    }

    const encoder = new TextEncoder();
    let stopped = false;

    const stream = new ReadableStream({
        async start(controller) {
            const send = (data: unknown) => {
                if (stopped) return;
                try {
                    controller.enqueue(encoder.encode(`data: ${JSON.stringify(data)}\n\n`));
                } catch {
                    stopped = true;
                }
            };

            // Send initial snapshot
            try {
                const res = await fetch(`${BACKEND_URL}/api/v1/monitoring/app-status`, {
                    headers: { Authorization: `Bearer ${token}` },
                });
                if (res.ok) send(await res.json());
            } catch (e) {
                console.error('[DeployStatus] initial fetch error', e);
            }

            // Poll every POLL_INTERVAL_MS
            while (!stopped) {
                await new Promise(r => setTimeout(r, POLL_INTERVAL_MS));
                if (stopped) break;
                try {
                    const res = await fetch(`${BACKEND_URL}/api/v1/monitoring/app-status`, {
                        headers: { Authorization: `Bearer ${token}` },
                    });
                    if (res.ok) send(await res.json());
                } catch (e) {
                    console.error('[DeployStatus] poll error', e);
                }
            }
        },
        cancel() {
            stopped = true;
        },
    });

    return new Response(stream, {
        headers: {
            Connection: 'keep-alive',
            'Content-Encoding': 'none',
            'Cache-Control': 'no-cache, no-transform',
            'Content-Type': 'text/event-stream; charset=utf-8',
        },
    });
}
