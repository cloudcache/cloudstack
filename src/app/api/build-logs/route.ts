/**
 * Proxy server action → Rust SSE build-log stream.
 * Body: { projectId: string; appId: string; buildId: string }
 * Authenticates via NextAuth session and forwards to Rust backend.
 */

import { getServerSession } from "next-auth";
import { authOptions } from "@/server/utils/auth-options";
import { z } from "zod";

export const dynamic = "force-dynamic";

const bodySchema = z.object({
    projectId: z.string(),
    appId: z.string(),
    buildId: z.string(),
});

const BACKEND_URL = process.env.BACKEND_URL ?? 'http://localhost:3001';

export async function POST(request: Request) {
    const session = await getServerSession(authOptions);
    const token: string | undefined = (session as any)?.backendToken;
    if (!token) {
        return new Response('Unauthorized', { status: 401 });
    }

    let body: z.infer<typeof bodySchema>;
    try {
        body = bodySchema.parse(await request.json());
    } catch {
        return new Response('Bad Request', { status: 400 });
    }

    const { projectId, appId, buildId } = body;
    const rustUrl = `${BACKEND_URL}/api/v1/projects/${projectId}/apps/${appId}/builds/${buildId}/logs`;

    const upstreamRes = await fetch(rustUrl, {
        headers: { Authorization: `Bearer ${token}` },
        signal: request.signal,
    });

    if (!upstreamRes.ok) {
        return new Response(`Upstream error: ${upstreamRes.status}`, { status: upstreamRes.status });
    }

    return new Response(upstreamRes.body, {
        headers: {
            Connection: 'keep-alive',
            'Content-Encoding': 'none',
            'Cache-Control': 'no-cache, no-transform',
            'Content-Type': 'text/event-stream; charset=utf-8',
        },
    });
}
