import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import React from "react";
import LogsStreamed from "@/components/custom/logs-streamed";

export function LogsDialog({
  logsUrl,
  onClose,
  children,
  /** @deprecated use logsUrl instead */
  namespace,
  /** @deprecated use logsUrl instead */
  podName,
}: {
  logsUrl?: string;
  onClose?: () => void;
  children: React.ReactNode;
  /** @deprecated */
  namespace?: string;
  /** @deprecated */
  podName?: string;
}) {

  const [isOpen, setIsOpen] = React.useState(false);
  const hasLogs = !!logsUrl || (!!namespace && !!podName);

  return (<>
    <div onClick={() => setIsOpen(true)}>
      {children}
    </div>
    <Dialog open={isOpen} onOpenChange={(isO) => {
      setIsOpen(isO);
      if (onClose && !isO) {
        onClose();
      }
    }}>
      <DialogContent className="sm:max-w-[1300px]">
        <DialogHeader>
          <DialogTitle>Logs</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          {hasLogs
            ? <LogsStreamed logsUrl={logsUrl} fullHeight={true} />
            : 'Currently there are no Logs available'
          }
        </div>
      </DialogContent>
    </Dialog>
  </>
  )
}
