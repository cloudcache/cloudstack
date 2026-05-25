import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import React, { useEffect } from "react";
import { TerminalSetupInfoModel } from "@/shared/model/terminal-setup-info.model";
import TerminalStreamed from "./terminal-streamed";

export function TerminalDialog({
  terminalInfo,
  children
}: {
  terminalInfo: TerminalSetupInfoModel;
  children: React.ReactNode;
}) {

  const [isOpen, setIsOpen] = React.useState(false);

  return (
    <Dialog open={isOpen} onOpenChange={(isO) => {
      setIsOpen(isO);
    }}>
      <DialogTrigger asChild>
        {children}
      </DialogTrigger>
      <DialogContent className="sm:max-w-[1300px]">
        <DialogHeader>
          <DialogTitle>Terminal</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          {terminalInfo ? <TerminalStreamed terminalInfo={terminalInfo} /> : 'Currently there is no Terminal available'}
        </div>
      </DialogContent>
    </Dialog>
  )
}
