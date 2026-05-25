"use client";

import { Manager } from "socket.io-client";

const manager = new Manager();
export const podTerminalSocket = manager.socket("/pod-terminal");