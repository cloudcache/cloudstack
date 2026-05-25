import { useEffect, useRef, useState } from "react";
import { Textarea } from "@/components/ui/textarea";
import React from "react";
import {
    HoverCard,
    HoverCardContent,
    HoverCardTrigger,
} from "@/components/ui/hover-card"
import { TerminalSetupInfoModel } from "@/shared/model/terminal-setup-info.model";
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { podTerminalSocket } from "@/frontend/sockets/sockets";
import { StreamUtils } from "@/shared/utils/stream.utils";
import { Button } from "@/components/ui/button";

export default function TerminalStreamed({
    terminalInfo,
}: {
    terminalInfo: TerminalSetupInfoModel;
}) {
    const [isConnected, setIsConnected] = useState(false);
    const terminalWindow = useRef<HTMLDivElement>(null);

    const [terminal, setTerminal] = useState<Terminal | undefined>(undefined);
    const [sessionTerminalInfo, setSessionTerminalInfo] = useState<TerminalSetupInfoModel | undefined>(undefined);

    const startTerminalSession = (terminalType: 'sh' | 'bash') => {
        if (!terminalInfo || !terminalWindow || !terminalWindow.current) {
            return;
        }
        const terminalSessionKey = `${terminalInfo.namespace}-${terminalInfo.podName}-${terminalInfo.containerName}-${terminalType}-${new Date().getTime()}`;
        const termInfo = {
            ...terminalInfo,
            terminalSessionKey,
            terminalType,
        };
        const terminalInputKey = StreamUtils.getInputStreamName(termInfo);
        const terminalOutputKey = StreamUtils.getOutputStreamName(termInfo);
        console.log(`InputKey ${terminalInputKey}`);
        console.log(`OutputKey ${terminalOutputKey}`);

        var term = new Terminal();
        term.open(terminalWindow.current);
        term.onData((data) => {
            podTerminalSocket.emit(terminalInputKey, data);
        });

        podTerminalSocket.on(terminalOutputKey, (data: string) => {
            term.write(data);
        });
        podTerminalSocket.emit('openTerminal', termInfo);
        setTerminal(term);
        setSessionTerminalInfo(termInfo);
    };

    const disconnectTerminalSession = () => {
        terminal?.dispose();
        if (sessionTerminalInfo) {
            podTerminalSocket.emit('closeTerminal', sessionTerminalInfo);
            setSessionTerminalInfo(undefined);
        }
    }


    return <>
        <div className="space-y-4">
            {!sessionTerminalInfo ? <>
                <div className="flex gap-4">
                    <Button variant="secondary" onClick={() => startTerminalSession('sh')}>Start sh</Button>
                    <Button variant="secondary" onClick={() => startTerminalSession('bash')}>Start bash</Button>
                </div>
            </> : <Button variant="secondary" onClick={() => disconnectTerminalSession()}>Disconnect Session</Button>}

            <div className={sessionTerminalInfo ? 'px-4 py-4 bg-black rounded-lg' : ''}>
                <div ref={terminalWindow}></div>
            </div>
        </div>
    </>;
}
