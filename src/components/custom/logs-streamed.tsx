/**
 * Streams logs from a given SSE URL (Rust backend).
 * The caller is responsible for constructing the URL, e.g. via
 *   backend.apps.logsUrl(projectId, appId, token)
 */
import { useEffect, useRef, useState } from "react";
import { Textarea } from "@/components/ui/textarea";
import React from "react";
import {
    HoverCard,
    HoverCardContent,
    HoverCardTrigger,
} from "@/components/ui/hover-card"
import { Source_Code_Pro } from "next/font/google";
import { cn } from "@/frontend/utils/utils";

const sourceCodePro = Source_Code_Pro({
    subsets: ["latin"],
    variable: "--font-sans",
});

export default function LogsStreamed({
    logsUrl,
    fullHeight = false,
}: {
    logsUrl?: string;
    fullHeight?: boolean;
    /** @deprecated use logsUrl instead */
    namespace?: string;
    /** @deprecated use logsUrl instead */
    podName?: string;
    /** @deprecated use logsUrl instead */
    buildJobName?: string;
    linesCount?: number;
}) {
    const [isConnected, setIsConnected] = useState(false);
    const [logs, setLogs] = useState<string>('');
    const textAreaRef = useRef<HTMLTextAreaElement>(null);

    const initializeConnection = async (url: string, controller: AbortController) => {
        setLogs('Loading...');
        const signal = controller.signal;

        const apiResponse = await fetch(url, { signal });

        if (!apiResponse.ok) return;
        if (!apiResponse.body) return;
        setIsConnected(true);

        const reader = apiResponse.body
            .pipeThrough(new TextDecoderStream())
            .getReader();

        setLogs('');
        while (true) {
            const { value, done } = await reader.read();
            if (done) {
                setIsConnected(false);
                break;
            }
            if (value) {
                setLogs((prevLogs) => prevLogs + value);
            }
        }
    }

    useEffect(() => {
        if (!logsUrl) return;
        const controller = new AbortController();
        initializeConnection(logsUrl, controller);
        return () => {
            setLogs('');
            controller.abort();
        };
    }, [logsUrl]);

    useEffect(() => {
        if (textAreaRef.current) {
            textAreaRef.current.scrollTop = textAreaRef.current.scrollHeight;
        }
    }, [logs]);

    return <>
        <div className="space-y-4">
            <Textarea ref={textAreaRef} value={logs} readOnly className={cn(
                (fullHeight ? "h-[80vh]" : "h-[400px]"),
                " bg-slate-900 text-white ",
                sourceCodePro.className)} />
            <div className="w-fit">
                <HoverCard>
                    <HoverCardTrigger>
                        {isConnected ? <div className="w-3 h-3 rounded-full bg-green-500"></div> : <div className="w-3 h-3 rounded-full bg-slate-500"></div>}
                    </HoverCardTrigger>
                    <HoverCardContent className="text-sm">
                        {isConnected ? 'Connected to Logstream' : 'Disconnected from Logstream'}
                    </HoverCardContent>
                </HoverCard>
            </div>
        </div>
    </>;
}
