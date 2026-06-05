"use client";

import { Manager } from "socket.io-client";

const backendUrl = (process.env.NEXT_PUBLIC_BACKEND_URL?.trim() || 'http://localhost:3001').replace(/\/+$/, '');
const manager = new Manager(backendUrl);
export const podTerminalSocket = manager.socket("/pod-terminal");
